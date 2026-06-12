//! Repository-level extraction: walk the tree, dispatch files to language
//! extractors, and do all cross-file linking. Containment and import edges are
//! `Confidence::Resolved`; the heuristic call graph is `Confidence::Heuristic`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use core_ir::{
    CallSite, Confidence, Edge, EdgeKind, IrDocument, Location, Metadata, Node, NodeKind, RootInfo,
    Range, SCHEMA_VERSION,
};
use ignore::WalkBuilder;

use crate::error::ExtractError;
use crate::extractor::{CallRef, FileExtraction, ImportBinding, LanguageExtractor};
use crate::go::GoExtractor;
use crate::ts::TsExtractor;

const TOOL_VERSION: &str = "0.1.0";

/// One extracted file plus its directory (package) relpath and source language.
struct ExtractedFile {
    relpath: String,
    dir: String,
    /// The producing extractor's language tag (e.g. `"go"`, `"ts"`).
    language: &'static str,
    extraction: FileExtraction,
}

/// Extract a full repository into a canonical [`IrDocument`].
pub fn extract_repo(root: &Path) -> Result<IrDocument, ExtractError> {
    let module_path = read_module_path(root)?;
    let extractors: Vec<Box<dyn LanguageExtractor>> =
        vec![Box::new(GoExtractor::new()), Box::new(TsExtractor::new())];

    let files = walk_and_extract(root, &extractors)?;

    // Directory (package) → chosen package name. BTreeMap keeps it deterministic.
    let packages = collect_packages(&files);
    // Directory → its language, derived from the files directly within it.
    let dir_languages = collect_dir_languages(&files);

    // Dedup nodes/edges by id; canonical serialization re-sorts regardless.
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut edges: BTreeMap<String, Edge> = BTreeMap::new();

    // Module nodes.
    for (dir, pkg) in &packages {
        let language = dir_languages.get(dir).copied().unwrap_or("");
        let node = module_node(dir, pkg, language);
        nodes.insert(node.id.clone(), node);
    }

    // Module → Module containment (each package to its nearest ancestor package).
    for dir in packages.keys() {
        if let Some(parent) = nearest_ancestor_package(dir, &packages) {
            push_contains(&mut edges, &module_id(&parent), &module_id(dir));
        }
    }

    // Lookup of struct/class nodes by (package dir, type name) for method linking.
    let mut classes: BTreeMap<(String, String), String> = BTreeMap::new();

    // Extractor-resolved intra-file edges (e.g. TS Class→Method). Their `contains`
    // targets are already parented, so the linker must not re-parent them.
    let mut parented: BTreeSet<String> = BTreeSet::new();
    for f in &files {
        for edge in &f.extraction.edges {
            if edge.kind == EdgeKind::Contains {
                parented.insert(edge.target.clone());
            }
            edges.entry(edge.id.clone()).or_insert_with(|| edge.clone());
        }
    }

    // File and symbol nodes + Module→File and File→symbol containment.
    for f in &files {
        nodes.insert(f.extraction.file_node.id.clone(), f.extraction.file_node.clone());
        push_contains(&mut edges, &module_id(&f.dir), &f.extraction.file_node.id);

        for sym in &f.extraction.symbols {
            nodes.insert(sym.id.clone(), sym.clone());
            if sym.kind == NodeKind::Class {
                classes.insert((f.dir.clone(), sym.qualified_name.clone()), sym.id.clone());
            }
            // Methods are linked to their class (or filed) in a later pass; nodes
            // already parented by the extractor keep that parent.
            if sym.kind != NodeKind::Method && !parented.contains(&sym.id) {
                push_contains(&mut edges, &f.extraction.file_node.id, &sym.id);
            }
        }
    }

    // Method containment for languages that resolve methods by receiver (Go):
    // Class→Method when the receiver type resolves in-package, else File→Method
    // with a note. Methods already parented lexically by the extractor are skipped.
    for f in &files {
        for sym in &f.extraction.symbols {
            if sym.kind != NodeKind::Method || parented.contains(&sym.id) {
                continue;
            }
            let receiver = sym
                .metadata
                .get("receiver")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match classes.get(&(f.dir.clone(), receiver.to_string())) {
                Some(class_id) => push_contains(&mut edges, class_id, &sym.id),
                None => {
                    let mut meta = Metadata::new();
                    meta.insert(
                        "unresolvedReceiver".to_string(),
                        serde_json::Value::String(receiver.to_string()),
                    );
                    push_edge(
                        &mut edges,
                        &f.extraction.file_node.id,
                        EdgeKind::Contains,
                        &sym.id,
                        meta,
                    );
                }
            }
        }
    }

    // Import resolution is language-specific: Go imports target packages
    // (File→Module); TypeScript imports target files (File→File).
    let file_ids: BTreeSet<&str> = files.iter().map(|f| f.relpath.as_str()).collect();
    for f in &files {
        for raw in &f.extraction.imports {
            let target = match f.language {
                "go" => module_path
                    .as_deref()
                    .and_then(|mp| resolve_import(raw, mp))
                    .filter(|dir| packages.contains_key(dir))
                    .map(|dir| module_id(&dir)),
                "ts" => resolve_ts_import(raw, &f.dir, &file_ids)
                    .map(|relpath| Node::make_id(NodeKind::File, "ts", &relpath, "")),
                _ => None,
            };
            if let Some(target) = target {
                push_edge(
                    &mut edges,
                    &f.extraction.file_node.id,
                    EdgeKind::Imports,
                    &target,
                    Metadata::new(),
                );
            }
        }
    }

    resolve_calls(&files, &packages, module_path.as_deref().unwrap_or(""), &mut edges);

    Ok(IrDocument {
        schema_version: SCHEMA_VERSION.to_string(),
        root: RootInfo {
            repo_path: canonical_repo_path(root),
            analyzed_at: crate::time::now_rfc3339(),
            tool_version: TOOL_VERSION.to_string(),
        },
        nodes: nodes.into_values().collect(),
        edges: edges.into_values().collect(),
    })
}

