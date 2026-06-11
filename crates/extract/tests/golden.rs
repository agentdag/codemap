//! Golden test for the Go structural extractor.
//!
//! `extract_repo` → `to_canonical_json` is compared byte-for-byte against
//! `fixtures/go-sample.expected.json`. Set `UPDATE_EXPECT=1` to (re)write it.
//!
//! `root.analyzedAt` is real wall-clock time in normal runs, so the test pins it
//! to a fixed placeholder before comparing — that normalization lives here, not
//! in the extractor, keeping the golden deterministic while production output
//! carries the real timestamp.

use std::path::Path;

use core_ir::IrDocument;
use extract::extract_repo;

const FIXTURE: &str = "../../fixtures/go-sample";
const GOLDEN: &str = "../../fixtures/go-sample.expected.json";
/// Fixed value `analyzedAt` is normalized to for deterministic comparison.
const NORMALIZED_AT: &str = "1970-01-01T00:00:00Z";

/// Canonical JSON with the non-deterministic timestamp pinned to a placeholder.
fn canonical_normalized(mut doc: IrDocument) -> String {
    doc.root.analyzed_at = NORMALIZED_AT.to_string();
    doc.to_canonical_json()
}

#[test]
fn go_sample_matches_golden() {
    let doc = extract_repo(Path::new(FIXTURE)).expect("extract_repo");
    let actual = canonical_normalized(doc);

    let golden = Path::new(GOLDEN);
    if std::env::var_os("UPDATE_EXPECT").is_some() {
        // Pretty-print for a reviewable diff; the test re-canonicalizes on read.
        let value: serde_json::Value = serde_json::from_str(&actual).expect("valid json");
        let pretty = serde_json::to_string_pretty(&value).expect("pretty");
        std::fs::write(golden, format!("{pretty}\n")).expect("write golden");
        return;
    }

    let expected_raw = std::fs::read_to_string(golden)
        .expect("missing golden file; run with UPDATE_EXPECT=1 to create it");
    // Normalize the committed (pretty) golden back to canonical form before
    // comparing, so on-disk formatting (and its timestamp) doesn't matter.
    let expected_doc = IrDocument::from_json(&expected_raw).expect("golden parses as IrDocument");
    let expected = canonical_normalized(expected_doc);

    assert_eq!(
        actual, expected,
        "extracted IR differs from golden; re-run with UPDATE_EXPECT=1 if intended"
    );
}

#[test]
fn extraction_is_deterministic() {
    // Everything except the wall-clock `analyzedAt` must be byte-identical.
    let a = canonical_normalized(extract_repo(Path::new(FIXTURE)).expect("first"));
    let b = canonical_normalized(extract_repo(Path::new(FIXTURE)).expect("second"));
    assert_eq!(a, b, "normalized extract_repo must be byte-identical across runs");
}

#[test]
fn analyzed_at_is_real_time_not_epoch() {
    let doc = extract_repo(Path::new(FIXTURE)).expect("extract_repo");
    let at = &doc.root.analyzed_at;
    assert_ne!(at, NORMALIZED_AT, "analyzedAt must be real time, not the epoch placeholder");
    // RFC3339 UTC shape: `YYYY-MM-DDThh:mm:ssZ`.
    assert_eq!(at.len(), 20, "unexpected timestamp length: {at}");
    assert!(at.ends_with('Z'), "expected UTC 'Z' suffix: {at}");
    assert_eq!(at.as_bytes()[10], b'T', "expected date/time separator 'T': {at}");
}

#[test]
fn repo_path_is_canonical_final_component() {
    let doc = extract_repo(Path::new(FIXTURE)).expect("extract_repo");
    assert_eq!(
        doc.root.repo_path, "go-sample",
        "repoPath should be the stable final path component, not the caller's relative path"
    );
}
