//! Handles app persistence by saving and restoring client session data to/from the filesystem.

use std::path::PathBuf;
use anyhow::{anyhow, bail};
use makepad_widgets::{log, Cx};
use matrix_sdk::{
    authentication::matrix::MatrixSession,
    ruma::{OwnedUserId, UserId},
    sliding_sync,
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_data_dir,
    cache_dir,
    login::login_screen::LoginAction,
};

/// The data needed to re-build a client.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClientSessionPersisted {
    /// The URL of the homeserver of the user.
    pub homeserver: String,

    /// The database path. New sessions store this as a relative subfolder
    /// (joined with `app_data_dir()` at restore time); legacy sessions
    /// may have an absolute path. `restore_session` handles both.
    pub db_path: PathBuf,

    /// The passphrase of the database.
    pub passphrase: String,
}

impl std::fmt::Debug for ClientSessionPersisted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientSessionPersisted")
            .field("homeserver", &self.homeserver)
            .field("db_path", &self.db_path)
            .field("passphrase", &"<REDACTED>")
            .finish()
    }
}

/// The full session to persist.
#[derive(Debug, Serialize, Deserialize)]
pub struct FullSessionPersisted {
    /// The data to re-build the client.
    pub client_session: ClientSessionPersisted,

    /// The Matrix user session.
    pub user_session: MatrixSession,

    /// The latest sync token.
    ///
    /// It is only needed to persist it when using `Client::sync_once()` and we
    /// want to make our syncs faster by not receiving all the initial sync
    /// again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,

    /// The sliding sync version to use for this client session.
    /// 
    /// This determines the sync protocol used by the Matrix client:
    /// - `Native`: Uses the server's native sliding sync implementation for efficient syncing
    /// - `None`: Falls back to standard Matrix sync (without sliding sync optimizations)
    /// 
    /// The value is restored and applied to the client via `client.set_sliding_sync_version()`
    /// when rebuilding the session from persistent storage.
    #[serde(default)]
    pub sliding_sync_version: SlidingSyncVersion,
}

/// A serializable duplicate of [`sliding_sync::Version`].
#[derive(Debug, Default, Serialize, Deserialize)]
pub enum SlidingSyncVersion {
    #[default]
    Native,
    None,
}
impl From<SlidingSyncVersion> for sliding_sync::Version {
    fn from(version: SlidingSyncVersion) -> Self {
        match version {
            SlidingSyncVersion::None => sliding_sync::Version::None,
            SlidingSyncVersion::Native => sliding_sync::Version::Native,
        }
    }
}
impl From<sliding_sync::Version> for SlidingSyncVersion {
    fn from(version: sliding_sync::Version) -> Self {
        match version {
            sliding_sync::Version::None => SlidingSyncVersion::None,
            sliding_sync::Version::Native => SlidingSyncVersion::Native,
        }
    }
}

/// Returns a collision-free directory keyed by the complete Matrix user ID.
/// Legacy directories are validated and migrated by `accounts::refresh`.
pub fn persistent_state_dir(user_id: &UserId) -> PathBuf {
    crate::accounts::account_dir(app_data_dir(), user_id).join("persistent_state")
}

/// Returns the path to the session file for the given user.
pub fn session_file_path(user_id: &UserId) -> PathBuf {
    persistent_state_dir(user_id).join("session")
}

const LATEST_USER_ID_FILE_NAME: &str = "latest_user_id.txt";

/// Returns the user ID of the most recently-logged in user session.
pub async fn most_recent_user_id() -> Option<OwnedUserId> {
    tokio::fs::read_to_string(
        app_data_dir().join(LATEST_USER_ID_FILE_NAME)
    )
    .await
    .ok()?
    .trim()
    .try_into()
    .ok()
}

/// Resolves the path that `restore_session()` would actually open.
pub(crate) fn resolve_db_path(stored: PathBuf) -> PathBuf {
    if !stored.is_absolute() {
        return app_data_dir().join(stored);
    }
    if stored.exists() {
        return stored;
    }
    let Some(name) = stored.file_name() else {
        return stored;
    };
    // iOS sandbox UUID changes across reinstalls; the absolute path
    // baked into the session is now stale. Use the basename instead.
    app_data_dir().join(name)
}

