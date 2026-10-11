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

/// Retain verified source identities throughout export, including after their
/// names disappear or change. Reused identities conservatively refuse export
/// after an artifact is unlinked; they cannot permit overwriting the source.
pub(crate) struct DatabaseArtifactIdentities {
    identities: Vec<FileIdentity>,
}

impl DatabaseArtifactIdentities {
    pub(crate) fn capture(database: &Path) -> Result<Self, GhostraceError> {
        verify_existing_private_directory(parent_directory(database)?)?;
        let mut artifacts = Vec::with_capacity(DATABASE_ARTIFACT_SUFFIXES.len() + 1);
        artifacts.push(database.to_path_buf());
        for suffix in DATABASE_ARTIFACT_SUFFIXES {
            artifacts.push(sidecar_path(database, suffix)?);
        }
        let mut identities = Vec::with_capacity(artifacts.len());
        for artifact in artifacts {
            // Opening and closing an independent source descriptor could
            // release SQLite's process-associated POSIX locks on that inode.
            let metadata = match fs::symlink_metadata(&artifact) {
                Ok(metadata) => metadata,
                Err(error) if artifact != database && error.kind() == ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => return Err(io_error(&artifact, error)),
            };
            verify_file_metadata(&artifact, &metadata)?;
            identities.push(file_identity(&metadata));
        }
        Ok(Self { identities })
    }

    pub(crate) fn refuse_destination(&self, destination: &Path) -> Result<(), GhostraceError> {
        #[cfg(unix)]
        {
            let destination_metadata = match fs::metadata(destination) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(io_error(destination, error)),
            };
            let destination_identity = file_identity(&destination_metadata);
            if self.identities.contains(&destination_identity) {
                return Err(GhostraceError::ExportSourceConflict);
            }
        }
        #[cfg(not(unix))]
        let _ = (&self.identities, destination);
        Ok(())
    }
}

/// Reserve the database and its sidecar names before publishing an export.
/// Compare resolved parent directories so relative names and trusted system
/// aliases cannot evade the check. Sidecar names remain reserved while absent:
/// SQLite may create them after the export starts. Inode checks also recognize
/// existing aliases and sidecars whose base names resolve to the journal on
/// Unix, including case and Unicode aliases on macOS filesystems.
pub(crate) fn refuse_database_artifact_destination(
    database: &Path,
    destination: &Path,
) -> Result<(), GhostraceError> {
    if database == destination {
        return Err(GhostraceError::ExportSourceConflict);
    }
    let database_parent = parent_directory(database)?;
    let database_parent =
        fs::canonicalize(database_parent).map_err(|error| io_error(database_parent, error))?;
    let destination_parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let destination_parent = match fs::canonicalize(destination_parent) {
        Ok(parent) => parent,
        // The normal artifact-parent preparation may create a new folder.
        // An absent parent cannot currently alias the existing database home.
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(destination_parent, error)),
    };
    let mut artifacts = Vec::with_capacity(DATABASE_ARTIFACT_SUFFIXES.len() + 1);
    artifacts.push(database.to_path_buf());
    for suffix in DATABASE_ARTIFACT_SUFFIXES {
        artifacts.push(sidecar_path(database, suffix)?);
    }
    if database_parent == destination_parent
        && artifacts.iter().any(|artifact| artifact.file_name() == destination.file_name())
    {
        return Err(GhostraceError::ExportSourceConflict);
    }
    #[cfg(unix)]
    {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        // Resolve an absent sidecar's base through the filesystem rather than
        // inventing a case/Unicode normalization rule for the journal name.
        // Reserve ASCII suffix case variants on macOS, where ordinary APFS
        // volumes treat those names as aliases too.
        if let Some(name) = destination.file_name().map(OsStrExt::as_bytes) {
            for suffix in DATABASE_ARTIFACT_SUFFIXES {
                let suffix = suffix.as_bytes();
                if name.len() <= suffix.len() {
                    continue;
                }
                let (base, ending) = name.split_at(name.len() - suffix.len());
                let suffix_matches = if cfg!(target_os = "macos") {
                    ending.eq_ignore_ascii_case(suffix)
                } else {
                    ending == suffix
                };
                if !suffix_matches {
                    continue;
                }
                let database_alias = destination.with_file_name(OsStr::from_bytes(base));
                match fs::metadata(&database_alias) {
                    Ok(metadata) => {
                        let database_metadata =
                            fs::metadata(database).map_err(|error| io_error(database, error))?;
                        if file_identity(&metadata) == file_identity(&database_metadata) {
                            return Err(GhostraceError::ExportSourceConflict);
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => return Err(io_error(&database_alias, error)),
                }
            }
        }
        let destination_metadata = match fs::metadata(destination) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(destination, error)),
        };
        for artifact in artifacts {
            match fs::metadata(&artifact) {
                Ok(metadata)
                    if file_identity(&metadata) == file_identity(&destination_metadata) =>
                {
                    return Err(GhostraceError::ExportSourceConflict);
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(&artifact, error)),
            }
        }
    }
    Ok(())
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
    open_read_only_database_inner(path, None)
}

