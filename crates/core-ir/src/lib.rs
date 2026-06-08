//! IR data model
//!
//! A language-neutral, directed, typed property graph describing a codebase.
//! This crate is *pure data*: no file I/O, no parsing, no analysis. It is the
//! single source of truth every other crate reads and writes.
//!
//! Determinism is mandatory: the same logical document always serializes to
//! byte-identical JSON via [`IrDocument::to_canonical_json`], regardless of the
//! order nodes/edges were inserted. See `docs/specs/ir.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Schema version this crate implements (see `docs/specs/ir.md`).
pub const SCHEMA_VERSION: &str = "0.1.0";

/// Free-form metadata map. `BTreeMap` keeps keys sorted, which is part of the
/// determinism guarantee. `serde_json::Value` is itself key-sorted by default
/// (serde_json sorts object keys unless the `preserve_order` feature is on).
pub type Metadata = BTreeMap<String, serde_json::Value>;

/// What a node represents. Phase 0; extensible without breaking existing ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Module,
    Function,
    Method,
    Class,
    Interface,
    TypeAlias,
    Variable,
}

/// What a relationship represents. Phase 0; extensible.
///
/// `contains` edges form a strict tree; `imports`/`calls` are the cross-cutting
/// graph on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
}

impl EdgeKind {
    /// Canonical wire string (matches the serde representation). Used to build
    /// edge ids of the form `<source>=[<kind>]=><target>`.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Imports => "imports",
            EdgeKind::Calls => "calls",
        }
    }
}

/// How much to trust an edge.
///
/// CARDINAL RULE: a `calls` edge produced by Phase-0 tree-sitter is
/// [`Confidence::Heuristic`]. Never label a guess `Resolved`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Syntactic/semantic fact (containment, imports; later LSP-resolved calls).
    Resolved,
    /// Inferred by name/scope matching (Phase-0 calls, pre-LSP).
    Heuristic,
    /// Human-authored (from the overlay; never produced by extractors).
    Asserted,
}

/// A half-open-agnostic source span. Lines and columns are extractor-defined
/// (Phase 0 does not mandate 0- vs 1-based; it only round-trips the values).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Where a node lives: the file it belongs to and its span within that file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub range: Range,
}

/// One concrete location backing an edge (e.g. a single call expression).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSite {
    pub file: String,
    pub range: Range,
}

/// A thing in the code: a file, module, or symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Readable logical id (see [`Node::make_id`]).
    pub id: String,
    pub kind: NodeKind,
    /// Short display name.
    pub name: String,
    pub qualified_name: String,
    pub language: String,
    pub location: Location,
    /// blake3 of the node's source slice (file contents for `File` nodes).
    pub content_hash: String,
    pub metadata: Metadata,
}

impl Node {
    /// Build the canonical, readable node id per the spec scheme:
    /// - `File`   -> `file:<relpath>`
    /// - `Module` -> `module:<relpath-or-package>`
    /// - symbol   -> `<lang>:<relpath>#<qualifiedName>`
    ///
    /// `File`/`Module` ignore `qualified_name` (it is empty for them) and
    /// `language`. `language` is required for symbol ids because the spec's
    /// symbol id format embeds `<lang>`.
    pub fn make_id(
        kind: NodeKind,
        language: &str,
        relpath: &str,
        qualified_name: &str,
    ) -> String {
        match kind {
            NodeKind::File => format!("file:{relpath}"),
            NodeKind::Module => format!("module:{relpath}"),
            NodeKind::Function
            | NodeKind::Method
            | NodeKind::Class
            | NodeKind::Interface
            | NodeKind::TypeAlias
            | NodeKind::Variable => {
                format!("{language}:{relpath}#{qualified_name}")
            }
        }
    }
}

/// A relationship between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub sites: Vec<CallSite>,
    pub metadata: Metadata,
}

impl Edge {
    /// Build the canonical edge id: `<sourceId>=[<kind>]=><targetId>`.
    pub fn make_id(source: &str, kind: EdgeKind, target: &str) -> String {
        format!("{source}=[{}]=>{target}", kind.as_str())
    }
}

/// Provenance for a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub repo_path: String,
    /// ISO8601 timestamp of analysis.
    pub analyzed_at: String,
    pub tool_version: String,
}

