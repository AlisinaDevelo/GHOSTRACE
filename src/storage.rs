//! Secure local artifact and SQLite path handling.
//!
//! The journal path is an authority boundary.  It must not follow symlinks,
//! accept a non-regular file, inherit group/world access, or silently open a
//! file owned by another user.  The checks are deliberately repeated around
//! the open so a path-component replacement becomes a bounded refusal rather
//! than an implicit write to an attacker-selected location.

use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags};

use crate::error::GhostraceError;

const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;
const DATABASE_ARTIFACT_SUFFIXES: &[&str] = &["-wal", "-shm", "-journal", "-tmp", "-backup"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    device: u64,
    inode: u64,
}

/// Open a database only after its containing path and existing sidecars pass
/// the ownership, mode, type, link-count, and no-follow checks.
pub(crate) fn open_database(path: &Path) -> Result<Connection, GhostraceError> {
    let parent = parent_directory(path)?;
    let parent_identity = ensure_private_directory(parent)?;
    verify_database_artifacts(path)?;

    run_test_open_hook(path);

    let rechecked_parent = ensure_private_directory(parent)?;
    if rechecked_parent != parent_identity {
        return Err(GhostraceError::PathRace);
    }

    let file = open_database_file(path)?;
    let file_identity = verify_file_handle(path, &file)?;
    verify_database_artifacts(path)?;

    // The secure file is already present, so SQLite must not create a path if
    // a component changes between this check and the SQLite open.
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    if ensure_private_directory(parent)? != rechecked_parent
        || verify_private_file(path)? != file_identity
    {
        return Err(GhostraceError::PathRace);
    }
    Ok(connection)
}

/// Open the current database as a read-only SQLite connection after the same
/// sidecar and ownership checks used by the writer.
pub(crate) fn open_read_only_database(path: &Path) -> Result<Connection, GhostraceError> {
    verify_database_artifacts(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    add_no_follow(&mut options);
    let file = options.open(path).map_err(|error| secure_open_error(path, error))?;
    verify_file_handle(path, &file)?;
    drop(file);
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    verify_database_artifacts(path)?;
    Ok(connection)
}

/// Ensure the journal's database directory exists and is private to the
/// current user.  Existing path components are never followed when they are
/// symlinks; missing components are created one at a time with mode 0700.
pub(crate) fn ensure_private_directory(path: &Path) -> Result<FileIdentity, GhostraceError> {
    ensure_directory_components(path)?;
    let metadata = symlink_metadata(path)?;
    verify_directory_metadata(path, &metadata, true)?
        .then_some(file_identity(&metadata))
        .ok_or(GhostraceError::UnsafePath)
}

/// Prepare a non-journal artifact parent such as an export directory.  The
/// final directory must be a current-user-owned directory; its mode may be
/// broader than 0700 because the artifact itself is still forced to 0600.
pub(crate) fn ensure_artifact_parent(path: &Path) -> Result<(), GhostraceError> {
    ensure_directory_components(path)?;
    let metadata = symlink_metadata(path)?;
    if !verify_directory_metadata(path, &metadata, false)? {
        return Err(GhostraceError::UnsafePath);
    }
    Ok(())
}

/// Return whether an existing artifact is safe, refusing symlinks, non-regular
/// files, unexpected owners, hard links, and non-private modes.
pub(crate) fn validate_existing_artifact(path: &Path) -> Result<bool, GhostraceError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            verify_private_file(path)?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

/// Validate a regular output that is about to be atomically replaced.  A
/// forced export may repair an old mode, but it must never replace a symlink,
/// non-regular file, foreign-owned file, or hard-linked inode.
pub(crate) fn validate_existing_artifact_for_overwrite(
    path: &Path,
) -> Result<bool, GhostraceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            verify_file_shape(&metadata)?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

/// Force an artifact to mode 0600 and verify its complete metadata contract.
pub(crate) fn set_private_file_permissions(path: &Path) -> Result<(), GhostraceError> {
    set_file_mode(path)?;
    verify_private_file(path).map(|_| ())
}

/// Flush directory metadata after an atomic artifact rename.  A successful
/// rename makes the new name visible; syncing the containing directory also
/// makes that name durable across a power loss on filesystems that support
/// directory fsync (including macOS and other Unix targets).
pub(crate) fn sync_directory(path: &Path) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        File::open(path)
            .map_err(|source| io_error(path, source))?
            .sync_all()
            .map_err(|source| io_error(path, source))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Verify the database and all known SQLite sidecars after migration or a
/// write.  SQLite may create WAL/SHM lazily, so this check is intentionally
/// repeatable.
pub(crate) fn verify_database_artifacts(path: &Path) -> Result<(), GhostraceError> {
    let parent = parent_directory(path)?;
    ensure_private_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(_) => {
            verify_private_file(path)?;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(path, error)),
    }
    for suffix in DATABASE_ARTIFACT_SUFFIXES {
        let sidecar = sidecar_path(path, suffix)?;
        match fs::symlink_metadata(&sidecar) {
            Ok(_) => {
                verify_private_file(&sidecar)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&sidecar, error)),
        }
    }
    Ok(())
}