/// Walk `root` (respecting .gitignore) and extract every supported file.
fn walk_and_extract(
    root: &Path,
    extractors: &[Box<dyn LanguageExtractor>],
) -> Result<Vec<ExtractedFile>, ExtractError> {
    let mut out = Vec::new();
    for result in WalkBuilder::new(root).build() {
        let entry = result.map_err(|e| ExtractError::Walk(e.to_string()))?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };
        let Some(extractor) = extractors.iter().find(|x| x.extensions().contains(&ext)) else {
            continue;
        };
        let relpath = rel_to_slash(path, root);
        let source = std::fs::read_to_string(path).map_err(|e| ExtractError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let language = extractor.language();
        let extraction = extractor.extract_file(&relpath, &source)?;
        out.push(ExtractedFile {
            dir: dir_of(&relpath),
            relpath,
            language,
            extraction,
        });
    }
    Ok(out)
}

/// Pick a deterministic package name per directory: the first file (by relpath)
/// whose package isn't an external `_test` package, else the first.
fn collect_packages(files: &[ExtractedFile]) -> BTreeMap<String, String> {
    let mut by_dir: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for f in files {
        by_dir
            .entry(f.dir.clone())
            .or_default()
            .push((f.relpath.clone(), f.extraction.package_name.clone()));
    }
    let mut packages = BTreeMap::new();
    for (dir, mut entries) in by_dir {
        entries.sort();
        let chosen = entries
            .iter()
            .find(|(_, pkg)| !pkg.ends_with("_test"))
            .or_else(|| entries.first())
            .map(|(_, pkg)| pkg.clone())
            .unwrap_or_default();
        packages.insert(dir, chosen);
    }
    packages
}

/// Derive each directory's language from the files directly within it: a single
/// shared language wins; a directory with mixed (or no) languages maps to `""`.
/// A `Module` is not intrinsically one language, so this is not hardcoded.
fn collect_dir_languages(files: &[ExtractedFile]) -> BTreeMap<String, &'static str> {
    let mut by_dir: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
    for f in files {
        by_dir.entry(f.dir.clone()).or_default().insert(f.language);
    }
    by_dir
        .into_iter()
        .map(|(dir, langs)| {
            let language = if langs.len() == 1 {
                langs.into_iter().next().unwrap_or("")
            } else {
                ""
            };
            (dir, language)
        })
        .collect()
}

