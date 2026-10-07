//! Actual ActionRoomBoard + MiniAppsPanel + SDK account against a local fixture.
pub use makepad_widgets;
use makepad_widgets::*;
use rinx::miniapps::{action_room::{ActionRoomBoardWidgetRefExt, install_instrument_account}, palpo::{PalpoHost, APP_ID}};
use rinx_miniapp_core::{InstanceId, Lease};
use std::time::{Duration, Instant};
app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root { main_window := Window {
            window.inner_size: vec2(1000, 800)
            body +: {flow: Down show_bg: true draw_bg.color: RINX_PAGE
                controls := View {width: Fill height: Fit padding: 8 spacing: 8
                    switch_account := RinxButton {text: "Switch fixture account"}
                    dark := RinxButton {text: "Change fixture theme"}
                }
                board := ActionRoomBoard {}
                timeline_fallback := RinxLabel {text: "Ordinary chat timeline fallback (fixture)"}
            }
        }}
    }
}
#[derive(Script, ScriptHook)]
struct App {
    #[live] ui: WidgetRef,
    #[rust] runtime: Option<tokio::runtime::Runtime>,
    #[rust] draws: usize,
    #[rust] signals: usize,
}
impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let endpoint = std::env::var("PALPO_FIXTURE_URL").expect("local fixture origin required");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let account = "@coordinator:example.test";
        let room = runtime.block_on(async {
            install_instrument_account(&endpoint, account).await.unwrap();
            let host = PalpoHost::new("a".repeat(64)).unwrap();
            let lease = Lease::new(InstanceId { app: APP_ID.into(), account: account.into(), room: None, generation: 1 },
                ["palpo.actions.room.ensure".into()].into(), Default::default(), Instant::now() + Duration::from_secs(60));
            rinx::miniapps::palpo::request(host, lease, "palpo.actions.room.ensure".into(), serde_json::json!({})).await.unwrap()["roomId"].as_str().unwrap().to_owned()
        });
        self.runtime = Some(runtime);
        self.ui.action_room_board(cx, ids!(board)).select(cx, Some(ruma::RoomId::parse(room).unwrap()));
    }
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(switch_account)).clicked(actions) {
            let endpoint = std::env::var("PALPO_FIXTURE_URL").unwrap();
            self.runtime.as_ref().unwrap().block_on(install_instrument_account(&endpoint, "@owner:example.test")).unwrap();
            SignalToUI::set_ui_signal();
        }
        if self.ui.button(cx, ids!(dark)).clicked(actions) {
            rinx::theme::select(cx, rinx::theme::Selection { appearance: rinx::theme::Appearance::Dark, accent: rinx::theme::Accent::Teal }).unwrap();
        }
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
        if matches!(event, Event::Draw(_)) { self.draws += 1; }
        if matches!(event, Event::Signal) { self.signals += 1; }
        if let Event::Custom(command) = event {
            if command == "palpo:signal" { SignalToUI::set_ui_signal(); }
            if command == "palpo:inspect" {
                std::fs::write(rinx::app_data_dir().join("inspection.json"),
                    serde_json::json!({"draws":self.draws,"signals":self.signals}).to_string()).unwrap();
            }
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
        let active = self.ui.action_room_board(cx, ids!(board)).active();
        self.ui.widget(cx, ids!(timeline_fallback)).set_visible(cx, !active);
    }
}
