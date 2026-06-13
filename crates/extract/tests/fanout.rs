//! Part A: heuristic call-graph fan-out cap (`MAX_CALL_FANOUT`).
//!
//! A method name matching more candidates than the cap (at its tightest scope)
//! is too ambiguous and emits no edge; a uniquely-named call emits exactly one.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use core_ir::EdgeKind;
use extract::extract_repo;

fn temp_repo(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("codemap-fanout-{tag}-{nanos}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn over_cap_method_is_ambiguous_and_unique_resolves() {
    let dir = temp_repo("ts");
    // Five `execute` methods + one uniquely-named method, all in the same file as
    // the caller, so they share the tightest (same-file) scope.
    fs::write(
        dir.join("a.ts"),
        r#"
class C1 { execute(): void {} }
class C2 { execute(): void {} }
class C3 { execute(): void {} }
class C4 { execute(): void {} }
class C5 { execute(): void {} }
class Uniq { runOnce(): void {} }

export function run(x: any): void {
  x.execute();
  x.runOnce();
}
"#,
    )
    .unwrap();

    let doc = extract_repo(&dir).expect("extract_repo");
    let calls: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::Calls).collect();

    // `x.execute()` matches 5 methods (> cap of 3) → ZERO edges.
    let execute_edges = calls.iter().filter(|e| e.target.ends_with(".execute")).count();
    assert_eq!(execute_edges, 0, "over-cap `execute` must emit no edge: {calls:?}");

    // `x.runOnce()` is unique → exactly ONE heuristic edge run → Uniq.runOnce.
    let unique: Vec<_> = calls.iter().filter(|e| e.target.ends_with("#Uniq.runOnce")).collect();
    assert_eq!(unique.len(), 1, "unique call must emit one edge: {calls:?}");
    assert!(unique[0].source.ends_with("#run"));
    assert_eq!(unique[0].confidence, core_ir::Confidence::Heuristic);

    // Only the unique call survives.
    assert_eq!(calls.len(), 1, "expected exactly one calls edge: {calls:?}");

    fs::remove_dir_all(&dir).ok();
}