fn module_node(dir: &str, package: &str, language: &str) -> Node {
    Node {
        id: module_id(dir),
        kind: NodeKind::Module,
        name: package.to_string(),
        qualified_name: String::new(),
        language: language.to_string(),
        location: Location {
            file: dir.to_string(),
            range: Range {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 0,
            },
        },
        // A directory has no single source slice (see ADR 0001).
        content_hash: String::new(),
        metadata: Metadata::new(),
    }
}

fn module_id(dir: &str) -> String {
    Node::make_id(NodeKind::Module, "go", dir, "")
}

fn push_contains(edges: &mut BTreeMap<String, Edge>, source: &str, target: &str) {
    push_edge(edges, source, EdgeKind::Contains, target, Metadata::new());
}

fn push_edge(
    edges: &mut BTreeMap<String, Edge>,
    source: &str,
    kind: EdgeKind,
    target: &str,
    metadata: Metadata,
) {
    let id = Edge::make_id(source, kind, target);
    edges.entry(id.clone()).or_insert(Edge {
        id,
        source: source.to_string(),
        target: target.to_string(),
        kind,
        confidence: Confidence::Resolved,
        sites: Vec::<CallSite>::new(),
        metadata,
    });
}

/// Resolve a TypeScript *relative* import specifier (`./x`, `../x`) to a repo
/// file relpath, trying `.ts`, `.tsx`, then `/index.ts`. Returns `None` for bare
/// / node_modules specifiers (skipped) or unresolved targets. tsconfig path
/// aliases are deferred (see ADR 0002).
fn resolve_ts_import(spec: &str, from_dir: &str, file_ids: &BTreeSet<&str>) -> Option<String> {
    if !(spec.starts_with("./") || spec.starts_with("../")) {
        return None;
    }
    let base = normalize_rel(from_dir, spec);
    [format!("{base}.ts"), format!("{base}.tsx"), format!("{base}/index.ts")]
        .into_iter()
        .find(|candidate| file_ids.contains(candidate.as_str()))
}

