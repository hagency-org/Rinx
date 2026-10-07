//! A verified room projection of the installed Palpo app, with a chat fallback.
use super::{palpo::{ActionsRoomTarget, PalpoHost, APP_ID}, package::Package, ui::MiniAppsPanelWidgetExt, InstanceId, Lease};
use makepad_widgets::*;
use std::{sync::mpsc::{self, Receiver}, time::{Duration, Instant}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    mod.widgets.ActionRoomBoard = #(ActionRoomBoard::register_widget(vm)) {
        visible: false width: Fill height: Fill flow: Down
        bar := View {width: Fill height: Fit padding: 12 spacing: 8 flow: Right align: Align{y: 0.5}
            RinxLabel {width: Fill text: "My Actions"}
            toggle := RinxButton {text: "Chat history"}
        }
        panel := MiniAppsPanel {}
    }
}

#[derive(Script, Widget)]
pub struct ActionRoomBoard {
    #[deref] view: View,
    #[rust] room: Option<ruma::OwnedRoomId>,
    #[rust] lease: Option<Lease>,
    #[rust] host: Option<PalpoHost>,
    #[rust] pending: Option<Receiver<Result<Option<ActionsRoomTarget>, String>>>,
    #[rust] verified: Option<ActionsRoomTarget>,
    #[rust] active: bool,
    #[rust] history: bool,
    #[rust] timer: Timer,
}
impl ScriptHook for ActionRoomBoard {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, _apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        vm.with_cx_mut(|cx| self.present(cx));
    }
}
impl Drop for ActionRoomBoard {
    fn drop(&mut self) {
        if let Some(lease) = self.lease.take() { lease.revoke(); }
    }
}
impl ActionRoomBoard {
    fn reset(&mut self, cx: &mut Cx) {
        if let Some(lease) = self.lease.take() { lease.revoke(); }
        cx.stop_timer(self.timer);
        self.timer = Timer::empty();
        self.pending = None;
        self.verified = None;
        self.host = None;
        self.room = None;
        self.history = false;
        self.active = false;
        self.view.mini_apps_panel(cx, ids!(panel)).close_board(cx);
        self.view.set_visible(cx, false);
    }
    fn select(&mut self, cx: &mut Cx, room: Option<ruma::OwnedRoomId>) {
        self.reset(cx);
        let Some(room) = room else { return };
        let Some(account) = crate::sliding_sync::current_user_id() else { return };
        // Loading and inspecting a bundle is not consent. Probe only after this
        // account has reviewed this exact installed version's requested grants.
        let Ok(package) = Package::load_builtin(APP_ID, &crate::app_data_dir().join("miniapps/imports")) else { return };
        let digest = package.manifest.integrity.bundle_blake3;
        if !super::consent::remembered(account.as_str(), &digest)
            || !package.manifest.capabilities.iter().any(|c| c == "palpo.actions.room.get") { return; }
        let Ok(host) = PalpoHost::new(digest) else { return };
        static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        self.lease = Some(super::AUTHORITY.issue(InstanceId {
            app: APP_ID.into(), account: account.to_string(), room: Some(room.to_string()),
            generation: GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }, ["palpo.actions.room.get".to_owned()].into(), Default::default(), Instant::now() + Duration::from_secs(3600)));
        self.host = Some(host);
        self.room = Some(room);
        self.probe();
    }
    fn probe(&mut self) {
        if self.pending.is_some() { return; }
        let (Some(host), Some(lease), Some(room)) = (self.host.clone(), self.lease.clone(), self.room.clone()) else { return };
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending = Some(rx);
        crate::sliding_sync::spawn_async_task(async move {
            let result = super::palpo::request(host, lease.clone(), "palpo.actions.room.get".into(), serde_json::json!({"roomId":room})).await
                .and_then(|value| if value["room"].is_null() { Ok(None) } else {
                    ActionsRoomTarget::from_reply(&value["room"], &lease.identity().account).map(Some)
                });
            let _ = tx.send(result);
            SignalToUI::set_ui_signal();
        });
    }
    fn present(&mut self, cx: &mut Cx) {
        let active = self.verified.is_some() && !self.history;
        if active && !self.active {
            if self.view.mini_apps_panel(cx, ids!(panel)).open_board(cx, self.verified.clone().unwrap()).is_err() {
                self.reset(cx);
                return;
            }
        } else if !active && self.active {
            // Revokes requests too; returning to the board gets fresh state.
            self.view.mini_apps_panel(cx, ids!(panel)).close_board(cx);
        }
        self.active = active;
        self.view.set_visible(cx, self.verified.is_some());
        self.view.widget(cx, ids!(panel)).set_visible(cx, active);
        self.view.button(cx, ids!(toggle)).set_text(cx, if active { "Chat history" } else { "Action board" });
        self.view.walk.height = if active { Size::fill() } else { Size::fit() };
        cx.redraw_all();
    }
}
impl Widget for ActionRoomBoard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let Some(lease) = self.lease.as_ref() else { return };
        if lease.check(&crate::sliding_sync::current_user_id().map(|u| u.to_string()).unwrap_or_default()).is_err() {
            self.reset(cx);
            return;
        }
        if self.timer.is_event(event).is_some() { self.probe(); }
        let result = self.pending.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Room verification stopped".into())),
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(result) = result {
            self.pending = None;
            match result {
                Ok(Some(target)) => {
                    if self.verified.as_ref().is_some_and(|old| old != &target) {
                        self.reset(cx); return;
                    }
                    self.verified = Some(target);
                    if self.timer.is_empty() { self.timer = cx.start_interval(30.); }
                    self.present(cx);
                }
                _ => { self.reset(cx); cx.redraw_all(); return; }
            }
        }
        if self.verified.is_none() { return; }
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(toggle)).clicked(actions) {
                self.history = !self.history;
                self.present(cx);
            }
        }
        if let Event::BackPressed { handled } = event {
            if self.active {
                self.history = true;
                self.present(cx);
                handled.set(true);
                return;
            }
        }
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl ActionRoomBoardRef {
    pub fn show_history(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.history = true;
            inner.present(cx);
        }
    }
    pub fn select(&self, cx: &mut Cx, room: Option<ruma::OwnedRoomId>) {
        if let Some(mut inner) = self.borrow_mut() { inner.select(cx, room); }
    }
    pub fn active(&self) -> bool { self.borrow().is_some_and(|inner| inner.active) }
}

