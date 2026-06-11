//! Repository-level extraction: walk the tree, dispatch files to language
//! extractors, and do all cross-file linking. Containment and import edges are
//! `Confidence::Resolved`; the heuristic call graph is `Confidence::Heuristic`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use core_ir::{
    CallSite, Confidence, Edge, EdgeKind, IrDocument, Location, Metadata, Node, NodeKind, RootInfo,
    Range, SCHEMA_VERSION,
};
use ignore::WalkBuilder;

use crate::error::ExtractError;
use crate::extractor::{CallRef, FileExtraction, LanguageExtractor};
use crate::go::GoExtractor;

const TOOL_VERSION: &str = "0.1.0";

/// One extracted file plus its directory (package) relpath.
struct ExtractedFile {
    relpath: String,
    dir: String,
    extraction: FileExtraction,
}

/// Extract a full repository into a canonical [`IrDocument`].
pub fn extract_repo(root: &Path) -> Result<IrDocument, ExtractError> {
    let module_path = read_module_path(root)?;
    let extractors: Vec<Box<dyn LanguageExtractor>> = vec![Box::new(GoExtractor::new())];

    let files = walk_and_extract(root, &extractors)?;

    // Directory (package) → chosen package name. BTreeMap keeps it deterministic.
    let packages = collect_packages(&files);

    // Dedup nodes/edges by id; canonical serialization re-sorts regardless.
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut edges: BTreeMap<String, Edge> = BTreeMap::new();

    // Module nodes.
    for (dir, pkg) in &packages {
        let node = module_node(dir, pkg);
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

    // File and symbol nodes + Module→File and File→symbol containment.
    for f in &files {
        nodes.insert(f.extraction.file_node.id.clone(), f.extraction.file_node.clone());
        push_contains(&mut edges, &module_id(&f.dir), &f.extraction.file_node.id);

        for sym in &f.extraction.symbols {
            nodes.insert(sym.id.clone(), sym.clone());
            if sym.kind == NodeKind::Class {
                classes.insert((f.dir.clone(), sym.qualified_name.clone()), sym.id.clone());
            }
            // Methods are linked to their class (or filed) in a later pass.
            if sym.kind != NodeKind::Method {
                push_contains(&mut edges, &f.extraction.file_node.id, &sym.id);
            }
        }
    }

    // Method containment: Class→Method when the receiver type resolves in-package,
    // else File→Method with a note.
    for f in &files {
        for sym in &f.extraction.symbols {
            if sym.kind != NodeKind::Method {
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

    // Import resolution: File → Module for intra-repo packages only.
    for f in &files {
        for raw in &f.extraction.imports {
            if let Some(target_dir) = resolve_import(raw, &module_path) {
                if packages.contains_key(&target_dir) {
                    push_edge(
                        &mut edges,
                        &f.extraction.file_node.id,
                        EdgeKind::Imports,
                        &module_id(&target_dir),
                        Metadata::new(),
                    );
                }
            }
        }
    }

    resolve_calls(&files, &packages, &module_path, &mut edges);

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
        let extraction = extractor.extract_file(&relpath, &source)?;
        out.push(ExtractedFile {
            dir: dir_of(&relpath),
            relpath,
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

fn module_node(dir: &str, package: &str) -> Node {
    Node {
        id: module_id(dir),
        kind: NodeKind::Module,
        name: package.to_string(),
        qualified_name: String::new(),
        language: "go".to_string(),
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
    /// (package dir, function name) → Function node ids (Rule 2).
    functions_in_pkg: BTreeMap<(String, String), Vec<String>>,
    /// (package dir, name) → Function|Method node ids (Rule 1).
    callable_in_module: BTreeMap<(String, String), Vec<String>>,
    /// method name → (package dir, Method node id) (Rule 3).
    methods_by_name: BTreeMap<String, Vec<(String, String)>>,
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

    // (source id, target id) → call sites.
    let mut call_sites: BTreeMap<(String, String), Vec<CallSite>> = BTreeMap::new();
    for f in files {
        // This file's intra-repo imports, keyed by the imported package's name.
        let mut imported: BTreeMap<String, String> = BTreeMap::new();
        for raw in &f.extraction.imports {
            if let Some(dir) = resolve_import(raw, module_path) {
                if let Some(pkg) = packages.get(&dir) {
                    imported.insert(pkg.clone(), dir);
                }
            }
        }
        for call in &f.extraction.calls {
            for target in resolve_call(call, &f.dir, &imported, &index) {
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

/// Rule 3: match a method name, preferring methods in the caller's package; if
/// none are in-package, fan out to every same-named method across the repo.
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

/// Read the `module <path>` directive from `<root>/go.mod`.
fn read_module_path(root: &Path) -> Result<String, ExtractError> {
    let go_mod: PathBuf = root.join("go.mod");
    if !go_mod.exists() {
        return Err(ExtractError::GoModMissing(go_mod));
    }
    let content = std::fs::read_to_string(&go_mod).map_err(|e| ExtractError::Io {
        path: go_mod.clone(),
        source: e,
    })?;
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix("module ") {
            if let Some(path) = rest.split_whitespace().next() {
                return Ok(path.to_string());
            }
        }
    }
    Err(ExtractError::GoModNoModule(go_mod))
}
