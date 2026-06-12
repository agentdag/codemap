//! `TsExtractor`: tree-sitter-typescript → per-file [`FileExtraction`].
//! Structure + imports + raw call refs. See ADR 0002 for the mapping, the call
//! resolution rules, and the deferred constructs.
//!
//! This exists to exercise the [`LanguageExtractor`] boundary against a language
//! shaped differently from Go: methods are *lexically* nested, so `Class`→`Method`
//! containment is emitted here (via [`FileExtraction::edges`]) rather than being
//! resolved by a receiver pass in the linker. Calls are collected unresolved and
//! resolved cross-file in [`crate::extract_repo`].

use core_ir::{
    content_hash, CallSite, Confidence, Edge, EdgeKind, Location, Metadata, Node, NodeKind, Range,
};
use tree_sitter::{Language, Node as TsNode, Parser};

use crate::error::ExtractError;
use crate::extractor::{CallRef, FileExtraction, ImportBinding, LanguageExtractor};

const LANGUAGE: &str = "ts";

/// Structural extractor for TypeScript (`.ts` / `.tsx`).
#[derive(Debug, Default, Clone, Copy)]
pub struct TsExtractor;

impl TsExtractor {
    pub fn new() -> Self {
        TsExtractor
    }
}

impl LanguageExtractor for TsExtractor {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx"]
    }

    fn extract_file(&self, relpath: &str, source: &str) -> Result<FileExtraction, ExtractError> {
        let mut parser = Parser::new();
        let language: Language = if relpath.ends_with(".tsx") {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        };
        parser
            .set_language(&language)
            .map_err(|e| ExtractError::Language(e.to_string()))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ExtractError::Parse {
                relpath: relpath.to_string(),
            })?;

        let src = source.as_bytes();
        let root = tree.root_node();

        let file_node = Node {
            id: Node::make_id(NodeKind::File, LANGUAGE, relpath, ""),
            kind: NodeKind::File,
            name: basename(relpath).to_string(),
            qualified_name: String::new(),
            language: LANGUAGE.to_string(),
            location: Location {
                file: relpath.to_string(),
                range: node_range(root),
            },
            content_hash: content_hash(source),
            metadata: Metadata::new(),
        };

        let mut out = Collected::default();
        let mut imports = Vec::new();
        let mut import_bindings = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" => {
                    if let Some(spec) = import_specifier(child, src) {
                        collect_import_bindings(child, &spec, src, &mut import_bindings);
                        imports.push(spec);
                    }
                }
                // Exported declarations nest under `export_statement`.
                "export_statement" => {
                    if let Some(decl) = child.child_by_field_name("declaration") {
                        handle_declaration(decl, relpath, src, &mut out);
                    }
                }
                _ => handle_declaration(child, relpath, src, &mut out),
            }
        }

        Ok(FileExtraction {
            file_node,
            symbols: out.symbols,
            edges: out.edges,
            imports,
            import_bindings,
            package_name: module_name_for(relpath),
            calls: out.calls,
        })
    }
}

/// Accumulator for a file's extracted symbols, intra-file edges, and call refs.
#[derive(Default)]
struct Collected {
    symbols: Vec<Node>,
    edges: Vec<Edge>,
    calls: Vec<CallRef>,
}

/// Process one top-level declaration node (no-op for unsupported kinds).
fn handle_declaration(node: TsNode, relpath: &str, src: &[u8], out: &mut Collected) {
    match node.kind() {
        "function_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                let built = symbol_node(NodeKind::Function, &name, &name, relpath, node, src);
                collect_call_refs(node, &built.id, relpath, src, &mut out.calls);
                out.symbols.push(built);
            }
        }
        "class_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                let class = symbol_node(NodeKind::Class, &name, &name, relpath, node, src);
                collect_methods(node, &name, &class.id, relpath, src, out);
                out.symbols.push(class);
            }
        }
        "interface_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                out.symbols
                    .push(symbol_node(NodeKind::Interface, &name, &name, relpath, node, src));
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                out.symbols
                    .push(symbol_node(NodeKind::TypeAlias, &name, &name, relpath, node, src));
            }
        }
        _ => {}
    }
}

/// Emit `Method` nodes for a class body plus a resolved `Class`→`Method` edge,
/// and collect each method body's call refs.
fn collect_methods(
    class: TsNode,
    class_name: &str,
    class_id: &str,
    relpath: &str,
    src: &[u8],
    out: &mut Collected,
) {
    let Some(body) = class.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        if member.kind() != "method_definition" {
            continue;
        }
        let Some(name_node) = member.child_by_field_name("name") else {
            continue;
        };
        // Defer computed/non-identifier method names.
        if name_node.kind() != "property_identifier" {
            continue;
        }
        let method = text(name_node, src);
        let qn = format!("{class_name}.{method}");
        let node = symbol_node(NodeKind::Method, method, &qn, relpath, member, src);
        out.edges.push(Edge {
            id: Edge::make_id(class_id, EdgeKind::Contains, &node.id),
            source: class_id.to_string(),
            target: node.id.clone(),
            kind: EdgeKind::Contains,
            confidence: Confidence::Resolved,
            sites: Vec::new(),
            metadata: Metadata::new(),
        });
        collect_call_refs(member, &node.id, relpath, src, &mut out.calls);
        out.symbols.push(node);
    }
}

