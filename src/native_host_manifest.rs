//! Explicit, reversible installation of the browser native-messaging host
//! manifest.
//!
//! A Chromium browser launches a native host only if a JSON manifest in the
//! browser's per-user `NativeMessagingHosts` directory names it and lists the
//! extension origins allowed to connect. Registration is a local mutation, so
//! it is planned, applied atomically, verified against a digest receipt, and
//! removed only while the manifest is byte-identical to what was installed.
//! Every allowed origin is an exact `chrome-extension://<32 a-p letters>/`;
//! wildcards and anything else are refused. Other hosts' manifests in the same
//! directory are never read for content, modified, or removed.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Registered host name; also the manifest file stem.
pub const NATIVE_HOST_NAME: &str = "com.alisinadevelo.ghostrace";
/// Receipt kept beside the manifest.
pub const NATIVE_HOST_RECEIPT_SUFFIX: &str = ".ghostrace-receipt";
const NATIVE_HOST_PENDING_RECEIPT_SUFFIX: &str = ".ghostrace-receipt.pending";
/// The browsers this host supports, each with its per-user manifest directory
/// below the user's `Library/Application Support`.
pub const NATIVE_HOST_CHANNELS: [(&str, &str); 4] = [
    ("chrome", "Google/Chrome/NativeMessagingHosts"),
    ("chrome-beta", "Google/Chrome Beta/NativeMessagingHosts"),
    ("chromium", "Chromium/NativeMessagingHosts"),
    ("edge", "Microsoft Edge/NativeMessagingHosts"),
];

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum NativeHostError {
    #[error("the extension origin is not an exact extension identifier")]
    InvalidOrigin,
    #[error("the host binary must be an absolute path to an executable file")]
    InvalidHostBinary,
    #[error("the browser channel is not supported")]
    UnknownChannel,
    #[error(
        "the manifest directory is unsafe (symlink, not a directory, or not owned by this user)"
    )]
    UnsafeDirectory,
    #[error("an existing manifest was not installed by GHOSTRACE")]
    ForeignManifest,
    #[error("the installed manifest changed since it was written")]
    Drift,
    #[error("a manifest appeared while it was being created")]
    ConcurrentEdit,
    #[error("a filesystem operation failed")]
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHostAction {
    Create,
    Replace,
    Remove,
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeHostChange {
    pub channel: &'static str,
    pub path: PathBuf,
    pub action: NativeHostAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHostHealth {
    Intact,
    Missing,
    Drifted,
    NotInstalled,
}

/// The manifest Chromium reads. Field order and content are fixed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    description: String,
    path: String,
    #[serde(rename = "type")]
    kind: String,
    allowed_origins: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    manifest_sha256: String,
}

/// Plans and applies manifest changes below one `Application Support` root.
pub struct NativeHostInstaller {
    support_root: PathBuf,
    host_binary: PathBuf,
    origin: String,
}

impl NativeHostInstaller {
    /// `support_root` is the user's `~/Library/Application Support` (a test
    /// directory in tests). `extension_id` is the 32-letter Chromium ID.
    pub fn new(
        support_root: impl Into<PathBuf>,
        host_binary: impl Into<PathBuf>,
        extension_id: &str,
    ) -> Result<Self, NativeHostError> {
        let origin = exact_origin(extension_id)?;
        let host_binary = host_binary.into();
        let executable = fs::metadata(&host_binary)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        let printable =
            host_binary.to_str().is_some_and(|text| !text.chars().any(char::is_control));
        if !host_binary.is_absolute() || !executable || !printable {
            return Err(NativeHostError::InvalidHostBinary);
        }
        Ok(Self { support_root: support_root.into(), host_binary, origin })
    }

    pub fn plan(&self, channel: &str) -> Result<NativeHostChange, NativeHostError> {
        let (channel, directory) = self.directory(channel)?;
        reconcile_pending(&directory)?;
        let path = directory.join(format!("{NATIVE_HOST_NAME}.json"));
        let desired = self.manifest_bytes();
        let action = match (read_regular(&path)?, read_receipt(&directory)?) {
            (None, _) => NativeHostAction::Create,
            (Some(current), Some(receipt)) if digest(&current) == receipt.manifest_sha256 => {
                if current == desired {
                    NativeHostAction::Unchanged
                } else {
                    NativeHostAction::Replace
                }
            }
            (Some(_), Some(_)) => return Err(NativeHostError::Drift),
            (Some(_), None) => return Err(NativeHostError::ForeignManifest),
        };
        Ok(NativeHostChange { channel, path, action })
    }

    pub fn install(&self, channel: &str) -> Result<NativeHostChange, NativeHostError> {
        let change = self.plan(channel)?;
        let directory = change.path.parent().expect("manifest directory").to_path_buf();
        if !directory.exists() {
            create_private_dir_all(&directory)?;
        }
        ensure_safe_directory(&directory)?;
        let desired = self.manifest_bytes();
        let receipt = Receipt { schema_version: 1, manifest_sha256: digest(&desired) };
        let receipt_bytes = serde_json::to_vec_pretty(&receipt).map_err(|_| NativeHostError::Io)?;
        match change.action {
            NativeHostAction::Create | NativeHostAction::Replace => {
                // Publish a recovery receipt before replacing the manifest.
                // If the process dies between the two renames, the next
                // planner can reconcile this exact digest instead of treating
                // its own half-publication as a foreign manifest.
                replace(&pending_receipt_path(&directory), &receipt_bytes)?;
                if change.action == NativeHostAction::Create {
                    write_new(&change.path, &desired)?;
                } else {
                    replace(&change.path, &desired)?;
                }
            }
            _ => {}
        }
        if matches!(change.action, NativeHostAction::Create | NativeHostAction::Replace) {
            replace(&receipt_path(&directory), &receipt_bytes)?;
            fs::remove_file(pending_receipt_path(&directory)).map_err(|_| NativeHostError::Io)?;
            sync_directory(&directory)?;
        }
        Ok(change)
    }

    pub fn verify(&self, channel: &str) -> Result<NativeHostHealth, NativeHostError> {
        let (_, directory) = self.directory(channel)?;
        let path = directory.join(format!("{NATIVE_HOST_NAME}.json"));
        let Some(receipt) = read_receipt(&directory)? else {
            return Ok(match read_regular(&path)? {
                None => NativeHostHealth::NotInstalled,
                Some(_) => NativeHostHealth::Drifted,
            });
        };
        let desired = self.manifest_bytes();
        Ok(match read_regular(&path) {
            Ok(None) => NativeHostHealth::Missing,
            Ok(Some(current))
                if digest(&current) == receipt.manifest_sha256 && current == desired =>
            {
                NativeHostHealth::Intact
            }
            _ => NativeHostHealth::Drifted,
        })
    }

    pub fn uninstall(&self, channel: &str) -> Result<NativeHostChange, NativeHostError> {
        let (channel, directory) = self.directory(channel)?;
        reconcile_pending(&directory)?;
        let path = directory.join(format!("{NATIVE_HOST_NAME}.json"));
        let Some(receipt) = read_receipt(&directory)? else {
            return match read_regular(&path)? {
                None => Ok(NativeHostChange { channel, path, action: NativeHostAction::Unchanged }),
                Some(_) => Err(NativeHostError::ForeignManifest),
            };
        };
        match read_regular(&path)? {
            Some(current) if digest(&current) == receipt.manifest_sha256 => {
                fs::remove_file(&path).map_err(|_| NativeHostError::Io)?;
                sync_directory(&directory)?;
            }
            Some(_) => return Err(NativeHostError::Drift),
            None => {}
        }
        fs::remove_file(receipt_path(&directory)).map_err(|_| NativeHostError::Io)?;
        sync_directory(&directory)?;
        Ok(NativeHostChange { channel, path, action: NativeHostAction::Remove })
    }

    fn directory(&self, channel: &str) -> Result<(&'static str, PathBuf), NativeHostError> {
        let (name, relative) = NATIVE_HOST_CHANNELS
            .iter()
            .find(|(name, _)| *name == channel)
            .ok_or(NativeHostError::UnknownChannel)?;
        let directory = self.support_root.join(relative);
        // Every existing ancestor below the support root must be a real,
        // user-owned directory; a symlink anywhere could redirect the write.
        let mut current = self.support_root.clone();
        ensure_safe_directory(&current)?;
        for component in Path::new(relative).components() {
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(_) => ensure_safe_directory(&current)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_) => return Err(NativeHostError::Io),
            }
        }
        Ok((name, directory))
    }

    fn manifest_bytes(&self) -> Vec<u8> {
        let manifest = Manifest {
            name: NATIVE_HOST_NAME.to_owned(),
            description: "GHOSTRACE native messaging host".to_owned(),
            path: self.host_binary.to_string_lossy().into_owned(),
            kind: "stdio".to_owned(),
            allowed_origins: vec![self.origin.clone()],
        };
        let mut bytes = serde_json::to_vec_pretty(&manifest).expect("manifest serializes");
        bytes.push(b'\n');
        bytes
    }
}