/// Refuse oversized SQLite recovery/coordination input before SQLite opens it.
pub(crate) fn open_bounded_read_only_database(
    path: &Path,
    max_sidecar_bytes: u64,
) -> Result<Connection, GhostraceError> {
    open_read_only_database_inner(path, Some(max_sidecar_bytes))
}

fn open_read_only_database_inner(
    path: &Path,
    max_sidecar_bytes: Option<u64>,
) -> Result<Connection, GhostraceError> {
    let parent = parent_directory(path)?;
    let parent_identity = verify_existing_private_directory(parent)?;
    verify_read_only_artifacts(path, max_sidecar_bytes)?;
    let mut options = OpenOptions::new();
    options.read(true);
    add_no_follow(&mut options);
    let file = options.open(path).map_err(|error| secure_open_error(path, error))?;
    let file_identity = verify_file_handle(path, &file)?;
    run_test_open_hook(path);
    if verify_existing_private_directory(parent)? != parent_identity
        || verify_private_file(path)? != file_identity
    {
        return Err(GhostraceError::PathRace);
    }
    verify_read_only_artifacts(path, max_sidecar_bytes)?;
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // Keep the verified descriptor alive until SQLite's path-based open and
    // the second identity comparison have completed.
    if verify_existing_private_directory(parent)? != parent_identity
        || verify_private_file(path)? != file_identity
    {
        return Err(GhostraceError::PathRace);
    }
    verify_read_only_artifacts(path, max_sidecar_bytes)?;
    drop(file);
    Ok(connection)
}

fn verify_existing_private_directory(path: &Path) -> Result<FileIdentity, GhostraceError> {
    check_directory_components(path, false)?;
    let metadata = symlink_metadata(path)?;
    verify_directory_metadata(path, &metadata, true)?;
    Ok(file_identity(&metadata))
}

fn verify_read_only_artifacts(
    path: &Path,
    max_sidecar_bytes: Option<u64>,
) -> Result<(), GhostraceError> {
    verify_existing_private_directory(parent_directory(path)?)?;
    verify_private_file(path)?;
    let mut total_bytes = 0_u64;
    for suffix in DATABASE_ARTIFACT_SUFFIXES {
        let sidecar = sidecar_path(path, suffix)?;
        match fs::symlink_metadata(&sidecar) {
            Ok(metadata) => {
                verify_file_metadata(&sidecar, &metadata)?;
                total_bytes = total_bytes
                    .checked_add(metadata.len())
                    .ok_or(GhostraceError::ReadOnlyResourceLimit)?;
                if max_sidecar_bytes.is_some_and(|limit| total_bytes > limit) {
                    return Err(GhostraceError::ReadOnlyResourceLimit);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&sidecar, error)),
        }
    }
    Ok(())
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

/// Own the directory authority used throughout one atomic artifact export.
pub(crate) struct ArtifactDirectory {
    path: PathBuf,
    #[cfg(unix)]
    file: File,
    #[cfg(unix)]
    identity: FileIdentity,
}

pub(crate) struct ArtifactTemporary<'directory> {
    directory: &'directory ArtifactDirectory,
    file: File,
    #[cfg(unix)]
    name: Option<std::ffi::CString>,
    #[cfg(not(unix))]
    path: Option<tempfile::TempPath>,
}