pub(crate) fn wal_size_bytes(path: &Path) -> Result<u64, GhostraceError> {
    let wal = sidecar_path(path, "-wal")?;
    match fs::symlink_metadata(&wal) {
        Ok(metadata) => {
            verify_file_metadata(&wal, &metadata)?;
            Ok(metadata.len())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(0),
        Err(error) => Err(io_error(&wal, error)),
    }
}

/// Copy only a checkpointed database file.  SQLite WAL and SHM sidecars are
/// never independent backups because they require their matching database and
/// live reader state.
pub(crate) fn copy_database_snapshot(
    source: &Path,
    destination: &Path,
) -> Result<u64, GhostraceError> {
    if is_sidecar_path(destination) {
        return Err(GhostraceError::SidecarBackupRefused);
    }
    if source == destination {
        return Err(GhostraceError::BackupExists);
    }
    let parent = parent_directory(destination)?;
    ensure_private_directory(parent)?;
    verify_database_artifacts(source)?;
    verify_private_file(source)?;
    if fs::symlink_metadata(destination).is_ok() {
        return Err(GhostraceError::BackupExists);
    }

    let mut input_options = OpenOptions::new();
    input_options.read(true);
    add_no_follow(&mut input_options);
    let mut input = input_options.open(source).map_err(|error| secure_open_error(source, error))?;
    let mut output_options = OpenOptions::new();
    output_options.write(true).create_new(true);
    add_no_follow(&mut output_options);
    let mut output =
        output_options.open(destination).map_err(|error| secure_open_error(destination, error))?;
    let bytes =
        std::io::copy(&mut input, &mut output).map_err(|error| io_error(destination, error))?;
    output.sync_all().map_err(|error| io_error(destination, error))?;
    set_file_mode_handle(destination, &output)?;
    verify_private_file(destination)?;
    Ok(bytes)
}

fn open_database_file(path: &Path) -> Result<File, GhostraceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            verify_file_metadata(path, &metadata)?;
            let mut options = OpenOptions::new();
            options.read(true).write(true);
            add_no_follow(&mut options);
            options.open(path).map_err(|error| secure_open_error(path, error))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            add_no_follow(&mut options);
            let file = options.open(path).map_err(|error| {
                if error.kind() == ErrorKind::AlreadyExists {
                    GhostraceError::PathRace
                } else {
                    secure_open_error(path, error)
                }
            })?;
            set_file_mode_handle(path, &file)?;
            Ok(file)
        }
        Err(error) => Err(io_error(path, error)),
    }
}

fn verify_file_handle(path: &Path, file: &File) -> Result<FileIdentity, GhostraceError> {
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    verify_file_metadata(path, &metadata)?;
    let path_identity = verify_private_file(path)?;
    let handle_identity = file_identity(&metadata);
    if path_identity != handle_identity {
        return Err(GhostraceError::PathRace);
    }
    Ok(handle_identity)
}

