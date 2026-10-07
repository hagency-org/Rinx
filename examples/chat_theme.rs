//! Offline visual review of the production chat widgets. No Matrix connection.
use makepad_widgets::*;
use std::collections::HashMap;
use rinx::{
    home::link_preview::{LinkPreviewCache, LinkPreviewData, LinkPreviewWidgetRefExt},
    media_cache::MediaCache,
    shared::{avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt},
    theme::{self, Accent, Appearance, Selection},
};

app_main!(App);

fn narrow() -> bool {
    std::env::args().any(|a| a == "--narrow")
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.inner_size: #(if narrow() {dvec2(430., 820.)} else {dvec2(1000., 800.)})
                window.title: "Rinx · Chat appearance review"
                body +: {flow: Down show_bg: true draw_bg.color: RINX_PAGE
                    header := SolidView {
                        width: Fill height: Fit flow: Down padding: 20 spacing: 4
                        draw_bg.color: RINX_SURFACE
                        RinxPageTitle {text: "Design review"}
                        RinxHint {text: "Emma, Leo and you"}
                    }
                    timeline := Timeline {
                        list +: {
                            auto_tail: false
                            padding: Inset{top: 16 bottom: 16}
                            OwnFixture := Message {body.content.draw_bg.color: RINX_OUTGOING}
                        }
                    }
                    desktop_input := RoomInputBar {visible: #(!narrow())}
                    mobile_input := MobileRoomInputBar {visible: #(narrow())}
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    populated: HashMap<usize, u64>,
}

impl MatchEvent for App {
    fn handle_draw(&mut self, cx: &mut Cx, event: &DrawEvent) {
        let mut draw = CxDraw::new(cx, event);
        let mut cx = Cx2d::new(&mut draw);
        let revision = theme::snapshot(&mut cx).revision;
        while let Some(widget) = self.ui.draw(&mut cx, &mut Scope::empty()).step() {
            let Some(mut list) = widget.borrow_mut::<PortalList>() else {
                continue;
            };
            list.set_item_range(&mut cx, 0, 5);
            while let Some(index) = list.next_visible_item(&mut cx) {
                let own = matches!(index, 1 | 4);
                let template = if narrow() {
                    if own {
                        id!(MobileOwnMessage)
                    } else {
                        id!(MobileMessage)
                    }
                } else if own {
                    id!(OwnFixture)
                } else if index == 3 {
                    id!(CondensedMessage)
                } else {
                    id!(Message)
                };
                let row = list.item(&mut cx, index, template);
                if self.populated.get(&index) == Some(&revision) {
                    row.draw_all(&mut cx, &mut Scope::empty());
                    continue;
                }
                let (name, id, text) = match index {
                    0 => ("Emma", "@emma:example.org", "The updated chat screens are ready to review. Can you check the message spacing and attachment preview?"),
                    1 => ("You", "@you:example.org", "Yes. Incoming messages and my replies should be easy to distinguish, in both light and dark mode."),
                    2 => ("Leo", "@leo:example.org", "Here is the review document. 中文内容也需要清楚易读，长句应该自然换行。"),
                    3 => ("Leo", "@leo:example.org", "review-notes.txt  ·  2.2 KB"),
                    _ => ("You", "@you:example.org", "Downloaded, thank you. I’ll add my feedback after reviewing the screenshots."),
                };
                row.html_or_plaintext(&mut cx, ids!(content.message))
                    .show_plaintext(&mut cx, text);
                row.label(&mut cx, ids!(username)).set_text(&mut cx, name);
                let user: ruma::OwnedUserId = id.try_into().unwrap();
                row.avatar(&mut cx, ids!(profile.avatar))
                    .show_user_text(&mut cx, &user, name);
                for path in [
                    ids!(replied_to_message),
                    ids!(mobile_reply_preview),
                    ids!(thread_root_summary),
                    ids!(edited_indicator),
                    ids!(tsp_sign_indicator),
                ] {
                    row.widget(&mut cx, path).set_visible(&mut cx, false);
                }
                row.view(&mut cx, ids!(download_section))
                    .set_visible(&mut cx, index == 3);
                if index == 2 {
                    let url = "https://example.org/design-review".parse().unwrap();
                    let mut cache = LinkPreviewCache::new(None);
                    cache.insert(&url, LinkPreviewData {
                        title: Some("Chat design review".into()),
                        description: Some("Message surfaces, typography and readable attachments across desktop and mobile layouts.".into()),
                        site_name: Some("Project notes".into()),
                        ..Default::default()
                    });
                    row.link_preview(&mut cx, ids!(content.link_preview_view))
                        .populate_below_message(
                            &mut cx,
                            &[url],
                            &mut MediaCache::new(None),
                            &mut cache,
                            &|_, _, _, _, _, _| true,
                        );
                }
                self.populated.insert(index, revision);
                row.draw_all(&mut cx, &mut Scope::empty());
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
            let mut selection = theme::selection(cx).unwrap_or_default();
            match command.as_str() {
                "theme:light" => selection.appearance = Appearance::Light,
                "theme:dark" => selection.appearance = Appearance::Dark,
                "theme:teal" => selection.accent = Accent::Teal,
                "theme:violet" => selection.accent = Accent::Violet,
                command if command.starts_with("theme:preset:") => {
                    let index: usize = command.trim_start_matches("theme:preset:").parse().unwrap();
                    let preferences = theme::packages::current(cx).unwrap();
                    let preferences = theme::presets::select(index, preferences).unwrap();
                    theme::packages::apply(cx, preferences).unwrap();
                    return;
                }
                "theme:inspect" => {
                    let palette = theme::snapshot(cx);
                    let colors: serde_json::Map<String, serde_json::Value> = [
                        "color.surface.page",
                        "color.chat.incoming",
                        "color.chat.outgoing",
                        "color.content.primary",
                        "color.content.secondary",
                    ]
                    .into_iter()
                    .map(|key| {
                        (
                            key.into(),
                            format!("#{:06x}", theme::argb(palette.role(key)) & 0xffffff).into(),
                        )
                    })
                    .collect();
                    std::fs::write(
                        rinx::app_data_dir().join("chat-theme.json"),
                        serde_json::to_vec_pretty(&colors).unwrap(),
                    )
                    .unwrap();
                    return;
                }
                _ => return,
            }
            theme::select(cx, Selection { ..selection }).unwrap();
        }
        self.match_event(cx, event);
        if !matches!(event, Event::Draw(_)) {
            self.ui.handle_event(cx, event, &mut Scope::empty());
        }
        theme::packages::after_event(cx, event);
    }
}