/// Returns the set of `db` paths referenced by any saved session file.
///
/// This basically scans every saved user session dir, not just the most recent one,
/// to help ensure that db dirs don't get orphaned on the filesystem forever.
///
/// Returns `None` if the app data directory can't be accessed,
/// which means that nothing should be considered as eligible for deletion.
async fn collect_referenced_db_paths() -> Option<std::collections::HashSet<PathBuf>> {
    use std::collections::HashSet;
    let mut paths = HashSet::new();
    let data_dir = app_data_dir();

    let mut entries = match tokio::fs::read_dir(data_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            log!("collect_referenced_db_paths: could not read data dir {}: {e}", data_dir.display());
            return None;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("db_") {
            continue;
        }
        let session_file = path.join("persistent_state").join("session");
        let retained = path.join("persistent_state/retained_db.json");
        match tokio::fs::read(retained).await {
            Ok(bytes) => {
                let Ok(sessions) = serde_json::from_slice::<Vec<ClientSessionPersisted>>(&bytes) else { return None; };
                for session in sessions { paths.insert(resolve_db_path(session.db_path)); }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return None,
        }
        let bytes = match tokio::fs::read(&session_file).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        };
        let session: FullSessionPersisted = match serde_json::from_slice(&bytes) {
            Ok(s) => s,
            Err(e) => {
                log!("collect_referenced_db_paths: skipping unparsable session file {}: {e}",
                    session_file.display(),
                );
                return None;
            }
        };
        paths.insert(resolve_db_path(session.client_session.db_path));
    }

    Some(paths)
}

/// Deletes old database files that start with `"db_"` within the given `dir` and its subdirectories.
///
/// Only deletes database files that are inactive, i.e., where `is_active` returns false.
async fn prune_orphan_db_dirs(dir: &std::path::Path, is_active: impl Fn(&std::ffi::OsStr) -> bool) {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            log!("prune_orphan_db_dirs: could not read {}: {e}", dir.display());
            return;
        }
    };

    let mut deleted: usize = 0;
    let mut bytes_freed: u64 = 0;
    let mut kept: usize = 0;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.starts_with("db_") {
            continue;
        }
        if is_active(name) {
            kept += 1;
            continue;
        }
        let size = dir_size_bytes(&path).await.unwrap_or(0);
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => {
                deleted += 1;
                bytes_freed += size;
                // log!(
                //     "prune_orphan_db_dirs: deleted orphaned dir ({size} bytes): {}",
                //     path.display(),
                // );
            }
            Err(e) => {
                log!("prune_orphan_db_dirs: failed to delete {}: {e}", path.display());
            }
        }
    }

    if deleted > 0 || kept > 0 {
        log!("prune_orphan_db_dirs ({}): deleted {deleted} orphan(s), freed {bytes_freed} bytes; kept {kept} active", dir.display());
    }
}

/// Deletes orphaned (no longer used) database and cache directories.
pub async fn cleanup_orphan_db_dirs() {
    use std::collections::HashSet;
    use std::ffi::OsString;

    // If we couldn't read the data directory, we can't know which ones are active, so skip pruning.
    let Some(active) = collect_referenced_db_paths().await else {
        return;
    };
    let active_names: HashSet<OsString> = active
        .iter()
        .filter_map(|p| p.file_name().map(ToOwned::to_owned))
        .collect();

    let data_dir = app_data_dir();
    prune_orphan_db_dirs(data_dir, |name| active.contains(&data_dir.join(name))).await;
    prune_orphan_db_dirs(cache_dir(), |name| active_names.contains(name)).await;
}

/// Recursive size sum, best-effort. Just for the cleanup log line.
async fn dir_size_bytes(path: &std::path::Path) -> Option<u64> {
    let mut total = 0u64;
    let mut entries = tokio::fs::read_dir(path).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(md) = entry.metadata().await else {
            continue;
        };
        if md.is_file() {
            total = total.saturating_add(md.len());
        } else if md.is_dir() {
            // matrix-sdk-sqlite doesn't nest subdirectories, but be safe.
            if let Some(sub) = Box::pin(dir_size_bytes(&entry.path())).await {
                total = total.saturating_add(sub);
            }
        }
    }
    Some(total)
}

/// Save which user was the most recently logged in.
async fn save_latest_user_id(user_id: &UserId) -> anyhow::Result<()> {
    tokio::fs::write(
        app_data_dir().join(LATEST_USER_ID_FILE_NAME),
        user_id.as_str(),
    ).await?;
    Ok(())
}