fn verify_private_file(path: &Path) -> Result<FileIdentity, GhostraceError> {
    let metadata = symlink_metadata(path)?;
    verify_file_metadata(path, &metadata)?;
    Ok(file_identity(&metadata))
}

fn verify_file_metadata(path: &Path, metadata: &Metadata) -> Result<(), GhostraceError> {
    verify_file_shape(metadata)?;
    if file_mode(metadata) != PRIVATE_FILE_MODE {
        return Err(GhostraceError::InsecurePermissions(path.to_path_buf()));
    }
    Ok(())
}

fn verify_file_shape(metadata: &Metadata) -> Result<(), GhostraceError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GhostraceError::UnsafePath);
    }
    if !owned_by_current_user(metadata) {
        return Err(GhostraceError::UnexpectedOwner);
    }
    if hard_link_count(metadata) != 1 {
        return Err(GhostraceError::UnexpectedHardLinks);
    }
    Ok(())
}

fn verify_directory_metadata(
    path: &Path,
    metadata: &Metadata,
    require_private_mode: bool,
) -> Result<bool, GhostraceError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GhostraceError::UnsafePath);
    }
    if !owned_by_current_user(metadata) {
        return Err(GhostraceError::UnexpectedOwner);
    }
    if require_private_mode && directory_mode(metadata) != PRIVATE_DIRECTORY_MODE {
        return Err(GhostraceError::InsecurePermissions(path.to_path_buf()));
    }
    Ok(true)
}

fn ensure_directory_components(path: &Path) -> Result<(), GhostraceError> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| matches!(component, Component::ParentDir))
    {
        return Err(GhostraceError::UnsafePath);
    }

    let mut current = if path.is_absolute() { PathBuf::from("/") } else { PathBuf::from(".") };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current = PathBuf::from("/"),
            Component::CurDir => {}
            Component::ParentDir => return Err(GhostraceError::UnsafePath),
            Component::Normal(name) => {
                current.push(name);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) => {
                        if metadata.file_type().is_symlink() {
                            if !is_trusted_system_alias(&current) {
                                return Err(GhostraceError::UnsafePath);
                            }
                            let target = fs::metadata(&current)
                                .map_err(|error| io_error(&current, error))?;
                            if !target.is_dir() {
                                return Err(GhostraceError::UnsafePath);
                            }
                        } else if !metadata.is_dir() {
                            return Err(GhostraceError::UnsafePath);
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        fs::create_dir(&current).map_err(|create_error| {
                            if create_error.kind() == ErrorKind::AlreadyExists {
                                GhostraceError::PathRace
                            } else {
                                io_error(&current, create_error)
                            }
                        })?;
                        set_directory_mode(&current)?;
                        let created = symlink_metadata(&current)?;
                        if created.file_type().is_symlink() || !created.is_dir() {
                            return Err(GhostraceError::UnsafePath);
                        }
                    }
                    Err(error) => return Err(io_error(&current, error)),
                }
            }
        }
    }
    Ok(())
}