impl ArtifactDirectory {
    pub(crate) fn open(path: &Path) -> Result<Self, GhostraceError> {
        ensure_artifact_parent(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|error| secure_open_error(path, error))?;
            let metadata = file.metadata().map_err(|error| io_error(path, error))?;
            verify_directory_metadata(path, &metadata, false)?;
            let directory =
                Self { path: path.to_path_buf(), identity: file_identity(&metadata), file };
            directory.revalidate()?;
            Ok(directory)
        }
        #[cfg(not(unix))]
        Ok(Self { path: path.to_path_buf() })
    }

    pub(crate) fn revalidate(&self) -> Result<(), GhostraceError> {
        check_directory_components(&self.path, false).map_err(|_| GhostraceError::PathRace)?;
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| GhostraceError::PathRace)?;
        verify_directory_metadata(&self.path, &metadata, false)
            .map_err(|_| GhostraceError::PathRace)?;
        #[cfg(unix)]
        if file_identity(&metadata) != self.identity {
            return Err(GhostraceError::PathRace);
        }
        Ok(())
    }

    pub(crate) fn temporary(
        &self,
        prefix: &str,
        anonymous: bool,
    ) -> Result<ArtifactTemporary<'_>, GhostraceError> {
        self.revalidate()?;
        #[cfg(unix)]
        {
            use std::{
                ffi::CString,
                os::fd::{AsRawFd, FromRawFd},
            };
            for _ in 0..16 {
                let name = CString::new(format!("{prefix}{}", uuid::Uuid::new_v4().simple()))
                    .map_err(|_| GhostraceError::UnsafePath)?;
                // SAFETY: the directory descriptor is owned here, the name
                // is NUL-terminated, and O_EXCL/O_NOFOLLOW prohibit aliasing an
                // existing entry. Ownership of the returned descriptor follows.
                let fd = unsafe {
                    libc::openat(
                        self.file.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDWR
                            | libc::O_CREAT
                            | libc::O_EXCL
                            | libc::O_NOFOLLOW
                            | libc::O_CLOEXEC,
                        PRIVATE_FILE_MODE as libc::c_uint,
                    )
                };
                if fd < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == ErrorKind::AlreadyExists {
                        continue;
                    }
                    return Err(io_error(&self.path, error));
                }
                // SAFETY: openat returned a new owned descriptor exactly once.
                let file = unsafe { File::from_raw_fd(fd) };
                let mut temporary = ArtifactTemporary { directory: self, file, name: Some(name) };
                set_file_mode_handle(&self.path, &temporary.file)?;
                if anonymous {
                    temporary.unlink_name()?;
                }
                return Ok(temporary);
            }
            Err(io_error(
                &self.path,
                std::io::Error::new(ErrorKind::AlreadyExists, "temporary artifact collision limit"),
            ))
        }
        #[cfg(not(unix))]
        {
            let _ = anonymous;
            let temporary = tempfile::Builder::new()
                .prefix(prefix)
                .tempfile_in(&self.path)
                .map_err(|error| io_error(&self.path, error))?;
            let (file, path) = temporary.into_parts();
            Ok(ArtifactTemporary { directory: self, file, path: Some(path) })
        }
    }
}