/// Restores the given user's previous session from the filesystem.
///
/// If no User ID is specified, the ID of the most recently-logged in user
/// is retrieved from the filesystem.
pub async fn restore_session(
    user_id: Option<OwnedUserId>
) -> anyhow::Result<(Client, Option<String>)> {
    let user_id = if let Some(user_id) = user_id {
        Some(user_id)
    } else {
        most_recent_user_id().await
    };

    let Some(user_id) = user_id else {
        log!("Could not find previous latest User ID");
        bail!("Could not find previous latest User ID");
    };
    let session_file = session_file_path(&user_id);
    if !session_file.exists() {
        log!("Could not find previous session file for user {user_id}");
        bail!("Could not find previous session file");
    }
    let status_str = format!("Loading previous session file for {user_id}...");
    log!("{status_str}: '{}'", session_file.display());
    Cx::post_action(LoginAction::Status {
        title: "Restoring session".into(),
        status: status_str,
    });

    // The session was serialized as JSON in a file.
    let serialized_session = tokio::fs::read_to_string(session_file).await?;
    let FullSessionPersisted { client_session, user_session, sync_token, sliding_sync_version } =
        serde_json::from_str(&serialized_session)?;

    if user_session.meta.user_id != user_id { bail!("Saved session belongs to another account"); }

    let status_str = format!(
        "Loaded session file for:\n{user_id}\n\nTrying to connect to homeserver...\n{}",
        client_session.homeserver,
    );
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Connecting to homeserver".into(),
        status: status_str,
    });
    let original_stored = client_session.db_path.clone();
    let db_path = resolve_db_path(client_session.db_path);
    if db_path != original_stored {
        log!(
            "Stored db_path '{}' relocated to '{}'",
            original_stored.display(),
            db_path.display(),
        );
    }
    log!(
        "Restoring session for {user_id} with db at: {} (stored as: {})",
        db_path.display(),
        original_stored.display(),
    );
    let client = crate::sliding_sync::base_client_builder(&db_path, &client_session.passphrase)
        .homeserver_url(client_session.homeserver)
        .build()
        .await?;
    client.set_sliding_sync_version(sliding_sync_version.into());
    let status_str = format!("Authenticating previous login session for {}...", user_session.meta.user_id);
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Authenticating session".into(),
        status: status_str,
    });

    // Restore the Matrix user session.
    client.restore_session(user_session).await?;
    save_latest_user_id(&user_id).await?;

    Ok((client, sync_token))
}

/// Persist a logged-in client session to the filesystem for later use.
///
/// TODO: This is not very secure, for simplicity. We should use robius-keychain
///       or `keyring-rs` to storing secrets securely.
///
/// Note that we could also build the user session from the login response.
pub async fn save_session(
    client: &Client,
    client_session: ClientSessionPersisted,
) -> anyhow::Result<()> {
    let user_session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| anyhow!("A logged-in client should have a session"))?;

    let user_id = user_session.meta.user_id.clone();
    let sliding_sync_version = client.sliding_sync_version().into();
    // Save that user's session.
    let session_file = session_file_path(&user_session.meta.user_id);
    let serialized_session = serde_json::to_string(&FullSessionPersisted {
        client_session,
        user_session,
        sync_token: None,
        sliding_sync_version
    })?;
    if let Some(parent) = session_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).await?;
            if let Some(account) = parent.parent() { tokio::fs::set_permissions(account, std::fs::Permissions::from_mode(0o700)).await?; }
        }
    }
    write_private_session(&session_file, serialized_session.as_bytes()).await?;
    save_latest_user_id(&user_id).await?;

    crate::accounts::refresh()?;
    log!("Session persisted to: {}", session_file.display());
    Ok(())
}

/// Atomically replace the token-bearing session with owner-only permissions.
pub(crate) async fn write_private_session(path: &std::path::Path, data: &[u8]) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let temporary = path.with_extension(format!("{:016x}.tmp", rand::random::<u64>()));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary).await?;
    let result = async {
        file.write_all(data).await?;
        file.sync_all().await?;
        tokio::fs::rename(&temporary, path).await
    }.await;
    if result.is_err() { let _ = tokio::fs::remove_file(&temporary).await; }
    result?;
    Ok(())
}

/// Refresh tokens rotate. Persist both tokens before the next app restart.
pub async fn save_refreshed_session(client: &Client) -> anyhow::Result<()> {
    let session = client.matrix_auth().session()
        .ok_or_else(|| anyhow!("No Matrix session to persist"))?;
    let path = session_file_path(&session.meta.user_id);
    // Logout may already have removed this session. Never recreate it here.
    let mut stored: FullSessionPersisted = serde_json::from_slice(&tokio::fs::read(&path).await?)?;
    if stored.user_session.meta != session.meta { bail!("Session changed while refreshing tokens"); }
    stored.user_session = session;
    write_private_session(&path, &serde_json::to_vec(&stored)?).await
}

/// Remove the LATEST_USER_ID_FILE_NAME file if it exists
/// 
/// Returns:
/// - Ok(true) if file was found and deleted
/// - Ok(false) if file didn't exist
/// - Err if deletion failed
pub async fn delete_latest_user_id() -> anyhow::Result<bool> {
    let last_login_path = app_data_dir().join(LATEST_USER_ID_FILE_NAME);
    
    if last_login_path.exists() {
        tokio::fs::remove_file(&last_login_path).await
            .map_err(|e| anyhow::anyhow!("Failed to remove latest user file: {e}"))
            .map(|_| true)
    } else {
        Ok(false)
    }
}