fn is_trusted_system_alias(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        // macOS exposes these root-owned compatibility aliases.  They are not
        // user-controlled path components, but rejecting them would make a
        // normal `tempdir()` path unusable (`/var` -> `/private/var`).
        let expected = match path.to_str() {
            Some("/var") => Some("/private/var"),
            Some("/tmp") => Some("/private/tmp"),
            Some("/etc") => Some("/private/etc"),
            _ => None,
        };
        expected.is_some_and(|target| {
            fs::canonicalize(path)
                .ok()
                .and_then(|canonical| canonical.to_str().map(str::to_owned))
                .as_deref()
                == Some(target)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

fn parent_directory(path: &Path) -> Result<&Path, GhostraceError> {
    path.parent().filter(|parent| !parent.as_os_str().is_empty()).ok_or(GhostraceError::UnsafePath)
}

fn sidecar_path(path: &Path, suffix: &str) -> Result<PathBuf, GhostraceError> {
    let name = path.file_name().ok_or(GhostraceError::UnsafePath)?.to_string_lossy();
    Ok(path.with_file_name(format!("{name}{suffix}")))
}

fn is_sidecar_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    DATABASE_ARTIFACT_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn symlink_metadata(path: &Path) -> Result<Metadata, GhostraceError> {
    fs::symlink_metadata(path).map_err(|error| io_error(path, error))
}

fn io_error(path: &Path, source: std::io::Error) -> GhostraceError {
    GhostraceError::Io { path: path.to_path_buf(), source }
}

fn secure_open_error(path: &Path, source: std::io::Error) -> GhostraceError {
    #[cfg(unix)]
    if source.raw_os_error() == Some(libc::ELOOP) {
        return GhostraceError::UnsafePath;
    }
    io_error(path, source)
}

fn set_directory_mode(path: &Path) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
            .map_err(|error| io_error(path, error))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn set_file_mode(path: &Path) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .map_err(|error| io_error(path, error))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn set_file_mode_handle(path: &Path, file: &File) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .map_err(|error| io_error(path, error))?;
    }
    #[cfg(not(unix))]
    let _ = (path, file);
    Ok(())
}

fn add_no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(not(unix))]
    let _ = options;
}

fn file_identity(metadata: &Metadata) -> FileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileIdentity { device: metadata.dev(), inode: metadata.ino() }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        FileIdentity { device: 0, inode: 0 }
    }
}

fn file_mode(metadata: &Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.mode() & 0o777
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        PRIVATE_FILE_MODE
    }
}

fn directory_mode(metadata: &Metadata) -> u32 {
    file_mode(metadata)
}

fn hard_link_count(metadata: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink()
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        1
    }
}

fn owned_by_current_user(metadata: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid has no memory or pointer arguments and is provided by
        // the host POSIX runtime.
        metadata.uid() == unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        true
    }
}

#[cfg(test)]
mod test_hooks {
    use std::{
        path::Path,
        sync::{Mutex, OnceLock},
    };

    type Hook = Box<dyn FnOnce(&Path) + Send + 'static>;

    static OPEN_HOOK: OnceLock<Mutex<Option<Hook>>> = OnceLock::new();

    pub(super) fn install(hook: Hook) {
        let slot = OPEN_HOOK.get_or_init(|| Mutex::new(None));
        let mut guard = slot.lock().expect("test hook lock");
        assert!(guard.replace(hook).is_none(), "test hook already installed");
    }

    pub(super) fn take() -> Option<Hook> {
        OPEN_HOOK.get().and_then(|slot| slot.lock().expect("test hook lock").take())
    }
}

