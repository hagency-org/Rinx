//! Offline production settings/composer regression. Never logs in or sends messages.
use makepad_widgets::*;
use rinx::{app::AppState, room::room_input_bar::RoomInputBarWidgetRefExt,
    settings::theme_studio::{ThemeStudioAction, ThemeStudioWidgetRefExt},
    miniapps::{MiniAppsAction, window::MiniAppsWindowHostWidgetRefExt}};
use matrix_sdk::ruma::{events::room::message::RoomMessageEventContent, room_id};
app_main!(App);
script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root {
            miniapps := MiniAppsWindowHost {}
            main_window := Window {
                window.inner_size: #(if std::env::args().any(|a| a == "--narrow") {dvec2(390.,844.)} else {dvec2(1000.,850.)})
                window.title: "Rinx settings parity · offline"
                body +: {flow: Overlay
                    content := SolidView {width: Fill height: Fill flow: Down draw_bg.color: RINX_PAGE
                        settings := SettingsScreen {}
                        isolated := Splash {width: Fill height: 40}
                        desktop_composer := RoomInputBar {}
                        mobile_composer := MobileRoomInputBar {}
                    }
                    // Keep the production tree instantiated, including inactive pages.
                    production := RinxContent {visible: false
                        overlay_container +: {home_screen_view.visible: true login_screen_view.visible: false}
                    }
                    fixture_theme_studio_modal := Modal {can_dismiss: false content := ThemeStudio {}}
                }
            }
        }
    }
}
#[derive(Script, ScriptHook)]
struct App {
    #[live] ui: WidgetRef,
    #[rust] state: AppState,
}
impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.state.selected_tab = rinx::home::navigation_tab_bar::SelectedTab::Settings;
        self.draft(cx);
        self.ui.splash(cx, ids!(isolated)).set_text(cx, "width: Fill height: Fill\nLabel {text: \"Mini app keeps its own language\"}");
    }
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(action) = action.downcast_ref::<ThemeStudioAction>() {
                self.ui.theme_studio(cx, ids!(fixture_theme_studio_modal.content)).action(
                    cx, self.ui.modal(cx, ids!(fixture_theme_studio_modal)), action);
            }
        }
    }
}
impl App {
    fn draft(&self, cx: &mut Cx) {
        let kind = rinx::sliding_sync::TimelineKind::MainRoom {room_id: room_id!("!offline:example.org").into()};
        for path in [ids!(desktop_composer), ids!(mobile_composer)] {
            self.ui.widget(cx, path).as_room_input_bar().restore_unsent_message(cx,
                &RoomMessageEventContent::text_plain("Unsent 中文 draft"), None, &kind);
        }
    }
    fn inspect(&self, cx: &mut Cx) {
        let buttons: Vec<_> = [ids!(desktop_composer.send_message_button), ids!(mobile_composer.send_message_button)].into_iter().map(|p| {
            let b = self.ui.button(cx, p);
            let b = b.borrow().unwrap();
            serde_json::json!({"text":b.text(), "size":format!("{:?}", b.area().rect(cx).size), "icon":format!("{:?}", b.draw_icon.rect_size), "svg":format!("{:?}", b.draw_icon.svg.as_ref().map(|s| s.as_handle()))})
        }).collect();
        let fields: Vec<_> = [ids!(desktop_composer.text_input), ids!(mobile_composer.text_input)].into_iter().map(|p| self.ui.text_input(cx,p).text()).collect();
        let report = serde_json::json!({"selection":rinx::theme::selection(cx), "buttons":buttons,"drafts":fields,
            "mode_index":self.ui.drop_down(cx,ids!(settings.mode)).selected_item(),
            "follow_system":rinx::theme::packages::current(cx).unwrap().follow_system,
            "isolate":self.ui.splash(cx,ids!(isolated)).isolate_heap_key(cx)});
        std::fs::write(rinx::app_data_dir().join("inspection.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
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
        rinx::theme::system::handle_event(cx,event);
        if let Event::Custom(command) = event {
            match command.as_str() {
                "inspect" => self.inspect(cx),
                "gc" => cx.with_vm(|vm| vm.gc()),
                "production:init" => {
                    self.state.selected_tab = rinx::home::navigation_tab_bar::SelectedTab::Home;
                    self.ui.view(cx,ids!(production)).set_visible(cx,true);
                    self.ui.view(cx,ids!(content)).set_visible(cx,false);
                }
                "production:hide" => {
                    self.state.selected_tab = rinx::home::navigation_tab_bar::SelectedTab::Settings;
                    self.ui.view(cx,ids!(production)).set_visible(cx,false);
                    self.ui.view(cx,ids!(content)).set_visible(cx,true);
                }
                "miniapps:open" => self.ui.mini_apps_window_host(cx, ids!(miniapps)).action(cx,&MiniAppsAction::Open),
                "miniapps:close" => self.ui.mini_apps_window_host(cx, ids!(miniapps)).action(cx,&MiniAppsAction::Close),
                "encrypt" => for p in [ids!(desktop_composer),ids!(mobile_composer)] {
                    self.ui.widget(cx,p).as_room_input_bar().update_encryption_state(cx,true);
                },
                _ => {}
            }
        }
        self.match_event(cx,event);
        self.ui.handle_event(cx,event,&mut Scope::with_data(&mut self.state));
        rinx::theme::packages::after_event(cx,event);
        if matches!(event, Event::LiveEdit) { rinx::i18n::refresh_ui(cx, &self.ui); }
    }
}
