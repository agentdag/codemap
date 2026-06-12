//! `GoExtractor`: tree-sitter-go → per-file [`FileExtraction`]. Syntactic only.

use core_ir::{content_hash, CallSite, Location, Metadata, Node, NodeKind, Range};
use tree_sitter::{Language, Node as TsNode, Parser};

use crate::error::ExtractError;
use crate::extractor::{CallRef, FileExtraction, LanguageExtractor};

const LANGUAGE: &str = "go";

/// Structural extractor for Go source files.
#[derive(Debug, Default, Clone, Copy)]
pub struct GoExtractor;

impl GoExtractor {
    pub fn new() -> Self {
        GoExtractor
    }
}

impl LanguageExtractor for GoExtractor {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn extract_file(&self, relpath: &str, source: &str) -> Result<FileExtraction, ExtractError> {
        let mut parser = Parser::new();
        let language: Language = tree_sitter_go::LANGUAGE.into();
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

        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut calls = Vec::new();
        let mut package_name = String::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "package_clause" => {
                    if let Some(id) = named_child_of_kind(child, "package_identifier") {
                        package_name = text(id, src).to_string();
                    }
                }
                "import_declaration" => collect_import_paths(child, src, &mut imports),
                "function_declaration" => {
                    if let Some(name) = field_text(child, "name", src) {
                        let node =
                            symbol_node(NodeKind::Function, &name, &name, relpath, child, src);
                        collect_call_refs(child, &node.id, relpath, src, &mut calls);
                        symbols.push(node);
                    }
                }
                "method_declaration" => {
                    if let (Some(name), Some(recv)) =
                        (field_text(child, "name", src), receiver_type(child, src))
                    {
                        let qn = format!("{recv}.{name}");
                        let mut node = symbol_node(NodeKind::Method, &name, &qn, relpath, child, src);
                        node.metadata
                            .insert("receiver".to_string(), serde_json::Value::String(recv));
                        collect_call_refs(child, &node.id, relpath, src, &mut calls);
                        symbols.push(node);
                    }
                }
                "type_declaration" => collect_type_specs(child, relpath, src, &mut symbols),
                _ => {}
            }
        }

        Ok(FileExtraction {
            file_node,
            symbols,
            // Go method→class linking happens in the linker (by receiver), not here.
            edges: Vec::new(),
            imports,
            package_name,
            calls,
        })
    }
}

/// Build a symbol `Node` whose hash/location come from `node`'s byte span.
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

/// Classify each `type_spec` / `type_alias` under a `type_declaration`.
fn collect_type_specs(decl: TsNode, relpath: &str, src: &[u8], out: &mut Vec<Node>) {
    let mut cursor = decl.walk();
    for spec in decl.named_children(&mut cursor) {
        let (kind, name) = match spec.kind() {
            "type_alias" => (NodeKind::TypeAlias, field_text(spec, "name", src)),
            "type_spec" => {
                let name = field_text(spec, "name", src);
                let kind = match spec.child_by_field_name("type").map(|t| t.kind()) {
                    Some("struct_type") => NodeKind::Class,
                    Some("interface_type") => NodeKind::Interface,
                    _ => NodeKind::TypeAlias,
                };
                (kind, name)
            }
            _ => continue,
        };
        if let Some(name) = name {
            out.push(symbol_node(kind, &name, &name, relpath, spec, src));
        }
    }
}

/// Collect every `call_expression` under `node`, attributing each to `caller_id`.
/// Recurses through everything (including nested calls and func literals), so
/// `fmt.Println(g.Hello())` yields both calls — all credited to the enclosing
/// top-level function/method (no closure nodes exist this stage).
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

/// Build a [`CallRef`] from a `call_expression`, or `None` when the callee shape
/// is unsupported (e.g. generic instantiation, immediately-invoked literal).
fn call_ref_from(call: TsNode, caller_id: &str, relpath: &str, src: &[u8]) -> Option<CallRef> {
    let function = call.child_by_field_name("function")?;
    let (callee_name, package_qualifier, is_method_call) = match function.kind() {
        // Bare `Name(...)`.
        "identifier" => (text(function, src).to_string(), None, false),
        // `X.Name(...)`.
        "selector_expression" => {
            let name = text(function.child_by_field_name("field")?, src).to_string();
            match function.child_by_field_name("operand") {
                // `ident.Name(...)` — ambiguous package-vs-receiver; resolver decides.
                Some(op) if op.kind() == "identifier" => {
                    (name, Some(text(op, src).to_string()), false)
                }
                // `<expr>.Name(...)` — definitely a method call, receiver unknown.
                _ => (name, None, true),
            }
        }
        _ => return None,
    };
    Some(CallRef {
        caller_node_id: caller_id.to_string(),
        callee_name,
        package_qualifier,
        is_method_call,
        site: CallSite {
            file: relpath.to_string(),
            range: node_range(call),
        },
    })
}

/// The receiver base type of a method, with any leading `*` removed (e.g.
/// `*Greeter` → `Greeter`, `Box[T]` → `Box`). `None` if it can't be determined.
fn receiver_type(method: TsNode, src: &[u8]) -> Option<String> {
    let recv = method.child_by_field_name("receiver")?;
    let mut cursor = recv.walk();
    for param in recv.named_children(&mut cursor) {
        if param.kind() == "parameter_declaration" {
            if let Some(ty) = param.child_by_field_name("type") {
                return base_type_name(ty, src);
            }
        }
    }
    None
}

/// Descend `pointer_type` / `generic_type` / qualified wrappers to the base
/// `type_identifier`.
fn base_type_name(node: TsNode, src: &[u8]) -> Option<String> {
    if node.kind() == "type_identifier" {
        return Some(text(node, src).to_string());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(name) = base_type_name(child, src) {
            return Some(name);
        }
    }
    None
}

/// Collect every import path under an `import_declaration` (single or grouped),
/// stripping the surrounding quotes.
fn collect_import_paths(decl: TsNode, src: &[u8], out: &mut Vec<String>) {
    let mut specs = Vec::new();
    collect_descendants_of_kind(decl, "import_spec", &mut specs);
    for spec in specs {
        if let Some(path) = spec.child_by_field_name("path") {
            out.push(text(path, src).trim_matches('"').to_string());
        }
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
