//! Native production MiniAppsPanel against the isolated ADR Matrix server.
pub use makepad_widgets;
use makepad_widgets::*;
use rinx::miniapps::{MiniAppsAction, MiniAppsPanelWidgetRefExt};
app_main!(App);
script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root { main_window := Window {
            window.inner_size: vec2(1000, 800)
            body +: {flow: Down show_bg: true draw_bg.color: RINX_PAGE
                controls := View {width: Fill height: Fit spacing: 8 padding: 8
                    reopen := RinxButton {text: "Open Palpo"}
                    change_theme := RinxButton {text: "Change theme"}
                    switch_account := RinxButton {text: "Switch test account"}
                }
                app := MiniAppsPanel {}
            }
        }}
    }
}
#[derive(Script, ScriptHook)]
struct App {
    #[live] ui: WidgetRef,
    #[rust] runtime: Option<tokio::runtime::Runtime>,
}
impl App {
    fn open(&self, cx: &mut Cx) {
        self.ui.mini_apps_panel(cx, ids!(app)).action(cx, ModalRef::default(), &MiniAppsAction::OpenPalpo(
            std::env::var("PALPO_ACCEPTANCE_ACTION").unwrap_or_default()));
    }
    fn account(&self, role: &str) {
        self.runtime.as_ref().unwrap().block_on(rinx::miniapps::action_room::install_local_acceptance_session(
            &std::env::var("PALPO_FIXTURE_URL").unwrap(),
            &std::path::PathBuf::from(std::env::var_os("PALPO_ACCEPTANCE_ACCOUNTS").unwrap()),role)).unwrap();
    }
}
impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        assert!(std::env::var_os("RINX_DATA_DIR").is_some(), "isolated profile required");
        self.runtime = Some(tokio::runtime::Runtime::new().unwrap());
        self.account(&std::env::var("PALPO_ACCEPTANCE_ROLE").unwrap_or("manager".into()));
        self.open(cx);
    }
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(reopen)).clicked(actions) { self.open(cx); }
        if self.ui.button(cx, ids!(change_theme)).clicked(actions) {
            rinx::theme::select(cx, rinx::theme::Selection { appearance: rinx::theme::Appearance::Dark, accent: rinx::theme::Accent::Teal }).unwrap();
        }
        if self.ui.button(cx, ids!(switch_account)).clicked(actions) { self.account("owner"); SignalToUI::set_ui_signal(); }
    }
}
impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        rinx::theme::init_standalone(vm);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        article_makepad::apple_fonts::install(vm);
        makepad_widgets::widgets_mod(vm);
        desktop_style::apply_widgets(vm);
        rinx::app::register_widgets(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
