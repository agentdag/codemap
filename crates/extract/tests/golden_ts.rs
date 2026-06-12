//! Golden test for the TypeScript structural extractor.
//!
//! Mirrors `golden.rs` for the `ts-sample` fixture. `analyzedAt` is normalized
//! before comparing (see `golden.rs` for the rationale).

use std::path::Path;

use core_ir::{Confidence, EdgeKind, IrDocument};
use extract::extract_repo;

const FIXTURE: &str = "../../fixtures/ts-sample";
const GOLDEN: &str = "../../fixtures/ts-sample.expected.json";
const NORMALIZED_AT: &str = "1970-01-01T00:00:00Z";

fn canonical_normalized(mut doc: IrDocument) -> String {
    doc.root.analyzed_at = NORMALIZED_AT.to_string();
    doc.to_canonical_json()
}

#[test]
fn ts_sample_matches_golden() {
    let doc = extract_repo(Path::new(FIXTURE)).expect("extract_repo");
    let actual = canonical_normalized(doc);

    let golden = Path::new(GOLDEN);
    if std::env::var_os("UPDATE_EXPECT").is_some() {
        let value: serde_json::Value = serde_json::from_str(&actual).expect("valid json");
        let pretty = serde_json::to_string_pretty(&value).expect("pretty");
        std::fs::write(golden, format!("{pretty}\n")).expect("write golden");
        return;
    }

    let expected_raw = std::fs::read_to_string(golden)
        .expect("missing golden file; run with UPDATE_EXPECT=1 to create it");
    let expected_doc = IrDocument::from_json(&expected_raw).expect("golden parses as IrDocument");
    let expected = canonical_normalized(expected_doc);

    assert_eq!(
        actual, expected,
        "extracted TS IR differs from golden; re-run with UPDATE_EXPECT=1 if intended"
    );
}

#[test]
fn ts_has_lexical_class_method_and_file_imports() {
    let doc = extract_repo(Path::new(FIXTURE)).expect("extract_repo");

    // Class→Method containment, emitted lexically by the extractor (resolved).
    let class_method = doc.edges.iter().any(|e| {
        e.kind == EdgeKind::Contains
            && e.source.ends_with("#User")
            && e.target.contains("#User.")
            && e.confidence == Confidence::Resolved
    });
    assert!(class_method, "expected resolved Class→Method contains edge");

    // File→File import from a resolved relative specifier.
    let file_import = doc.edges.iter().any(|e| {
        e.kind == EdgeKind::Imports
            && e.source.starts_with("file:")
            && e.target.starts_with("file:")
            && e.confidence == Confidence::Resolved
    });
    assert!(file_import, "expected resolved File→File import edge");

    // No calls this stage.
    assert!(
        doc.edges.iter().all(|e| e.kind != EdgeKind::Calls),
        "no calls edges expected for TypeScript this stage"
    );
}

#[test]
fn ts_extraction_is_deterministic() {
    let a = canonical_normalized(extract_repo(Path::new(FIXTURE)).expect("first"));
    let b = canonical_normalized(extract_repo(Path::new(FIXTURE)).expect("second"));
    assert_eq!(a, b, "normalized TS extract_repo must be byte-identical across runs");
}