impl ArtifactTemporary<'_> {
    pub(crate) fn as_file(&self) -> &File {
        &self.file
    }
    pub(crate) fn as_file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    #[cfg(unix)]
    fn unlink_name(&mut self) -> Result<(), GhostraceError> {
        use std::os::fd::AsRawFd;
        let name = self.name.as_ref().ok_or(GhostraceError::UnsafePath)?;
        // SAFETY: directory and name are owned, live values; flags select a file.
        if unsafe { libc::unlinkat(self.directory.file.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(io_error(&self.directory.path, std::io::Error::last_os_error()));
        }
        self.name = None;
        Ok(())
    }

    /// Publish within the pinned directory, so a namespace change after the
    /// recheck cannot redirect either the rename/link or directory fsync.
    pub(crate) fn publish(
        &mut self,
        destination: &Path,
        force: bool,
    ) -> Result<(), GhostraceError> {
        self.directory.revalidate()?;
        #[cfg(unix)]
        {
            use std::{
                ffi::CString,
                os::{fd::AsRawFd, unix::ffi::OsStrExt},
            };
            let destination_name =
                CString::new(destination.file_name().ok_or(GhostraceError::UnsafePath)?.as_bytes())
                    .map_err(|_| GhostraceError::UnsafePath)?;
            let directory_fd = self.directory.file.as_raw_fd();
            let name = self.name.as_ref().ok_or(GhostraceError::UnsafePath)?;
            let metadata = self.file.metadata().map_err(|error| io_error(destination, error))?;
            verify_file_metadata(destination, &metadata)?;
            let named =
                stat_at(directory_fd, name, destination)?.ok_or(GhostraceError::PathRace)?;
            if stat_identity(&named) != file_identity(&metadata) {
                return Err(GhostraceError::PathRace);
            }
            if let Some(existing) = stat_at(directory_fd, &destination_name, destination)? {
                if existing.st_mode & libc::S_IFMT != libc::S_IFREG {
                    return Err(GhostraceError::UnsafePath);
                }
                // SAFETY: geteuid has no preconditions.
                if existing.st_uid != unsafe { libc::geteuid() } {
                    return Err(GhostraceError::UnexpectedOwner);
                }
                if existing.st_nlink != 1 {
                    return Err(GhostraceError::UnexpectedHardLinks);
                }
                if !force {
                    return Err(GhostraceError::ExportExists(destination.to_path_buf()));
                }
            }
            // SAFETY: both operations use owned NUL-terminated basenames and
            // the pinned directory descriptor. linkat refuses an existing name
            // atomically; renameat supplies the requested atomic replacement.
            run_test_publish_hook(destination);
            let result = unsafe {
                if force {
                    libc::renameat(
                        directory_fd,
                        name.as_ptr(),
                        directory_fd,
                        destination_name.as_ptr(),
                    )
                } else {
                    libc::linkat(
                        directory_fd,
                        name.as_ptr(),
                        directory_fd,
                        destination_name.as_ptr(),
                        0,
                    )
                }
            };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == ErrorKind::AlreadyExists {
                    return Err(GhostraceError::ExportExists(destination.to_path_buf()));
                }
                return Err(io_error(destination, error));
            }
            if force {
                self.name = None;
            } else {
                self.unlink_name()?;
            }
            // The temporary basename may have been replaced after its stat.
            // Verify the actual published entry against the retained, validated
            // inode without reopening the mutable destination pathname. Leave a
            // different inode intact: it belongs to the actor that replaced it.
            let published = stat_at(directory_fd, &destination_name, destination)?
                .ok_or(GhostraceError::PathRace)?;
            if stat_identity(&published) != file_identity(&metadata) {
                return Err(GhostraceError::PathRace);
            }
            verify_file_metadata(
                destination,
                &self.file.metadata().map_err(|error| io_error(destination, error))?,
            )?;
            self.directory
                .file
                .sync_all()
                .map_err(|error| io_error(&self.directory.path, error))?;
        }
        #[cfg(not(unix))]
        {
            let path = self.path.take().ok_or(GhostraceError::UnsafePath)?;
            let result =
                if force { path.persist(destination) } else { path.persist_noclobber(destination) };
            if let Err(error) = result {
                self.path = Some(error.path);
                if error.error.kind() == ErrorKind::AlreadyExists {
                    return Err(GhostraceError::ExportExists(destination.to_path_buf()));
                }
                return Err(io_error(destination, error.error));
            }
            set_file_mode_handle(destination, &self.file)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn stat_at(
    directory_fd: std::os::fd::RawFd,
    name: &std::ffi::CStr,
    path: &Path,
) -> Result<Option<libc::stat>, GhostraceError> {
    // SAFETY: the stat buffer is fully initialized by successful fstatat;
    // name is NUL-terminated and the descriptor remains owned by the caller.
    let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstatat(directory_fd, name.as_ptr(), &mut stat, libc::AT_SYMLINK_NOFOLLOW) }
        == 0
    {
        return Ok(Some(stat));
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == ErrorKind::NotFound {
        return Ok(None);
    }
    Err(io_error(path, error))
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)] // libc's stat integer widths vary across Unix targets.
fn stat_identity(stat: &libc::stat) -> FileIdentity {
    FileIdentity { device: stat.st_dev as u64, inode: stat.st_ino as u64 }
}

impl Drop for ArtifactTemporary<'_> {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(name) = self.name.as_ref() {
            use std::os::fd::AsRawFd;
            // Do not remove a replacement entry another actor supplied.
            if let (Ok(Some(named)), Ok(metadata)) = (
                stat_at(self.directory.file.as_raw_fd(), name, &self.directory.path),
                self.file.metadata(),
            ) {
                if stat_identity(&named) == file_identity(&metadata) {
                    // SAFETY: name and the pinned directory descriptor are live.
                    unsafe {
                        libc::unlinkat(self.directory.file.as_raw_fd(), name.as_ptr(), 0);
                    }
                }
            }
        }
    }
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
    check_directory_components(path, true)
}