fn run_test_open_hook(path: &Path) {
    #[cfg(test)]
    if let Some(hook) = test_hooks::take() {
        hook(path);
    }
    #[cfg(not(test))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };

    use tempfile::tempdir;

    use super::*;

    fn private(path: &Path) {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
            .expect("private mode");
    }

    #[test]
    fn secure_database_rejects_symlinks_non_regular_files_and_hard_links() {
        let root = tempdir().expect("root");
        let root_path = fs::canonicalize(root.path()).expect("canonical root");
        private(&root_path);
        let outside = root_path.join("outside.sqlite3");
        fs::write(&outside, b"outside").expect("outside");
        set_file_mode(&outside).expect("outside mode");

        let symlinked = root_path.join("symlink.sqlite3");
        symlink(&outside, &symlinked).expect("symlink");
        assert!(matches!(open_database(&symlinked), Err(GhostraceError::UnsafePath)));
        fs::remove_file(&symlinked).expect("remove symlink");

        let directory = root_path.join("directory.sqlite3");
        fs::create_dir(&directory).expect("directory");
        private(&directory);
        assert!(matches!(open_database(&directory), Err(GhostraceError::UnsafePath)));
        fs::remove_dir(&directory).expect("remove directory");

        let original = root_path.join("original.sqlite3");
        fs::write(&original, b"original").expect("original");
        set_file_mode(&original).expect("original mode");
        let hard_link = root_path.join("hard-link.sqlite3");
        fs::hard_link(&original, &hard_link).expect("hard link");
        let hard_link_error = match open_database(&hard_link) {
            Ok(_) => panic!("hard link must refuse"),
            Err(error) => error,
        };
        assert!(matches!(hard_link_error, GhostraceError::UnexpectedHardLinks));
    }

    #[test]
    fn secure_database_rejects_parent_replacement_between_checks_and_open() {
        let root = tempdir().expect("root");
        let root_path = fs::canonicalize(root.path()).expect("canonical root");
        private(&root_path);
        let parent = root_path.join("journal");
        fs::create_dir(&parent).expect("journal directory");
        private(&parent);
        let outside = root_path.join("outside");
        fs::create_dir(&outside).expect("outside directory");
        private(&outside);
        let moved = root.path().join("journal-moved");
        let original = parent.clone();
        let moved_for_hook = moved.clone();
        let outside_for_hook = outside.clone();
        test_hooks::install(Box::new(move |_| {
            fs::rename(&original, &moved_for_hook).expect("move journal directory");
            symlink(&outside_for_hook, &original).expect("replace journal directory");
        }));

        let path = parent.join("journal.sqlite3");
        let error = match open_database(&path) {
            Ok(_) => panic!("parent replacement must refuse"),
            Err(error) => error,
        };
        assert!(matches!(error, GhostraceError::UnsafePath | GhostraceError::PathRace));
        assert!(!outside.join("journal.sqlite3").exists());

        fs::remove_file(&parent).expect("remove replacement symlink");
        fs::rename(&moved, &parent).expect("restore journal directory");
    }

    #[test]
    fn private_artifact_modes_are_exact() {
        let root = tempdir().expect("root");
        private(root.path());
        let artifact = root.path().join("artifact");
        fs::write(&artifact, b"secret").expect("artifact");
        set_private_file_permissions(&artifact).expect("set private artifact");
        assert_eq!(
            file_mode(&fs::symlink_metadata(&artifact).expect("metadata")),
            PRIVATE_FILE_MODE
        );
    }

    #[test]
    fn insecure_database_and_sidecar_modes_are_refused() {
        let root = tempdir().expect("root");
        let root_path = fs::canonicalize(root.path()).expect("canonical root");
        private(&root_path);

        let insecure_parent = root_path.join("insecure-parent");
        fs::create_dir(&insecure_parent).expect("insecure parent");
        fs::set_permissions(&insecure_parent, fs::Permissions::from_mode(0o750))
            .expect("insecure parent mode");
        let parent_error = match open_database(&insecure_parent.join("journal.sqlite3")) {
            Ok(_) => panic!("insecure parent must refuse"),
            Err(error) => error,
        };
        assert!(matches!(parent_error, GhostraceError::InsecurePermissions(_)));

        let database = root_path.join("journal.sqlite3");
        fs::write(&database, b"sqlite placeholder").expect("database");
        fs::set_permissions(&database, fs::Permissions::from_mode(0o640)).expect("database mode");
        let database_error = match open_database(&database) {
            Ok(_) => panic!("insecure database must refuse"),
            Err(error) => error,
        };
        assert!(matches!(database_error, GhostraceError::InsecurePermissions(_)));

        fs::set_permissions(&database, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .expect("database mode");
        let sidecar = database.with_file_name("journal.sqlite3-wal");
        fs::write(&sidecar, b"sidecar").expect("sidecar");
        fs::set_permissions(&sidecar, fs::Permissions::from_mode(0o640)).expect("sidecar mode");
        let sidecar_error =
            verify_database_artifacts(&database).expect_err("insecure sidecar must refuse");
        assert!(matches!(sidecar_error, GhostraceError::InsecurePermissions(_)));
    }
}