/// Only the separately built instrument example may install these fixed local
/// identities. This seam is absent from ordinary Rinx builds.
#[cfg(feature = "palpo-instrument")]
pub async fn install_instrument_account(endpoint: &str, account: &str) -> Result<(), String> {
    let url = matrix_sdk::reqwest::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") || url.path() != "/"
        || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err("Instrument account requires the local fixture origin".into());
    }
    let token = match account {
        "@owner:example.test" => "owner-secret",
        "@admin:example.test" => "admin-secret",
        "@coordinator:example.test" => "coordinator-secret",
        _ => return Err("Unknown instrument identity".into()),
    };
    let client = matrix_sdk::Client::builder().homeserver_url(endpoint)
        .server_versions([matrix_sdk::ruma::api::MatrixVersion::V1_0]).build().await.map_err(|e| e.to_string())?;
    client.matrix_auth().restore_session(serde_json::from_value(serde_json::json!({
        "user_id":account, "device_id":"ACTION_BOARD_INSTRUMENT", "access_token":token
    })).map_err(|e| e.to_string())?, Default::default()).await.map_err(|e| e.to_string())?;
    crate::sliding_sync::replace_client(Some(client));
    let package = Package::load_builtin(APP_ID, &crate::app_data_dir().join("miniapps/imports"))?;
    super::consent::remember(account, &package.manifest.integrity.bundle_blake3)
}