fn check_directory_components(path: &Path, create_missing: bool) -> Result<(), GhostraceError> {
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
                        if !create_missing {
                            return Err(io_error(&current, error));
                        }
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
    let mut name = path.file_name().ok_or(GhostraceError::UnsafePath)?.to_os_string();
    name.push(suffix);
    Ok(path.with_file_name(name))
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

#[cfg(test)]
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
    use std::{cell::RefCell, path::Path};

    type Hook = Box<dyn FnOnce(&Path) + Send + 'static>;

    thread_local! {
        static OPEN_HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
        static PUBLISH_HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    pub(super) fn install(hook: Hook) {
        OPEN_HOOK.with(|slot| {
            assert!(slot.borrow_mut().replace(hook).is_none(), "test hook already installed");
        });
    }

    pub(super) fn take() -> Option<Hook> {
        OPEN_HOOK.with(|slot| slot.borrow_mut().take())
    }

    pub(super) fn install_publish(hook: Hook) {
        PUBLISH_HOOK.with(|slot| {
            assert!(slot.borrow_mut().replace(hook).is_none(), "test hook already installed");
        });
    }

    pub(super) fn take_publish() -> Option<Hook> {
        PUBLISH_HOOK.with(|slot| slot.borrow_mut().take())
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

#[cfg(unix)]
fn run_test_publish_hook(path: &Path) {
    #[cfg(test)]
    if let Some(hook) = test_hooks::take_publish() {
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
    fn export_reserves_wal_and_shm_names_before_they_exist() {
        let directory = tempdir().expect("directory");
        private(directory.path());
        let source = directory.path().join("journal.sqlite3");
        let connection = open_database(&source).expect("database");
        connection.execute_batch("CREATE TABLE original(value TEXT)").expect("schema");
        drop(connection);
        let before = fs::read(&source).expect("source bytes");
        for name in ["journal.sqlite3-wal", "journal.sqlite3-shm"] {
            let output = directory.path().join(name);
            assert!(!output.exists());
            assert!(matches!(
                refuse_database_artifact_destination(&source, &output),
                Err(GhostraceError::ExportSourceConflict)
            ));
            assert!(!output.exists());
        }
        assert_eq!(fs::read(&source).expect("source bytes"), before);
    }

    #[test]
    fn sidecar_suffixes_preserve_non_utf8_path_bytes() {
        use std::{
            ffi::OsString,
            os::unix::ffi::{OsStrExt, OsStringExt},
        };
        let path = PathBuf::from(OsString::from_vec(b"journal-\xff.sqlite3".to_vec()));
        let sidecar = sidecar_path(&path, "-wal").unwrap();
        assert_eq!(sidecar.as_os_str().as_bytes(), b"journal-\xff.sqlite3-wal");
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
    fn readonly_database_rejects_private_file_or_parent_replacement() {
        for replace_parent in [false, true] {
            let root = tempdir().unwrap();
            let root_path = fs::canonicalize(root.path()).unwrap();
            private(&root_path);
            let parent = root_path.join("journal");
            fs::create_dir(&parent).unwrap();
            private(&parent);
            let path = parent.join("journal.sqlite3");
            let database = open_database(&path).unwrap();
            database.execute_batch("CREATE TABLE original(value TEXT)").unwrap();
            drop(database);
            let original = if replace_parent { parent.clone() } else { path.clone() };
            let moved = root_path.join("moved");
            let replacement = original.clone();
            test_hooks::install(Box::new(move |_| {
                fs::rename(&original, &moved).unwrap();
                if replace_parent {
                    fs::create_dir(&replacement).unwrap();
                    private(&replacement);
                    let new_database = replacement.join("journal.sqlite3");
                    fs::write(&new_database, b"").unwrap();
                    set_file_mode(&new_database).unwrap();
                } else {
                    fs::write(&replacement, b"").unwrap();
                    set_file_mode(&replacement).unwrap();
                }
            }));
            let result = open_read_only_database(&path);
            let _ = test_hooks::take();
            assert!(
                matches!(result, Err(GhostraceError::PathRace)),
                "must refuse a different private inode, replace_parent={replace_parent}"
            );
        }
    }

    #[test]
    fn private_artifact_modes_are_exact() {
        use std::io::Write;

        let root = tempdir().expect("root");
        private(root.path());
        let artifact = root.path().join("artifact");
        let directory = ArtifactDirectory::open(root.path()).expect("artifact directory");
        let mut temporary = directory
            .temporary(".ghostrace-export-incomplete-", false)
            .expect("temporary artifact");
        temporary.as_file_mut().write_all(b"secret").expect("artifact bytes");
        assert_eq!(
            file_mode(&temporary.as_file().metadata().expect("temporary metadata")),
            PRIVATE_FILE_MODE
        );
        temporary.publish(&artifact, false).expect("publish artifact");
        assert_eq!(
            file_mode(&fs::symlink_metadata(&artifact).expect("metadata")),
            PRIVATE_FILE_MODE
        );
    }

    #[test]
    fn artifact_publication_stays_in_pinned_directory_after_the_final_recheck() {
        use std::io::Write;

        for force in [false, true] {
            let root = tempdir().expect("root");
            let root_path = fs::canonicalize(root.path()).expect("canonical root");
            let original = root_path.join("exports");
            let moved = root_path.join("moved-exports");
            let outside = root_path.join("journal");
            fs::create_dir(&original).expect("export directory");
            fs::create_dir(&outside).expect("journal directory");
            let source = outside.join("journal.sqlite3");
            fs::write(&source, b"original journal bytes").expect("source");
            set_file_mode(&source).expect("source mode");
            let destination = original.join("journal.sqlite3");
            if force {
                fs::write(&destination, b"previous export").expect("old destination");
                set_file_mode(&destination).expect("old destination mode");
            }
            let directory = ArtifactDirectory::open(&original).expect("artifact directory");
            let mut temporary = directory
                .temporary(".ghostrace-export-incomplete-", false)
                .expect("temporary artifact");
            temporary.as_file_mut().write_all(b"complete export").expect("export bytes");
            temporary.as_file().sync_all().expect("flush export");
            let moved_for_hook = moved.clone();
            let original_for_hook = original.clone();
            let outside_for_hook = outside.clone();
            // Replace the namespace after every prepublication check. A final
            // path-based rename would overwrite the journal in this window.
            test_hooks::install_publish(Box::new(move |_| {
                fs::rename(&original_for_hook, &moved_for_hook).expect("move original parent");
                symlink(&outside_for_hook, &original_for_hook).expect("redirect parent");
            }));
            temporary.publish(&destination, force).expect("publish in pinned directory");
            drop(temporary);
            assert_eq!(fs::read(&source).expect("source bytes"), b"original journal bytes");
            let published = moved.join("journal.sqlite3");
            assert_eq!(fs::read(&published).expect("published bytes"), b"complete export");
            assert_eq!(
                file_mode(&fs::metadata(published).expect("published metadata")),
                PRIVATE_FILE_MODE
            );
            assert_eq!(fs::read_dir(&moved).expect("original parent entries").count(), 1);
            fs::remove_file(&original).expect("remove replacement symlink");
        }
    }

    #[test]
    fn artifact_refusal_cleans_named_spool_in_the_moved_original_directory() {
        use std::io::Write;

        let root = tempdir().expect("root");
        let root_path = fs::canonicalize(root.path()).expect("canonical root");
        let original = root_path.join("exports");
        let moved = root_path.join("moved-exports");
        let outside = root_path.join("other");
        fs::create_dir(&original).expect("export directory");
        fs::create_dir(&outside).expect("outside directory");
        let directory = ArtifactDirectory::open(&original).expect("artifact directory");
        let mut temporary = directory
            .temporary(".ghostrace-export-incomplete-", false)
            .expect("temporary artifact");
        temporary.as_file_mut().write_all(b"private export").expect("export bytes");
        fs::rename(&original, &moved).expect("move original parent");
        symlink(&outside, &original).expect("redirect parent");
        assert!(matches!(
            temporary.publish(&original.join("export.jsonl"), true),
            Err(GhostraceError::PathRace)
        ));
        drop(temporary);
        assert_eq!(fs::read_dir(&moved).expect("original parent entries").count(), 0);
        assert_eq!(fs::read_dir(&outside).expect("outside entries").count(), 0);
        fs::remove_file(&original).expect("remove replacement symlink");
    }

    #[test]
    fn artifact_noclobber_preserves_destination_created_after_the_final_check() {
        use std::io::Write;

        let root = tempdir().expect("root");
        let directory = ArtifactDirectory::open(root.path()).expect("artifact directory");
        let mut temporary = directory
            .temporary(".ghostrace-export-incomplete-", false)
            .expect("temporary artifact");
        temporary.as_file_mut().write_all(b"complete export").expect("export bytes");
        let destination = root.path().join("export.jsonl");
        test_hooks::install_publish(Box::new(|path| {
            fs::write(path, b"another actor's file").expect("late destination");
            set_file_mode(path).expect("late destination mode");
        }));
        assert!(matches!(
            temporary.publish(&destination, false),
            Err(GhostraceError::ExportExists(_))
        ));
        drop(temporary);
        assert_eq!(fs::read(destination).expect("destination bytes"), b"another actor's file");
        assert_eq!(fs::read_dir(root.path()).expect("directory entries").count(), 1);
    }

    #[test]
    fn artifact_publication_rejects_a_swapped_validated_spool() {
        use std::{io::Write, os::unix::ffi::OsStrExt};

        for force in [false, true] {
            let root = tempdir().expect("root");
            let directory = ArtifactDirectory::open(root.path()).expect("artifact directory");
            let mut temporary = directory
                .temporary(".ghostrace-export-incomplete-final-", false)
                .expect("temporary artifact");
            temporary.as_file_mut().write_all(b"validated export").expect("export bytes");
            let temporary_path = root.path().join(std::ffi::OsStr::from_bytes(
                temporary.name.as_ref().expect("temporary name").to_bytes(),
            ));
            let saved = root.path().join("saved-validated-spool");
            let destination = root.path().join("export.jsonl");
            if force {
                fs::write(&destination, b"old export").expect("old destination");
                set_file_mode(&destination).expect("old destination mode");
            }
            let saved_for_hook = saved.clone();
            test_hooks::install_publish(Box::new(move |_| {
                fs::rename(&temporary_path, &saved_for_hook).expect("move validated spool");
                fs::write(&temporary_path, b"unvalidated replacement").expect("replace spool name");
                set_file_mode(&temporary_path).expect("replacement mode");
            }));
            let result = temporary.publish(&destination, force);
            assert!(
                matches!(result, Err(GhostraceError::PathRace)),
                "publishing another inode must never report success, got {result:?}"
            );
            drop(temporary);
            assert_eq!(fs::read(saved).expect("retained inode bytes"), b"validated export");
            assert_eq!(
                fs::read(destination).expect("replacement bytes"),
                b"unvalidated replacement"
            );
            assert!(fs::read_dir(root.path()).expect("directory entries").all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".ghostrace-export-incomplete-")));
        }
    }

    #[test]
    fn anonymous_artifact_spool_has_no_directory_entry() {
        use std::io::{Read, Seek, Write};

        let root = tempdir().expect("root");
        let directory = ArtifactDirectory::open(root.path()).expect("artifact directory");
        let mut body = directory
            .temporary(".ghostrace-export-incomplete-body-", true)
            .expect("anonymous body spool");
        body.as_file_mut().write_all(b"private body").expect("body bytes");
        assert_eq!(fs::read_dir(root.path()).expect("spool directory").count(), 0);
        body.as_file_mut().rewind().expect("rewind body");
        let mut bytes = Vec::new();
        body.as_file_mut().read_to_end(&mut bytes).expect("retained body handle");
        assert_eq!(bytes, b"private body");
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
