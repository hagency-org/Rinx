//! Native credential export. The script receives only Save/Cancel, never bytes.
use rinx_miniapp_core::Lease;

#[cfg(any(target_env = "ohos", all(test, unix)))]
#[path = "private_export_ohos.rs"]
mod ohos;
#[cfg(target_env = "ohos")]
pub(super) use ohos::save;

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(super) async fn save(bytes: Vec<u8>, lease: Lease, account: String) -> Result<bool, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let choose = move || {
        let selected = std::panic::catch_unwind(|| {
            lease.check(&account)?;
            choose_path()
        })
        .unwrap_or_else(|_| Err("Could not open the system save dialog".into()));
        // AppKit dialogs must run on the main queue. File IO must not block it.
        std::thread::spawn(move || {
            let result = selected.and_then(|path| {
                lease.check(&account)?;
                match path {
                    None => Ok(false),
                    Some(path) => {
                        write_private(&path, &bytes, || lease.check(&account)).map(|()| true)
                    }
                }
            });
            let _ = tx.send(result);
        });
    };
    #[cfg(target_os = "macos")]
    on_main_run_loop(choose);
    #[cfg(not(target_os = "macos"))]
    std::thread::spawn(choose);
    rx.await
        .map_err(|_| "Configuration save was interrupted".to_owned())?
}

#[cfg(target_os = "macos")]
fn choose_path() -> Result<Option<std::path::PathBuf>, String> {
    use makepad_widgets::makepad_platform::os::{
        apple::apple_sys::*,
        apple_util::{str_to_nsstring, nsstring_to_string},
    };
    unsafe {
        let panel: ObjcId = msg_send![class!(NSSavePanel), savePanel];
        if panel == nil {
            return Err("Could not open the system save dialog".into());
        }
        let () = msg_send![panel, setCanCreateDirectories: YES];
        let () =
            msg_send![panel, setNameFieldStringValue: str_to_nsstring("hagency-registration.json")];
        let response: i64 = msg_send![panel, runModal];
        match response {
            0 => Ok(None),
            1 => {
                let url: ObjcId = msg_send![panel, URL];
                if url == nil {
                    return Err("The system save dialog returned no destination".into());
                }
                let path: ObjcId = msg_send![url, path];
                if path == nil {
                    return Err("The system save dialog returned no destination".into());
                }
                Ok(Some(nsstring_to_string(path).into()))
            }
            _ => Err("The system save dialog was interrupted".into()),
        }
    }
}

#[cfg(any(
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn choose_path() -> Result<Option<std::path::PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .set_file_name("hagency-registration.json")
        .save_file())
}

#[cfg(target_os = "macos")]
fn on_main_run_loop(choose: impl FnOnce() + Send + 'static) {
    use makepad_widgets::makepad_platform::{makepad_objc_sys::objc_block, os::apple::apple_sys::*};
    // Match Makepad's native document dialogs: NSOperationQueue schedules the
    // modal run loop outside the dispatch callback processing a UI event.
    let callback = std::sync::Mutex::new(Some(choose));
    unsafe {
        let block = objc_block!(move || {
            if let Some(choose) = callback.lock().unwrap().take() {
                choose();
            }
        });
        let queue: ObjcId = msg_send![class!(NSOperationQueue), mainQueue];
        let operation: ObjcId =
            msg_send![class!(NSBlockOperation), blockOperationWithBlock: &block];
        let () = msg_send![queue, addOperation: operation];
    }
}

// Mobile document providers own the destination permissions (including Android
// content URIs). They must receive the bytes directly, outside the app jail.
#[cfg(not(any(
    target_env = "ohos",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(super) async fn save(bytes: Vec<u8>, lease: Lease, account: String) -> Result<bool, String> {
    lease.check(&account)?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let dialog = robius_file_picker::FileDialog::new()
        .set_file_name("hagency-registration.json");
    let on_completion = move |result: robius_file_picker::Result<Option<robius_file_picker::PickedFile>>| {
            let _ = tx.send(
                result
                    .map(|file| file.is_some())
                    .map_err(|_| "Could not save configuration".to_string()),
            );
        };
    #[cfg(target_os = "android")]
    let save = {
        let current_lease = lease.clone();
        let current_account = account.clone();
        dialog.save_data_guarded(bytes, move || current_lease.check(&current_account).is_ok(), on_completion)
    };
    #[cfg(not(target_os = "android"))]
    let save = dialog.save_data(bytes, on_completion);
    save.map_err(|_| "Could not open the system save dialog")?;
    let result = rx
        .await
        .map_err(|_| "Configuration save was interrupted")??;
    lease.check(&account)?;
    Ok(result)
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn write_private(
    path: &std::path::Path,
    bytes: &[u8],
    authorize: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    use std::{fs, io::Write};
    authorize()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("Invalid configuration destination")?;
    // Do not follow a selected symlink or overwrite a device/directory. Replace
    // a regular file atomically: neither its prior mode nor hard links can make
    // the new credentials readable before we set private permissions.
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() => {
            return Err("Choose a regular file for the configuration".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err("Could not inspect configuration destination".into());
        }
        _ => {}
    }
    let temporary = parent.join(format!(".rinx-export-{}", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "Could not create private configuration file")?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|_| "Could not write configuration")?;
        file.sync_all()
            .map_err(|_| "Could not save configuration")?;
        authorize()?;
        fs::rename(&temporary, path).map_err(|_| "Could not replace configuration file")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(all(
    test,
    unix,
    not(any(target_os = "android", target_os = "ios", target_env = "ohos"))
))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    #[test]
    fn private_export_replaces_public_inode_and_preserves_hard_link() {
        let root = std::env::temp_dir().join(format!("rinx-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("profile.json");
        let link = root.join("old.json");
        fs::write(&path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        fs::hard_link(&path, &link).unwrap();
        write_private(&path, b"private", || Ok(())).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(&path).unwrap(), b"private");
        assert_eq!(fs::read(&link).unwrap(), b"old");
        fs::remove_file(&link).unwrap();
        symlink(&path, &link).unwrap();
        assert!(write_private(&link, b"other", || Ok(())).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"private");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn revoked_lease_keeps_old_file_and_removes_temporary_secret() {
        let root = std::env::temp_dir().join(format!("rinx-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("profile.json");
        fs::write(&path, b"old").unwrap();
        let checks = std::cell::Cell::new(0);
        assert!(
            write_private(&path, b"secret", || {
                checks.set(checks.get() + 1);
                if checks.get() == 1 {
                    Ok(())
                } else {
                    Err("revoked".into())
                }
            })
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