/// Restores only an isolated loopback acceptance server's ephemeral account.
/// Compiled out of release applications; credentials never enter Splash.
#[cfg(feature = "palpo-instrument")]
pub async fn install_local_acceptance_session(endpoint: &str, path: &std::path::Path, role: &str) -> Result<String, String> {
    let url = matrix_sdk::reqwest::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") || url.path() != "/"
        || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err("Acceptance requires a loopback homeserver".into());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 65536 { return Err("Invalid acceptance session file".into()); }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 { return Err("Acceptance sessions must be private".into()); }
    }
    let sessions: serde_json::Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|_| "Invalid acceptance sessions")?;
    let session = &sessions[role];
    let account = session["user_id"].as_str().filter(|s| s.ends_with(":rinx-adr0011.test"))
        .ok_or("Acceptance identity must belong to the isolated test server")?.to_owned();
    if !matches!(role, "manager" | "owner" | "coordinator" | "admin" | "notices") {
        return Err("Unknown acceptance role".into());
    }
    // Reusing a device ID with a fresh in-memory crypto store would replace
    // its keys on every harness launch and invalidate real DM recipient proof.
    let store = path.parent().ok_or("Missing acceptance directory")?
        .join("native-acceptance-sdk").join(role);
    std::fs::create_dir_all(&store).map_err(|e| e.to_string())?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        for directory in [store.parent().unwrap(), store.as_path()] {
            let metadata = std::fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
            if !metadata.is_dir() { return Err("Acceptance SDK directory must be real".into()); }
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
    }
    let client = matrix_sdk::Client::builder().homeserver_url(endpoint)
        .sqlite_store_with_config_and_cache_path(matrix_sdk::SqliteStoreConfig::with_low_memory_config(&store), None::<std::path::PathBuf>)
        .with_encryption_settings(matrix_sdk::encryption::EncryptionSettings {
            auto_enable_cross_signing: false,
            auto_enable_backups: false,
            backup_download_strategy: matrix_sdk::encryption::BackupDownloadStrategy::OneShot,
        })
        .build().await.map_err(|e| e.to_string())?;
    client.matrix_auth().restore_session(serde_json::from_value(serde_json::json!({
        "user_id":account, "device_id":session["device_id"], "access_token":session["access_token"]
    })).map_err(|_| "Invalid acceptance session")?, Default::default()).await.map_err(|e| e.to_string())?;
    client.whoami().await.map_err(|e| e.to_string())?;
    client.sync_once(matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(0)))
        .await.map_err(|e| e.to_string())?;
    crate::sliding_sync::replace_client(Some(client));
    let package = Package::load_builtin(APP_ID, &crate::app_data_dir().join("miniapps/imports"))?;
    super::consent::remember(&account, &package.manifest.integrity.bundle_blake3)?;
    Ok(account)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn board_restores_history_layout_and_revokes_on_account_loss() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut board = cx.with_vm(|vm| {
            makepad_widgets::script_mod(vm);
            crate::i18n::install(vm);
            makepad_code_editor::script_mod(vm);
            crate::shared::script_mod(vm);
            vm.bx.captured_errors = Some(Vec::new());
            crate::miniapps::script_mod(vm);
            let value = vm.eval(script!{use mod.prelude.widgets.* use mod.widgets.* ActionRoomBoard{}});
            let board = ActionRoomBoard::script_from_value(vm, value);
            assert!(vm.take_errors().is_empty());
            board
        });
        let target = ActionsRoomTarget::from_reply(&serde_json::json!({"v":1,"revision":1,"purpose":"my_actions",
            "account":"@owner:example.test","botMxid":"@bot:example.test","serverName":"example.test","roomId":"!actions:example.test"}), "@owner:example.test").unwrap();
        board.verified = Some(target);
        board.history = true;
        board.present(&mut cx);
        assert!(!board.active);
        assert!(board.view.visible());
        assert!(!board.view.widget(&mut cx, ids!(panel)).visible());
        let lease = super::super::AUTHORITY.issue(InstanceId { app: APP_ID.into(), account:"@owner:example.test".into(), room:None, generation:1 },
            ["palpo.actions.room.get".into()].into(), Default::default(), Instant::now() + Duration::from_secs(60));
        board.lease = Some(lease.clone());
        board.handle_event(&mut cx, &Event::Signal, &mut Scope::empty());
        assert!(lease.check("@owner:example.test").is_err());
        assert!(!board.view.visible());
        assert!(board.verified.is_none());
    }
}