/// Validate a Chromium extension ID (32 letters a–p) and build its origin.
pub fn exact_origin(extension_id: &str) -> Result<String, NativeHostError> {
    if extension_id.len() != 32 || !extension_id.bytes().all(|byte| (b'a'..=b'p').contains(&byte)) {
        return Err(NativeHostError::InvalidOrigin);
    }
    Ok(format!("chrome-extension://{extension_id}/"))
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn receipt_path(directory: &Path) -> PathBuf {
    directory.join(format!("{NATIVE_HOST_NAME}{NATIVE_HOST_RECEIPT_SUFFIX}"))
}

fn pending_receipt_path(directory: &Path) -> PathBuf {
    directory.join(format!("{NATIVE_HOST_NAME}{NATIVE_HOST_PENDING_RECEIPT_SUFFIX}"))
}

fn current_uid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

fn ensure_safe_directory(path: &Path) -> Result<(), NativeHostError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| NativeHostError::UnsafeDirectory)?;
    if !metadata.is_dir() || metadata.uid() != current_uid() || metadata.mode() & 0o022 != 0 {
        return Err(NativeHostError::UnsafeDirectory);
    }
    Ok(())
}

fn create_private_dir_all(path: &Path) -> Result<(), NativeHostError> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().recursive(true).mode(0o755).create(path).map_err(|_| NativeHostError::Io)
}

