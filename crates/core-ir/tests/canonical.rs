//! Serialization-determinism tests for the IR document. These exercise the
//! public API only, through `core_ir::*`.

use core_ir::{
    content_hash, CallSite, Confidence, Edge, EdgeKind, IrDocument, Location, Metadata, Node,
    NodeKind, Range, RootInfo, SCHEMA_VERSION,
};
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
