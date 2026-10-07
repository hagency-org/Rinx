//! Offline Makepad instrument harness for the production desktop mini-app host.
//! No account, Matrix client or live backend is started.
pub use makepad_widgets;
use makepad_widgets::*;
use rinx::miniapps::{MiniAppsAction, window::MiniAppsWindowHostWidgetRefExt};
use rinx::theme::{self, Accent, Appearance, Selection};
app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root {
            miniapps := MiniAppsWindowHost {}
            main_window := Window {
                window.inner_size: vec2(1100, 800)
                window.title: "Rinx · Mini-app window test"
                body +: {flow: Down padding: 20 spacing: 12 show_bg: true draw_bg.color: RINX_PAGE
                    RinxPageTitle {text: "Main conversation stays open"}
                    draft := RinxInput {text: "Unsent chat draft"}
                    open := RinxPrimaryButton {text: "Open mini apps"}
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
struct App {
    #[live] ui: WidgetRef,
}
impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let host = self.ui.mini_apps_window_host(cx, ids!(miniapps));
        if self.ui.button(cx, ids!(open)).clicked(actions) {
            host.action(cx, &MiniAppsAction::Open);
        }
        for action in actions {
            if let Some(action) = action.downcast_ref::<MiniAppsAction>() {
                host.action(cx, action);
            }
        }
    }
}
impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        theme::init_standalone(vm);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        article_makepad::apple_fonts::install(vm);
        makepad_widgets::widgets_mod(vm);
        desktop_style::apply_widgets(vm);
        rinx::app::register_widgets(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::Custom(command) = event {
            for (name, appearance) in [("window:dark", Appearance::Dark), ("window:light", Appearance::Light)] {
                if command == name {
                    theme::select(cx, Selection {appearance, accent: Accent::Teal}).unwrap();
                }
            }
            if command == "window:inspect" {
                let host = self.ui.mini_apps_window_host(cx, ids!(miniapps));
                let data = serde_json::json!({
                    "miniapp_window": host.window_id().map(|id| id.id()),
                    "screen_width": cx.display_context.screen_size.x,
                    "chat_draft": self.ui.text_input(cx, ids!(draft)).text(),
                });
                std::fs::write(rinx::app_data_dir().join("inspection.json"), data.to_string()).unwrap();
            }
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
