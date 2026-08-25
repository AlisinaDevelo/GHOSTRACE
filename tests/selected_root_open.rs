#[cfg(unix)]
mod unix {
    use std::{fs, os::unix::fs::symlink};

    use ghostrace::SelectedRoot;
    use tempfile::tempdir;

    #[test]
    fn public_open_is_descriptor_backed_and_refuses_link_aliases() {
        let directory = tempdir().expect("fixture root");
        let root_path = directory.path().join("selected");
        fs::create_dir(&root_path).expect("selected root");
        let root = SelectedRoot::new("root-main", &root_path).expect("selected root");

        let outside = directory.path().join("outside.txt");
        fs::write(&outside, b"outside-secret").expect("outside");
        let symlink_path = root.path().join("symlink.txt");
        symlink(&outside, &symlink_path).expect("symlink");
        assert!(root.open_contained(&symlink_path).is_err());

        let hard_link_path = root.path().join("hard-link.txt");
        fs::hard_link(&outside, &hard_link_path).expect("hard link");
        assert!(root.open_contained(&hard_link_path).is_err());

        let regular_path = root.path().join("regular.txt");
        fs::write(&regular_path, b"inside-secret").expect("regular");
        let descriptor = root.open_contained(&regular_path).expect("descriptor");
        assert!(descriptor.metadata().expect("descriptor metadata").is_file());
        assert!(descriptor.identity_is_stable().expect("descriptor identity"));
    }

    #[test]
    fn public_open_rejects_parent_and_lexical_escape_paths() {
        let directory = tempdir().expect("fixture root");
        let root_path = directory.path().join("selected");
        fs::create_dir(&root_path).expect("selected root");
        let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
        let sibling = directory.path().join("selected-sibling");
        fs::create_dir(&sibling).expect("sibling");
        fs::write(sibling.join("outside.txt"), b"outside-secret").expect("outside");

        assert!(root.open_contained(&sibling.join("outside.txt")).is_err());
        assert!(root
            .open_contained(&root.path().join("missing").join("..").join("outside.txt"))
            .is_err());
    }
}

#[cfg(not(unix))]
#[test]
fn descriptor_open_is_an_explicit_no_go_without_unix_no_follow_support() {
    let directory = tempfile::tempdir().expect("fixture root");
    let root = ghostrace::SelectedRoot::new("root-main", directory.path()).expect("root");
    assert!(root.open_contained(&root.path().join("missing")).is_err());
}