fn read_regular(path: &Path) -> Result<Option<Vec<u8>>, NativeHostError> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file() && metadata.uid() == current_uid() && metadata.nlink() == 1 =>
        {
            fs::read(path).map(Some).map_err(|_| NativeHostError::Io)
        }
        Ok(_) => Err(NativeHostError::ForeignManifest),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(NativeHostError::Io),
    }
}

fn read_receipt(directory: &Path) -> Result<Option<Receipt>, NativeHostError> {
    match read_regular(&receipt_path(directory))? {
        None => Ok(None),
        Some(bytes) => decode_receipt(&bytes).map(Some),
    }
}

fn decode_receipt(bytes: &[u8]) -> Result<Receipt, NativeHostError> {
    serde_json::from_slice::<Receipt>(bytes)
        .ok()
        .filter(|receipt| receipt.schema_version == 1)
        .ok_or(NativeHostError::Drift)
}

fn reconcile_pending(directory: &Path) -> Result<(), NativeHostError> {
    let pending = pending_receipt_path(directory);
    let Some(bytes) = read_regular(&pending)? else {
        return Ok(());
    };
    let receipt = decode_receipt(&bytes)?;
    let manifest = directory.join(format!("{NATIVE_HOST_NAME}.json"));
    let final_receipt = read_receipt(directory)?;
    match read_regular(&manifest)? {
        Some(current) if digest(&current) == receipt.manifest_sha256 => {
            replace(&receipt_path(directory), &bytes)?;
            fs::remove_file(&pending).map_err(|_| NativeHostError::Io)?;
            sync_directory(directory)
        }
        Some(current)
            if final_receipt
                .as_ref()
                .is_some_and(|installed| installed.manifest_sha256 == digest(&current)) =>
        {
            // The process died before publishing the new manifest. The
            // previous manifest/receipt pair is still coherent, so discard
            // only our pending marker and let the normal plan retry.
            fs::remove_file(&pending).map_err(|_| NativeHostError::Io)?;
            sync_directory(directory)
        }
        None if final_receipt.is_none() => {
            // Initial publication died before creating the manifest. Nothing
            // was installed, so the pending marker can be safely abandoned.
            fs::remove_file(&pending).map_err(|_| NativeHostError::Io)?;
            sync_directory(directory)
        }
        Some(_) | None => Err(NativeHostError::Drift),
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), NativeHostError> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o644).open(path).map_err(
        |error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                NativeHostError::ConcurrentEdit
            } else {
                NativeHostError::Io
            }
        },
    )?;
    file.write_all(bytes).map_err(|_| NativeHostError::Io)?;
    file.sync_all().map_err(|_| NativeHostError::Io)?;
    let parent = path.parent().ok_or(NativeHostError::Io)?;
    sync_directory(parent)
}

fn replace(path: &Path, bytes: &[u8]) -> Result<(), NativeHostError> {
    let temporary = path.with_extension(format!("ghostrace-tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    write_new(&temporary, bytes)?;
    fs::rename(&temporary, path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        NativeHostError::Io
    })?;
    let parent = path.parent().ok_or(NativeHostError::Io)?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), NativeHostError> {
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| NativeHostError::Io)?;
    directory.sync_all().map_err(|_| NativeHostError::Io)
}
