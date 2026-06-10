//! Golden test for the Go structural extractor.
//!
//! `extract_repo` → `to_canonical_json` is compared byte-for-byte against
//! `fixtures/go-sample.expected.json`. Set `UPDATE_EXPECT=1` to (re)write it.

use std::path::Path;

use extract::extract_repo;

#[test]
fn go_sample_matches_golden() {
    // cargo runs tests with the crate root as the working dir; the fixture lives
    // at the workspace root. A fixed relative path keeps `root.repoPath` (and the
    // golden) machine-independent.
    let root = Path::new("../../fixtures/go-sample");
    let doc = extract_repo(root).expect("extract_repo");
    let actual = doc.to_canonical_json();

    let golden = Path::new("../../fixtures/go-sample.expected.json");

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
    // comparing, so formatting of the file on disk doesn't matter.
    let expected_doc =
        core_ir::IrDocument::from_json(&expected_raw).expect("golden parses as IrDocument");
    let expected = expected_doc.to_canonical_json();

    assert_eq!(
        actual, expected,
        "extracted IR differs from golden; re-run with UPDATE_EXPECT=1 if intended"
    );
}

#[test]
fn extraction_is_deterministic() {
    let root = Path::new("../../fixtures/go-sample");
    let a = extract_repo(root).expect("first").to_canonical_json();
    let b = extract_repo(root).expect("second").to_canonical_json();
    assert_eq!(a, b, "extract_repo must be byte-identical across runs");
}