/// Join `base_dir` with a relative specifier and collapse `.`/`..` segments,
/// yielding a forward-slash relpath (no extension).
fn normalize_rel(base_dir: &str, spec: &str) -> String {
    let mut parts: Vec<&str> = if base_dir == "." {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    for part in spec.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Map an import path to a repo directory, or `None` for stdlib/external.
fn resolve_import(import_path: &str, module_path: &str) -> Option<String> {
    if import_path == module_path {
        return Some(".".to_string());
    }
    import_path
        .strip_prefix(&format!("{module_path}/"))
        .map(|rest| rest.to_string())
}

/// Symbol indexes used to resolve raw call references.
#[derive(Default)]
struct CallIndex {
    /// (package dir, function name) → Function node ids (Go Rule 2).
    functions_in_pkg: BTreeMap<(String, String), Vec<String>>,
    /// (package dir, name) → Function|Method node ids (Go Rule 1).
    callable_in_module: BTreeMap<(String, String), Vec<String>>,
    /// method name → (package dir, Method node id) (Go Rule 3 / TS Rule 2).
    methods_by_name: BTreeMap<String, Vec<(String, String)>>,
    /// (file relpath, function name) → Function node ids (TS Rule 1a/1b).
    functions_by_file: BTreeMap<(String, String), Vec<String>>,
}

/// Resolve every file's raw [`CallRef`]s into heuristic `calls` edges, merging
/// repeated source→target calls into one edge with multiple sites. Additive: it
/// only inserts `EdgeKind::Calls` edges and never touches existing ones.
fn resolve_calls(
    files: &[ExtractedFile],
    packages: &BTreeMap<String, String>,
    module_path: &str,
    edges: &mut BTreeMap<String, Edge>,
) {
    let index = build_call_index(files);
    let file_ids: BTreeSet<&str> = files.iter().map(|f| f.relpath.as_str()).collect();

    // (source id, target id) → call sites.
    let mut call_sites: BTreeMap<(String, String), Vec<CallSite>> = BTreeMap::new();
    for f in files {
        // This file's intra-repo imports, keyed by package name (Go only).
        let mut go_imported: BTreeMap<String, String> = BTreeMap::new();
        if f.language == "go" {
            for raw in &f.extraction.imports {
                if let Some(dir) = resolve_import(raw, module_path) {
                    if let Some(pkg) = packages.get(&dir) {
                        go_imported.insert(pkg.clone(), dir);
                    }
                }
            }
        }
        for call in &f.extraction.calls {
            // Resolution is language-specific; each branch only matches repo nodes.
            let targets = match f.language {
                "go" => resolve_call(call, &f.dir, &go_imported, &index),
                "ts" => resolve_call_ts(
                    call,
                    &f.relpath,
                    &f.dir,
                    &f.extraction.import_bindings,
                    &file_ids,
                    &index,
                ),
                _ => Vec::new(),
            };
            for target in targets {
                call_sites
                    .entry((call.caller_node_id.clone(), target))
                    .or_default()
                    .push(call.site.clone());
            }
        }
    }

    for ((source, target), mut sites) in call_sites {
        sites.sort_by_key(site_key);
        let id = Edge::make_id(&source, EdgeKind::Calls, &target);
        edges.entry(id.clone()).or_insert(Edge {
            id,
            source,
            target,
            kind: EdgeKind::Calls,
            confidence: Confidence::Heuristic,
            sites,
            metadata: Metadata::new(),
        });
    }
}

fn build_call_index(files: &[ExtractedFile]) -> CallIndex {
    let mut index = CallIndex::default();
    for f in files {
        for sym in &f.extraction.symbols {
            let dir = dir_of(&sym.location.file);
            match sym.kind {
                NodeKind::Function => {
                    index
                        .functions_in_pkg
                        .entry((dir.clone(), sym.name.clone()))
                        .or_default()
                        .push(sym.id.clone());
                    index
                        .callable_in_module
                        .entry((dir, sym.name.clone()))
                        .or_default()
                        .push(sym.id.clone());
                    index
                        .functions_by_file
                        .entry((sym.location.file.clone(), sym.name.clone()))
                        .or_default()
                        .push(sym.id.clone());
                }
                NodeKind::Method => {
                    index
                        .methods_by_name
                        .entry(sym.name.clone())
                        .or_default()
                        .push((dir.clone(), sym.id.clone()));
                    index
                        .callable_in_module
                        .entry((dir, sym.name.clone()))
                        .or_default()
                        .push(sym.id.clone());
                }
                _ => {}
            }
        }
    }
    index
}

/// Apply the heuristic resolution rules to one call reference. Returns every
/// matching target id (fan-out for ambiguous method calls); empty = skip.
fn resolve_call(
    call: &CallRef,
    caller_dir: &str,
    imported: &BTreeMap<String, String>,
    index: &CallIndex,
) -> Vec<String> {
    match &call.package_qualifier {
        Some(qualifier) => match imported.get(qualifier) {
            // Rule 1: `pkg.Name(...)` → Function/Method named Name in that module.
            Some(module_dir) => index
                .callable_in_module
                .get(&(module_dir.clone(), call.callee_name.clone()))
                .cloned()
                .unwrap_or_default(),
            // Qualifier isn't an imported package → a receiver var; Rule 3.
            None => method_match(&call.callee_name, caller_dir, index),
        },
        // Rule 3: `<expr>.Name(...)` method call.
        None if call.is_method_call => method_match(&call.callee_name, caller_dir, index),
        // Rule 2: bare `Name(...)` → Function named Name in the same package.
        None => index
            .functions_in_pkg
            .get(&(caller_dir.to_string(), call.callee_name.clone()))
            .cloned()
            .unwrap_or_default(),
    }
}

/// Apply the TypeScript heuristic call rules (ADR 0002). Returns matching target
/// ids (method calls fan out); empty = skip.
fn resolve_call_ts(
    call: &CallRef,
    caller_file: &str,
    caller_dir: &str,
    bindings: &[ImportBinding],
    file_ids: &BTreeSet<&str>,
    index: &CallIndex,
) -> Vec<String> {
    // Rule 2: `obj.method(...)` → match Method nodes by name (no type info).
    if call.is_method_call {
        return method_match(&call.callee_name, caller_dir, index);
    }
    // Rule 1a: bare `foo(...)` → Function named foo in the same file.
    let same_file = index
        .functions_by_file
        .get(&(caller_file.to_string(), call.callee_name.clone()))
        .cloned()
        .unwrap_or_default();
    if !same_file.is_empty() {
        return same_file;
    }
    // Rule 1b: an imported symbol foo → exported Function in the target file.
    for binding in bindings.iter().filter(|b| b.local == call.callee_name) {
        if let Some(target_file) = resolve_ts_import(&binding.specifier, caller_dir, file_ids) {
            let ids = index
                .functions_by_file
                .get(&(target_file, binding.imported.clone()))
                .cloned()
                .unwrap_or_default();
            if !ids.is_empty() {
                return ids;
            }
        }
    }
    Vec::new()
}

/// Rule 3 (Go) / Rule 2 (TS): match a method name, preferring methods in the
/// caller's package; if none are in-package, fan out to every same-named method.
fn method_match(name: &str, caller_dir: &str, index: &CallIndex) -> Vec<String> {
    let Some(candidates) = index.methods_by_name.get(name) else {
        return Vec::new();
    };
    let same_pkg: Vec<String> = candidates
        .iter()
        .filter(|(dir, _)| dir == caller_dir)
        .map(|(_, id)| id.clone())
        .collect();
    if same_pkg.is_empty() {
        candidates.iter().map(|(_, id)| id.clone()).collect()
    } else {
        same_pkg
    }
}

fn site_key(site: &CallSite) -> (String, u32, u32, u32, u32) {
    (
        site.file.clone(),
        site.range.start_line,
        site.range.start_col,
        site.range.end_line,
        site.range.end_col,
    )
}

/// Nearest ancestor directory that is also a package.
fn nearest_ancestor_package(dir: &str, packages: &BTreeMap<String, String>) -> Option<String> {
    let mut current = dir.to_string();
    while let Some(parent) = parent_dir(&current) {
        if packages.contains_key(&parent) {
            return Some(parent);
        }
        current = parent;
    }
    None
}

fn parent_dir(dir: &str) -> Option<String> {
    if dir == "." {
        return None;
    }
    match dir.rfind('/') {
        Some(i) => Some(dir[..i].to_string()),
        None => Some(".".to_string()),
    }
}

fn dir_of(relpath: &str) -> String {
    match relpath.rfind('/') {
        Some(i) => relpath[..i].to_string(),
        None => ".".to_string(),
    }
}

/// A stable, machine-independent repo identifier: the final component of the
/// canonicalized root (e.g. `../../fixtures/go-sample` → `go-sample`). Avoids
/// leaking the caller's relative/absolute path into the IR. Falls back to the
/// cleaned path string when there is no final component (e.g. filesystem root).
fn canonical_repo_path(root: &Path) -> String {
    let absolute = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    absolute
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| absolute.to_string_lossy().to_string())
}

fn rel_to_slash(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

/// Read the `module <path>` directive from `<root>/go.mod`. Returns `None` when
/// there is no `go.mod` (e.g. a TypeScript-only repo) — Go import resolution is
/// simply skipped in that case. An existing `go.mod` without a `module` line is
/// still an error.
fn read_module_path(root: &Path) -> Result<Option<String>, ExtractError> {
    let go_mod: PathBuf = root.join("go.mod");
    if !go_mod.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&go_mod).map_err(|e| ExtractError::Io {
        path: go_mod.clone(),
        source: e,
    })?;
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix("module ") {
            if let Some(path) = rest.split_whitespace().next() {
                return Ok(Some(path.to_string()));
            }
        }
    }
    Err(ExtractError::GoModNoModule(go_mod))
}
