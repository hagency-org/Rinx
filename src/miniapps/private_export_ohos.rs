//! Trusted ArkTS document picker bridge. Credentials stay in Rust, outside Splash.
#[cfg(target_env = "ohos")]
use napi_derive_ohos::napi;
use rinx_miniapp_core::Lease;
use std::{io::Write, os::fd::FromRawFd, sync::Mutex, time::Duration};
use tokio::sync::oneshot;

struct Pending {
    id: String,
    bytes: Vec<u8>,
    lease: Lease,
    account: String,
    shown: bool,
    completed: oneshot::Sender<Result<bool, String>>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

struct CancelOnDrop(String);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Ok(mut pending) = PENDING.lock() {
            if pending.as_ref().is_some_and(|p| p.id == self.0) {
                pending.take();
            }
        }
    }
}

pub(crate) async fn save(bytes: Vec<u8>, lease: Lease, account: String) -> Result<bool, String> {
    lease.check(&account)?;
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    {
        let mut pending = PENDING.lock().map_err(|_| "Save dialog unavailable")?;
        if pending.is_some() { return Err("Another save dialog is already open".into()); }
        *pending = Some(Pending { id: id.clone(), bytes, lease, account, shown: false, completed: tx });
    }
    let _cancel = CancelOnDrop(id);
    tokio::time::timeout(Duration::from_secs(600), rx).await
        .map_err(|_| "Configuration save timed out")?
        .map_err(|_| "Configuration save was interrupted")?
}

/// Only an opaque request ID and suggested filename cross into the native UI.
#[cfg_attr(target_env = "ohos", napi)]
pub fn rinx_take_export() -> String {
    let Ok(mut pending) = PENDING.lock() else { return String::new() };
    let Some(p) = pending.as_mut() else { return String::new() };
    if p.shown { return String::new(); }
    if p.lease.check(&p.account).is_err() {
        if let Some(p) = pending.take() {
            let _ = p.completed.send(Err("Configuration save was cancelled".into()));
        }
        return String::new();
    }
    p.shown = true;
    serde_json::json!({"id":p.id,"name":"hagency-registration.json"}).to_string()
}

#[cfg_attr(target_env = "ohos", napi)]
pub fn rinx_authorize_export(id: String) -> bool {
    PENDING.lock().ok().is_some_and(|pending| pending.as_ref().is_some_and(|p|
        p.id == id && p.shown && p.lease.check(&p.account).is_ok()))
}

/// The URI is opened by the document provider. Never accept a script path or
/// credential bytes; validate the lease again immediately before writing.
#[cfg_attr(target_env = "ohos", napi)]
pub fn rinx_write_export(id: String, fd: i32) -> bool {
    let Ok(pending) = PENDING.lock() else { return false };
    let Some(p) = pending.as_ref().filter(|p| p.id == id && p.shown) else { return false };
    if fd < 0 || p.lease.check(&p.account).is_err() { return false; }
    let duplicate = unsafe { libc::dup(fd) };
    if duplicate < 0 { return false; }
    // Own only the duplicate; the trusted ArkTS caller closes its descriptor.
    let mut file = unsafe { std::fs::File::from_raw_fd(duplicate) };
    file.write_all(&p.bytes).and_then(|()| file.set_len(p.bytes.len() as u64))
        .and_then(|()| file.sync_all()).is_ok()
}

#[cfg_attr(target_env = "ohos", napi)]
pub fn rinx_finish_export(id: String, status: i32) {
    let Ok(mut pending) = PENDING.lock() else { return };
    if !pending.as_ref().is_some_and(|p| p.id == id) { return; }
    let p = pending.take().unwrap();
    let result = p.lease.check(&p.account).and_then(|()| match status {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("Could not save configuration".into()),
    });
    let _ = p.completed.send(result);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rinx_miniapp_core::{InstanceId, SessionAuthority};
    use std::{fs, os::fd::AsRawFd, time::Instant};

    #[tokio::test]
    async fn native_export_checks_account_after_picker_and_cleans_cancelled_work() {
        let authority = SessionAuthority::default();
        let lease = || authority.issue(InstanceId { app: "palpo".into(), account: "manager".into(), room: None, generation: 1 },
            Default::default(), Default::default(), Instant::now() + Duration::from_secs(60));
        async fn request() -> String {
            loop {
                let spec = rinx_take_export();
                if !spec.is_empty() {
                    assert!(!spec.contains("secret"));
                    return serde_json::from_str::<serde_json::Value>(&spec).unwrap()["id"].as_str().unwrap().into();
                }
                tokio::task::yield_now().await;
            }
        }
        let path = std::env::temp_dir().join(format!("rinx-oh-export-{}", uuid::Uuid::new_v4()));
        fs::write(&path, b"original").unwrap();
        let file = fs::OpenOptions::new().read(true).write(true).open(&path).unwrap();
        let task = tokio::spawn(save(b"secret".to_vec(), lease(), "manager".into()));
        let id = tokio::time::timeout(Duration::from_secs(5), request()).await.unwrap();
        assert!(!rinx_write_export("wrong-request".into(), file.as_raw_fd()));
        assert_eq!(fs::read(&path).unwrap(), b"original");
        authority.invalidate();
        assert!(!rinx_authorize_export(id.clone()));
        assert!(!rinx_write_export(id.clone(), file.as_raw_fd()));
        rinx_finish_export(id, 1);
        assert!(task.await.unwrap().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");

        let task = tokio::spawn(save(b"secret".to_vec(), lease(), "manager".into()));
        let id = tokio::time::timeout(Duration::from_secs(5), request()).await.unwrap();
        rinx_finish_export(id, 0);
        assert!(!task.await.unwrap().unwrap());
        assert_eq!(fs::read(&path).unwrap(), b"original");

        let task = tokio::spawn(save(b"secret".to_vec(), lease(), "manager".into()));
        let id = tokio::time::timeout(Duration::from_secs(5), request()).await.unwrap();
        assert!(rinx_authorize_export(id.clone()));
        assert!(rinx_write_export(id.clone(), file.as_raw_fd()));
        rinx_finish_export(id, 1);
        assert!(task.await.unwrap().unwrap());
        assert_eq!(fs::read(&path).unwrap(), b"secret");

        let task = tokio::spawn(save(b"other".to_vec(), lease(), "manager".into()));
        tokio::time::timeout(Duration::from_secs(5), request()).await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(PENDING.lock().unwrap().is_none());
        fs::remove_file(path).unwrap();
    }
}