/// Collect every `call_expression` under `node`, attributed to `caller_id`.
/// Recurses through everything (including nested calls), so
/// `console.log(u.greet())` yields both the outer and inner calls.
fn collect_call_refs(
    node: TsNode,
    caller_id: &str,
    relpath: &str,
    src: &[u8],
    out: &mut Vec<CallRef>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "call_expression" {
            if let Some(call_ref) = call_ref_from(child, caller_id, relpath, src) {
                out.push(call_ref);
            }
        }
        collect_call_refs(child, caller_id, relpath, src, out);
    }
}

/// Build a [`CallRef`] from a `call_expression`, or `None` for unsupported callee
/// shapes (constructors are `new_expression`, not calls, so handled by omission).
fn call_ref_from(call: TsNode, caller_id: &str, relpath: &str, src: &[u8]) -> Option<CallRef> {
    let function = call.child_by_field_name("function")?;
    let (callee_name, qualifier, is_method_call) = match function.kind() {
        // Bare `foo(...)`.
        "identifier" => (text(function, src).to_string(), None, false),
        // `obj.method(...)` — always a member/method call (no Go-style packages).
        "member_expression" => {
            let property = function.child_by_field_name("property")?;
            if property.kind() != "property_identifier" {
                return None; // computed member access, deferred
            }
            let object = function.child_by_field_name("object");
            let qualifier = object
                .filter(|o| o.kind() == "identifier")
                .map(|o| text(o, src).to_string());
            (text(property, src).to_string(), qualifier, true)
        }
        _ => return None,
    };
    Some(CallRef {
        caller_node_id: caller_id.to_string(),
        callee_name,
        package_qualifier: qualifier,
        is_method_call,
        site: CallSite {
            file: relpath.to_string(),
            range: node_range(call),
        },
    })
}

/// Collect named import bindings from one `import_statement` for `specifier`.
/// Named imports only (`import { a, b as c }`); default/namespace imports are
/// deferred (see ADR 0002).
fn collect_import_bindings(
    node: TsNode,
    specifier: &str,
    src: &[u8],
    out: &mut Vec<ImportBinding>,
) {
    let mut specs = Vec::new();
    collect_descendants_of_kind(node, "import_specifier", &mut specs);
    for spec in specs {
        let Some(name_node) = spec.child_by_field_name("name") else {
            continue;
        };
        let imported = text(name_node, src).to_string();
        // `import { a as b }` → local is the alias; otherwise local == imported.
        let local = spec
            .child_by_field_name("alias")
            .map(|a| text(a, src).to_string())
            .unwrap_or_else(|| imported.clone());
        out.push(ImportBinding {
            local,
            imported,
            specifier: specifier.to_string(),
        });
    }
}

fn collect_descendants_of_kind<'a>(node: TsNode<'a>, kind: &str, out: &mut Vec<TsNode<'a>>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == kind {
            out.push(child);
        } else {
            collect_descendants_of_kind(child, kind, out);
        }
    }
}

fn symbol_node(
    kind: NodeKind,
    name: &str,
    qualified_name: &str,
    relpath: &str,
    node: TsNode,
    src: &[u8],
) -> Node {
    let slice = std::str::from_utf8(&src[node.byte_range()]).unwrap_or("");
    Node {
        id: Node::make_id(kind, LANGUAGE, relpath, qualified_name),
        kind,
        name: name.to_string(),
        qualified_name: qualified_name.to_string(),
        language: LANGUAGE.to_string(),
        location: Location {
            file: relpath.to_string(),
            range: node_range(node),
        },
        content_hash: content_hash(slice),
        metadata: Metadata::new(),
    }
}

/// The string specifier of an `import_statement` (quotes stripped), if present.
fn import_specifier(node: TsNode, src: &[u8]) -> Option<String> {
    let string = node
        .child_by_field_name("source")
        .or_else(|| named_child_of_kind(node, "string"))?;
    Some(
        text(string, src)
            .trim_matches(|c| c == '\'' || c == '"' || c == '`')
            .to_string(),
    )
}

/// Module display name for a file's directory: the directory's final component
/// (the repo root directory is `.`).
fn module_name_for(relpath: &str) -> String {
    let dir = match relpath.rfind('/') {
        Some(i) => &relpath[..i],
        None => ".",
    };
    dir.rsplit('/').next().unwrap_or(dir).to_string()
}

fn named_child_of_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|c| c.kind() == kind);
    found
}

fn field_text(node: TsNode, field: &str, src: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| text(n, src).to_string())
}

fn text<'a>(node: TsNode, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

fn node_range(node: TsNode) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start_line: start.row as u32,
        start_col: start.column as u32,
        end_line: end.row as u32,
        end_col: end.column as u32,
    }
}

fn basename(relpath: &str) -> &str {
    relpath.rsplit('/').next().unwrap_or(relpath)
}
