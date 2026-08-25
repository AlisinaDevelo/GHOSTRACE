use std::fs;

use ghostrace::SelectedRoot;
use tempfile::tempdir;

#[test]
fn composed_and_decomposed_unicode_follow_filesystem_identity() {
    let directory = tempdir().expect("fixture root");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");

    let composed = root.path().join("caf\u{e9}");
    let decomposed = root.path().join("cafe\u{301}");
    fs::write(&composed, b"composed").expect("composed fixture");
    let _ = fs::write(&decomposed, b"decomposed");

    let composed_canonical = fs::canonicalize(&composed).expect("composed canonical path");
    let decomposed_canonical = fs::canonicalize(&decomposed).expect("decomposed canonical path");
    assert!(root.contains_path(&composed));
    assert!(root.contains_path(&decomposed));

    let composed_digest = root.path_digest(&composed).expect("composed digest");
    let decomposed_digest = root.path_digest(&decomposed).expect("decomposed digest");
    if composed_canonical == decomposed_canonical {
        assert_eq!(composed_digest, decomposed_digest);
    } else {
        assert_ne!(composed_digest, decomposed_digest);
    }
}

#[test]
fn case_only_rename_preserves_containment_and_declares_digest_scope() {
    let directory = tempdir().expect("fixture root");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");

    let before = root.path().join("CaseOnly");
    let after = root.path().join("caseonly");
    fs::write(&before, b"fixture").expect("case fixture");
    let before_canonical = fs::canonicalize(&before).expect("before canonical path");
    let before_digest = root.path_digest(&before).expect("before digest");
    fs::rename(&before, &after).expect("case-only rename");

    let after_canonical = fs::canonicalize(&after).expect("after canonical path");
    let after_digest = root.path_digest(&after).expect("after digest");
    assert!(root.contains_path(&after));
    if before_canonical == after_canonical {
        assert_eq!(before_digest, after_digest);
    } else {
        assert_ne!(before_digest, after_digest);
    }
}
