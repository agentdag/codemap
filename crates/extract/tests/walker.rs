//! Part B: the walker always prunes heavy/generated directories (e.g.
//! `node_modules`) even when they aren't gitignored.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use extract::extract_repo;

fn temp_repo(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("codemap-walker-{tag}-{nanos}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn node_modules_is_skipped() {
    let dir = temp_repo("nm");
    fs::create_dir_all(dir.join("node_modules/dep")).unwrap();
    // A dependency file that must NOT be extracted.
    fs::write(dir.join("node_modules/dep/index.ts"), "export function fromDep(): void {}").unwrap();
    // A real source file that must be extracted.
    fs::write(dir.join("app.ts"), "export function appFn(): void {}").unwrap();

    let doc = extract_repo(&dir).expect("extract_repo");

    // Real source is present.
    assert!(
        doc.nodes.iter().any(|n| n.id == "file:app.ts"),
        "expected app.ts to be extracted"
    );
    // Nothing from node_modules made it in.
    assert!(
        !doc.nodes.iter().any(|n| n.location.file.contains("node_modules")),
        "node_modules must be skipped: {:?}",
        doc.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
    assert!(
        !doc.nodes.iter().any(|n| n.name == "fromDep"),
        "dependency symbol must not be extracted"
    );

    fs::remove_dir_all(&dir).ok();
}
