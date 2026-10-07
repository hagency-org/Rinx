//! Octoscript mini-app hosting with account-bound Matrix and Octos service access.

pub use rinx_miniapp_core::{InstanceId, Lease, OctosProvider, ServiceEvent};
use std::sync::LazyLock;
static AUTHORITY: LazyLock<rinx_miniapp_core::SessionAuthority> = LazyLock::new(Default::default);

pub fn invalidate_sessions() {
    AUTHORITY.invalidate();
    // Account change or logout: the assistant's contexts of the previous
    // account are revoked too (ADR 0007).
    crate::octos_service::revoke_account();
    // And the assistant's waiting and running calls.
    crate::assistant::invalidate();
}

/// A lease for one assistant request: the app `assistant`, this account,
/// this one room and this one service. The Matrix adapters check it like a
/// mini app's, so an account switch or logout revokes it mid-flight.
pub(crate) fn assistant_lease(account: &str, room: &str, service: &str) -> Lease {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    AUTHORITY.issue(
        InstanceId {
            app: "assistant".into(),
            account: account.into(),
            room: None,
            generation: GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        },
        [service.to_owned()].into(),
        [room.to_owned()].into(),
        std::time::Instant::now() + std::time::Duration::from_secs(120),
    )
}

pub async fn matrix_request(
    lease: Lease,
    service: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = crate::sliding_sync::get_client().ok_or("Not logged in")?;
    crate::host::matrix::execute(client, lease, service, args).await
}


pub mod action_room;
mod catalog_worker;
mod consent;
mod library;
mod package;
pub mod palpo;
mod private_export;
pub mod presentation;
mod sandbox;
pub mod ui;
pub mod window;
pub use crate::host::octos::ContextProvider;
pub use ui::{MiniAppsAction, MiniAppsPanelWidgetRefExt};

// The host already registered the OS faces. Reuse them in each isolate's
// theme families without reopening and reparsing the system TTC on reload.
// Trusted native registration must leave the app's short script budget free.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn install_miniapp_typography(vm: &mut makepad_widgets::ScriptVm) {
    use makepad_widgets::{*, makepad_draw::text::{font::FontId, fonts::Fonts, loader::FontFamilyDefinition}};
    use std::{cell::RefCell, rc::Rc};
    vm.with_cx_mut(CxDraw::lazy_construct_fonts);
    let fonts = vm.with_cx_mut(|cx| cx.get_global::<Rc<RefCell<Fonts>>>().clone());
    if !["PingFangSC-Regular", "PingFangSC-Semibold"].iter().all(|name| fonts.borrow().is_font_known(FontId::from(*name))) {
        return;
    }
    for (value, name) in [
        (script_eval!(vm, { mod.theme.font_label }), "PingFangSC-Regular"),
        (script_eval!(vm, { mod.theme.font_regular }), "PingFangSC-Regular"),
        (script_eval!(vm, { mod.theme.font_bold }), "PingFangSC-Semibold"),
        (script_eval!(vm, { mod.theme.font_italic }), "PingFangSC-Regular"),
        (script_eval!(vm, { mod.theme.font_bold_italic }), "PingFangSC-Semibold"),
    ] {
        let style = TextStyle::script_from_value(vm, value);
        vm.with_cx_mut(|cx| style.ensure_fonts_loaded(cx));
        let family = style.font_family_id();
        let mut fonts = fonts.borrow_mut();
        let fallback = fonts.get_or_load_font_family(family);
        let primary = FontId::from(name);
        let font_ids: Vec<_> = std::iter::once(primary)
            .chain(fallback.fonts().iter().map(|font| font.id()).filter(|id| *id != primary))
            .collect();
        fonts.set_font_family_definition(family, FontFamilyDefinition {
            expected_member_count: font_ids.len(), font_ids, diagnostics: Default::default(),
        });
    }
}

pub fn script_mod(vm: &mut makepad_widgets::ScriptVm) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        makepad_widgets::widget_async::register_splash_isolate_mod(install_miniapp_typography);
        makepad_widgets::widget_async::register_splash_isolate_mod(crate::theme::script_mod);
        makepad_widgets::widget_async::register_splash_isolate_mod(crate::i18n::splash_mod);
        makepad_widgets::widget_async::register_splash_isolate_mod(|vm| {
            octoscript_widgets::design::script_mod(vm);
        });
        makepad_widgets::widget_async::register_splash_isolate_mod(|vm| {
            octoscript_widgets::kit::script_mod(vm);
        });
        makepad_widgets::widget_async::register_splash_isolate_mod(|vm| {
            octoscript_widgets::tap::script_mod(vm);
        });
        makepad_widgets::widget_async::register_splash_isolate_mod(
            makepad_widgets::splash::register_agent_module,
        );
    });
    library::script_mod(vm);
    ui::script_mod(vm);
    action_room::script_mod(vm);
    window::script_mod(vm);
}
