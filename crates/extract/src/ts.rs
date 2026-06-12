//! `TsExtractor`: tree-sitter-typescript → per-file [`FileExtraction`].
//! Structure + imports only (no calls this stage). See ADR 0002 for the mapping
//! and the deferred constructs.
//!
//! This exists to exercise the [`LanguageExtractor`] boundary against a language
//! shaped differently from Go: methods are *lexically* nested, so `Class`→`Method`
//! containment is emitted here (via [`FileExtraction::edges`]) rather than being
//! resolved by a receiver pass in the linker.

use core_ir::{content_hash, Confidence, Edge, EdgeKind, Location, Metadata, Node, NodeKind, Range};
use tree_sitter::{Language, Node as TsNode, Parser};

use crate::error::ExtractError;
use crate::extractor::{FileExtraction, LanguageExtractor};

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

        let mut symbols = Vec::new();
        let mut edges = Vec::new();
        let mut imports = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" => {
                    if let Some(spec) = import_specifier(child, src) {
                        imports.push(spec);
                    }
                }
                // Exported declarations nest under `export_statement`.
                "export_statement" => {
                    if let Some(decl) = child.child_by_field_name("declaration") {
                        handle_declaration(decl, relpath, src, &mut symbols, &mut edges);
                    }
                }
                _ => handle_declaration(child, relpath, src, &mut symbols, &mut edges),
            }
        }

        Ok(FileExtraction {
            file_node,
            symbols,
            edges,
            imports,
            package_name: module_name_for(relpath),
            calls: Vec::new(),
        })
    }
}

/// Process one top-level declaration node (no-op for unsupported kinds).
fn handle_declaration(
    node: TsNode,
    relpath: &str,
    src: &[u8],
    symbols: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) {
    match node.kind() {
        "function_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                symbols.push(symbol_node(NodeKind::Function, &name, &name, relpath, node, src));
            }
        }
        "class_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                let class = symbol_node(NodeKind::Class, &name, &name, relpath, node, src);
                collect_methods(node, &name, &class.id, relpath, src, symbols, edges);
                symbols.push(class);
            }
        }
        "interface_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                symbols.push(symbol_node(NodeKind::Interface, &name, &name, relpath, node, src));
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = field_text(node, "name", src) {
                symbols.push(symbol_node(NodeKind::TypeAlias, &name, &name, relpath, node, src));
            }
        }
        _ => {}
    }
}

/// Emit `Method` nodes for a class body plus a resolved `Class`→`Method` edge.
fn collect_methods(
    class: TsNode,
    class_name: &str,
    class_id: &str,
    relpath: &str,
    src: &[u8],
    symbols: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
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
        edges.push(Edge {
            id: Edge::make_id(class_id, EdgeKind::Contains, &node.id),
            source: class_id.to_string(),
            target: node.id.clone(),
            kind: EdgeKind::Contains,
            confidence: Confidence::Resolved,
            sites: Vec::new(),
            metadata: Metadata::new(),
        });
        symbols.push(node);
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