/// The whole IR document: ground-truth nodes + edges plus provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IrDocument {
    pub schema_version: String,
    pub root: RootInfo,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl IrDocument {
    /// Serialize to canonical JSON: nodes sorted by id, edges sorted by id, and
    /// (via `BTreeMap`/serde_json) all map keys sorted. The same logical
    /// document always produces byte-identical output, independent of the order
    /// nodes/edges were supplied.
    ///
    /// Serialization of this fully-owned, string-keyed model is infallible, so
    /// this returns `String` directly. Use [`IrDocument::try_to_canonical_json`]
    /// if you want the error surfaced instead.
    pub fn to_canonical_json(&self) -> String {
        self.try_to_canonical_json()
            .expect("IrDocument serialization is infallible for this data model")
    }

    /// Fallible form of [`IrDocument::to_canonical_json`].
    pub fn try_to_canonical_json(&self) -> Result<String, serde_json::Error> {
        let mut canonical = self.clone();
        canonical.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        canonical.edges.sort_by(|a, b| a.id.cmp(&b.id));
        serde_json::to_string(&canonical)
    }

    /// Parse a document from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// blake3 content hash of a source string, hex-encoded. Used for incremental
/// change detection (a node's hash covers its source slice; a `File` node's
/// hash covers the whole file contents).
pub fn content_hash(source: &str) -> String {
    blake3::hash(source.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_range() -> Range {
        Range {
            start_line: 1,
            start_col: 0,
            end_line: 10,
            end_col: 1,
        }
    }

    fn sample_node(id: &str, kind: NodeKind) -> Node {
        let mut metadata = Metadata::new();
        // Insert out of sorted order to exercise key sorting.
        metadata.insert("zeta".into(), json!(true));
        metadata.insert("alpha".into(), json!(1));
        Node {
            id: id.into(),
            kind,
            name: "thing".into(),
            qualified_name: "Some.thing".into(),
            language: "ts".into(),
            location: Location {
                file: "src/a.ts".into(),
                range: sample_range(),
            },
            content_hash: content_hash("body"),
            metadata,
        }
    }

    fn sample_edge(source: &str, kind: EdgeKind, target: &str) -> Edge {
        Edge {
            id: Edge::make_id(source, kind, target),
            source: source.into(),
            target: target.into(),
            kind,
            confidence: Confidence::Heuristic,
            sites: vec![CallSite {
                file: "src/a.ts".into(),
                range: sample_range(),
            }],
            metadata: Metadata::new(),
        }
    }

    fn sample_doc() -> IrDocument {
        let n1 = sample_node("file:src/a.ts", NodeKind::File);
        let n2 = sample_node("ts:src/a.ts#Some.thing", NodeKind::Method);
        let n3 = sample_node("ts:src/a.ts#Other", NodeKind::Class);
        let e1 = sample_edge("file:src/a.ts", EdgeKind::Contains, "ts:src/a.ts#Other");
        let e2 = sample_edge(
            "ts:src/a.ts#Some.thing",
            EdgeKind::Calls,
            "ts:src/a.ts#Other",
        );
        IrDocument {
            schema_version: SCHEMA_VERSION.into(),
            root: RootInfo {
                repo_path: "/repo".into(),
                analyzed_at: "2026-06-08T00:00:00Z".into(),
                tool_version: "0.1.0".into(),
            },
            // Deliberately unsorted.
            nodes: vec![n2, n1, n3],
            edges: vec![e2, e1],
        }
    }

    // --- id determinism -----------------------------------------------------

    #[test]
    fn node_id_is_deterministic_and_matches_spec_scheme() {
        assert_eq!(
            Node::make_id(NodeKind::File, "ts", "src/a.ts", ""),
            "file:src/a.ts"
        );
        assert_eq!(
            Node::make_id(NodeKind::Module, "ts", "src/a.ts", ""),
            "module:src/a.ts"
        );
        assert_eq!(
            Node::make_id(NodeKind::Method, "ts", "src/users/user.service.ts", "UserService.create"),
            "ts:src/users/user.service.ts#UserService.create"
        );
        // Same inputs -> same id.
        assert_eq!(
            Node::make_id(NodeKind::Class, "py", "a/b.py", "Foo"),
            Node::make_id(NodeKind::Class, "py", "a/b.py", "Foo"),
        );
    }

    #[test]
    fn edge_id_is_deterministic_and_matches_spec_scheme() {
        let id = Edge::make_id("ts:src/a.ts#A.b", EdgeKind::Calls, "ts:src/a.ts#C");
        assert_eq!(id, "ts:src/a.ts#A.b=[calls]=>ts:src/a.ts#C");
        assert_eq!(
            id,
            Edge::make_id("ts:src/a.ts#A.b", EdgeKind::Calls, "ts:src/a.ts#C")
        );
    }

    // --- round-trip ---------------------------------------------------------

    #[test]
    fn round_trip_through_canonical_json() {
        let doc = sample_doc();
        let json = doc.to_canonical_json();
        let parsed = IrDocument::from_json(&json).expect("parse");
        // Equality must hold regardless of input ordering: compare canonical
        // forms (to_canonical_json sorts), then also the parsed re-serialization.
        assert_eq!(parsed.to_canonical_json(), json);
        // The parsed document, re-canonicalized, equals the original canonicalized.
        let mut original_sorted = doc.clone();
        original_sorted.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        original_sorted.edges.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(parsed, original_sorted);
    }

    // --- canonical determinism ---------------------------------------------

    #[test]
    fn canonical_json_is_byte_identical_across_repeated_serialization() {
        let doc = sample_doc();
        assert_eq!(doc.to_canonical_json(), doc.to_canonical_json());
    }

    #[test]
    fn canonical_json_is_byte_identical_across_shuffled_input() {
        let doc = sample_doc();
        let mut shuffled = doc.clone();
        shuffled.nodes.reverse();
        shuffled.edges.reverse();
        // Also rotate to a different permutation than a plain reverse.
        shuffled.nodes.swap(0, 1);
        assert_eq!(doc.to_canonical_json(), shuffled.to_canonical_json());
    }

    #[test]
    fn canonical_json_sorts_metadata_keys() {
        let doc = sample_doc();
        let json = doc.to_canonical_json();
        let alpha = json.find("\"alpha\"").expect("alpha present");
        let zeta = json.find("\"zeta\"").expect("zeta present");
        assert!(alpha < zeta, "metadata keys must be sorted: {json}");
    }

    // --- enum (de)serialization --------------------------------------------

    #[test]
    fn confidence_serializes_to_spec_strings() {
        assert_eq!(serde_json::to_string(&Confidence::Resolved).unwrap(), "\"resolved\"");
        assert_eq!(serde_json::to_string(&Confidence::Heuristic).unwrap(), "\"heuristic\"");
        assert_eq!(serde_json::to_string(&Confidence::Asserted).unwrap(), "\"asserted\"");

        assert_eq!(
            serde_json::from_str::<Confidence>("\"resolved\"").unwrap(),
            Confidence::Resolved
        );
        assert_eq!(
            serde_json::from_str::<Confidence>("\"heuristic\"").unwrap(),
            Confidence::Heuristic
        );
        assert_eq!(
            serde_json::from_str::<Confidence>("\"asserted\"").unwrap(),
            Confidence::Asserted
        );
    }

    #[test]
    fn edge_and_node_kinds_serialize_to_spec_strings() {
        assert_eq!(serde_json::to_string(&EdgeKind::Contains).unwrap(), "\"contains\"");
        assert_eq!(serde_json::to_string(&EdgeKind::Imports).unwrap(), "\"imports\"");
        assert_eq!(serde_json::to_string(&EdgeKind::Calls).unwrap(), "\"calls\"");
        // EdgeKind::as_str matches the serde wire form (id construction relies on it).
        assert_eq!(EdgeKind::Calls.as_str(), "calls");

        assert_eq!(serde_json::to_string(&NodeKind::File).unwrap(), "\"File\"");
        assert_eq!(serde_json::to_string(&NodeKind::TypeAlias).unwrap(), "\"TypeAlias\"");
        assert_eq!(
            serde_json::from_str::<NodeKind>("\"Interface\"").unwrap(),
            NodeKind::Interface
        );
    }

    #[test]
    fn content_hash_is_blake3_hex_and_stable() {
        let h = content_hash("hello");
        // blake3 hex is 64 chars.
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, content_hash("hello"));
        assert_ne!(h, content_hash("world"));
        // Known blake3 vector for "hello".
        assert_eq!(
            h,
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f"
        );
    }
}
