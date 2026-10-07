//! Device-local, token-free homeserver history. Only discovery successes are
//! recorded; existing persisted login sessions seed installations upgrading.
use std::{path::Path, sync::Mutex};
use anyhow::Result;
use serde::Deserialize;

const FILE: &str = "homeserver_history.json";
const LIMIT: usize = 20;
static IO: Mutex<()> = Mutex::new(());

fn normalized(server: &str) -> Result<String> {
    super::homeserver::login_server("", Some(server))
}
fn key(server: &str) -> String {
    let url = if server.contains("://") {
        server.to_owned()
    } else {
        format!("https://{server}")
    };
    url::Url::parse(&url)
        .map(|url| url.to_string())
        .unwrap_or(url)
}
fn insert(entries: &mut Vec<String>, server: &str) {
    if let Ok(server) = normalized(server) {
        if !entries.iter().any(|old| key(old) == key(&server)) {
            entries.push(server);
        }
    }
}
fn read(root: &Path) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    match std::fs::read(root.join(FILE)) {
        Ok(bytes) => {
            for server in serde_json::from_slice::<Vec<String>>(&bytes)? {
                insert(&mut entries, &server);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    // Deserialize only the destination; credentials never enter the history.
    #[derive(Deserialize)]
    struct Session {
        client_session: Destination,
    }
    #[derive(Deserialize)]
    struct Destination {
        homeserver: String,
    }
    if let Ok(dirs) = std::fs::read_dir(root) {
        let mut paths: Vec<_> = dirs.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            if let Ok(bytes) = std::fs::read(path.join("persistent_state/session")) {
                if let Ok(session) = serde_json::from_slice::<Session>(&bytes) {
                    insert(&mut entries, &session.client_session.homeserver);
                }
            }
        }
    }
    entries.truncate(LIMIT);
    Ok(entries)
}
pub(crate) fn load(root: &Path) -> Result<Vec<String>> {
    let _guard = IO.lock().unwrap();
    read(root)
}
pub(crate) fn remember(root: &Path, verified_server: &str) -> Result<()> {
    let server = normalized(verified_server)?;
    let _guard = IO.lock().unwrap();
    let mut entries = read(root)?;
    entries.retain(|old| key(old) != key(&server));
    entries.insert(0, server);
    entries.truncate(LIMIT);
    std::fs::create_dir_all(root)?;
    let destination = root.join(FILE);
    let temporary = root.join(format!(".homeserver-history-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&serde_json::to_vec(&entries)?)?;
        file.sync_all()?;
        std::fs::rename(&temporary, &destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
pub(crate) fn filtered(entries: &[String], query: &str) -> Vec<String> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|server| server.to_lowercase().contains(&query))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("rinx-server-history-{}", uuid::Uuid::new_v4())))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn remembers_deduplicates_and_limits_most_recent_destinations_across_reload() {
        let f = Fixture::new();
        for i in 0..25 {
            remember(&f.0, &format!("s{i}.example.org")).unwrap();
        }
        remember(&f.0, " HTTPS://s20.example.org:443/ ").unwrap();
        let entries = load(&f.0).unwrap();
        assert_eq!(entries.len(), LIMIT);
        assert_eq!(entries[0], "https://s20.example.org/");
        assert_eq!(
            entries
                .iter()
                .filter(|s| key(s) == key("s20.example.org"))
                .count(),
            1
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(f.0.join(FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn unsafe_destinations_do_not_replace_existing_history() {
        let f = Fixture::new();
        remember(&f.0, "matrix.org").unwrap();
        let before = std::fs::read(f.0.join(FILE)).unwrap();
        for server in [
            "https://user:secret@example.org",
            "https://example.org?token=secret",
            "javascript://host",
            "foo bar",
        ] {
            assert!(remember(&f.0, server).is_err());
        }
        assert_eq!(std::fs::read(f.0.join(FILE)).unwrap(), before);
        assert_eq!(
            filtered(&load(&f.0).unwrap(), " MATRIX "),
            vec!["matrix.org"]
        );
    }
    #[test]
    fn existing_sessions_seed_destinations_without_credentials() {
        let f = Fixture::new();
        let session = f.0.join("old_user/persistent_state");
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(session.join("session"), r#"{"client_session":{"homeserver":"http://localhost:8008","passphrase":"secret"},"user_session":{"access_token":"secret"}}"#).unwrap();
        assert_eq!(load(&f.0).unwrap(), vec!["http://localhost:8008/"]);
        remember(&f.0, "matrix.org").unwrap();
        let saved = std::fs::read_to_string(f.0.join(FILE)).unwrap();
        assert!(!saved.contains("secret"));
        assert!(!saved.contains("access_token"));
    }
    #[test]
    fn corrupt_history_is_not_silently_overwritten() {
        let f = Fixture::new();
        std::fs::create_dir_all(&f.0).unwrap();
        std::fs::write(f.0.join(FILE), "broken").unwrap();
        assert!(remember(&f.0, "matrix.org").is_err());
        assert_eq!(std::fs::read_to_string(f.0.join(FILE)).unwrap(), "broken");
    }
}
