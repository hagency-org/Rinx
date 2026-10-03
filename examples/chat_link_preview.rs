//! Offline link-card fixture. All metadata is local; no Matrix login or HTTP requests.
use makepad_widgets::*;
use rinx::home::link_preview::{LinkPreviewCache, LinkPreviewData, LinkPreviewWidgetRefExt};
use rinx::media_cache::MediaCache;
use rinx::shared::html_or_plaintext::HtmlOrPlaintextWidgetRefExt;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.inner_size: vec2(1200, 900)
                window.title: "Rinx · offline link preview"
                body +: {
                    flow: Down padding: 24 spacing: 12
                    Label {text: "Link preview fixture"}
                    card := LinkPreview {width: 320}
                    after := Label {text: "After the cards"}
                    controls := View {
                        width: Fill height: Fit flow: Right spacing: 8
                        replace := Button {text: "Replace links"}
                        narrow := Button {text: "Narrow card"}
                        empty := Button {text: "Empty preview"}
                        clear := Button {text: "Clear links"}
                        layouts := Button {text: "Message layouts"}
                        inspect := Button {text: "Inspect layout"}
                    }
                    result := Label {width: Fill height: Fit text: "Ready"}
                    layout_cases := View {
                        visible: false width: Fill height: Fit flow: Down spacing: 12
                        desktop := Message {}
                        compact := CondensedMessage {}
                        own := MobileOwnMessage {}
                        narrow_row := View {
                            width: 360 height: Fit
                            mobile := MobileMessage {}
                        }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
struct App {
    #[live] ui: WidgetRef,
}

impl App {
    fn populate(&self, cx: &mut Cx, replacement: bool) {
        let mut cache = LinkPreviewCache::new(None);
        let mut media = MediaCache::new(None);
        let links: Vec<url::Url> = if replacement {
            vec!["https://example.org/replacement".parse().unwrap()]
        } else {
            (1..=3).map(|i| format!("https://example.org/{i}").parse().unwrap()).collect()
        };
        for (i, url) in links.iter().enumerate() {
            cache.insert(url, LinkPreviewData {
                title: Some(if replacement { "Replacement card".into() } else { format!("Preview title {}", i + 1) }),
                site_name: Some("Example site".into()),
                description: Some("A readable description that wraps inside the card, including 中文 and a long line of text.".into()),
                image: (!replacement && i == 0).then(|| "mxc://example.org/fixture".into()),
                url: Some("https://example.org/metadata-canonical-url".into()),
                ..Default::default()
            });
        }
        self.ui.link_preview(cx, ids!(card)).populate_below_message(
            cx, &links, &mut media, &mut cache, &|cx, image, _, source, _, _| {
                image.show_image(cx, Some(source), |cx, image| {
                    image.load_png_from_data(cx, include_bytes!("../resources/icon_64.png"))?;
                    Ok::<_, makepad_widgets::image_cache::ImageError>((64, 64))
                }).unwrap();
                true
            },
        );
        self.ui.redraw(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.populate(cx, false);
        let text = "Long chat messages should wrap comfortably and leave space beside the text. ".repeat(3);
        for path in [ids!(desktop), ids!(compact), ids!(own), ids!(mobile)] {
            let row = self.ui.widget(cx, path);
            row.html_or_plaintext(cx, ids!(content.message)).show_plaintext(cx, &text);
            row.label(cx, ids!(username)).set_text(cx, "Sender");
            row.view(cx, ids!(replied_to_message)).set_visible(cx, false);
            row.view(cx, ids!(download_section)).set_visible(cx, false);
            row.view(cx, ids!(thread_root_summary)).set_visible(cx, false);
        }
    }
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(replace)).clicked(actions) { self.populate(cx, true); }
        if self.ui.button(cx, ids!(clear)).clicked(actions) {
            self.ui.link_preview(cx, ids!(card)).clear(cx);
        }
        if self.ui.button(cx, ids!(narrow)).clicked(actions) {
            let mut card = self.ui.link_preview(cx, ids!(card));
            script_apply_eval!(cx, card, {width: 240});
            self.ui.redraw(cx);
        }
        if self.ui.button(cx, ids!(empty)).clicked(actions) {
            let link = "https://example.org/empty".parse().unwrap();
            let mut cache = LinkPreviewCache::new(None);
            cache.insert(&link, LinkPreviewData {title: Some("  \n ".into()), ..Default::default()});
            self.ui.link_preview(cx, ids!(card)).populate_below_message(
                cx, &[link], &mut MediaCache::new(None), &mut cache, &|_, _, _, _, _, _| true,
            );
        }
        if self.ui.button(cx, ids!(layouts)).clicked(actions) {
            self.ui.link_preview(cx, ids!(card)).clear(cx);
            self.ui.view(cx, ids!(layout_cases)).set_visible(cx, true);
        }
        if self.ui.button(cx, ids!(inspect)).clicked(actions) {
            let mut rects = serde_json::Map::new();
            for (name, path) in [("desktop", ids!(desktop)), ("compact", ids!(compact)), ("own", ids!(own)), ("mobile", ids!(mobile))] {
                let row = self.ui.widget(cx, path);
                let rect = row.area().rect(cx);
                let body = row.widget(cx, ids!(content.message)).area().rect(cx);
                rects.insert(name.into(), serde_json::json!({"row": [rect.pos.x, rect.size.x], "text": [body.pos.x, body.size.x]}));
            }
            self.ui.label(cx, ids!(result)).set_text(cx, &serde_json::Value::Object(rects).to_string());
        }
        for action in actions {
            if let HtmlLinkAction::Clicked {url, ..} = action.as_widget_action().cast() {
                self.ui.label(cx, ids!(result)).set_text(cx, &url);
            }
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        script_eval!(vm, {mod.theme = mod.themes.light});
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        article_makepad::apple_fonts::install(vm);
        makepad_widgets::widgets_mod(vm);
        rinx::app::register_widgets(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
