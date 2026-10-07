//! Offline review of production Moments, article library and tabbed reader.
//! No account, backend operations, or permission grants.
use makepad_widgets::*;
use rinx::{
    article_app::{ArticlePanelWidgetRefExt, document::Document},
    moments::{
        backend::{Feed, Timeline},
        model::{Index, post_content},
        ui::MomentsPanelWidgetRefExt,
    },
    shared::web_browser::{WebBrowserAction, WebBrowserWidgetRefExt},
};
use serde_json::json;
app_main!(App);
fn mode() -> String {
    std::env::var("RINX_REVIEW_SCREEN").unwrap_or_else(|_| "moments".into())
}
fn narrow() -> bool {
    std::env::args().any(|arg| arg == "--narrow")
}
script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    startup() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.inner_size: #(if narrow() {dvec2(390., 844.)} else if mode() == "moments" {dvec2(560., 820.)} else {dvec2(1040., 820.)})
                window.title: #(match mode().as_str() {
                    "library" => "Rinx · 我的文章 · 离线设计预览",
                    "reader" => "Rinx · 标签阅读器 · 离线设计预览",
                    _ => "Rinx · 朋友圈 · 离线设计预览",
                })
                body +: {
                    moments := MomentsPanel {visible: #(mode() == "moments") padding: 0}
                    articles := ArticlePanel {visible: #(mode() == "library") padding: 0}
                    browser := WebBrowser {visible: #(mode() == "reader") padding: 0}
                }
            }
        }
    }
}
#[derive(Script, ScriptHook)]
struct App {
    #[live]
    ui: WidgetRef,
}
const NOTES: &str = "午后，沿着河边走了一会儿。阳光落在水面上，熟悉的街道也有了新的颜色。\n\n## 留一点时间给生活\n\n生活里的好时刻，往往没有计划。关掉通知，带一本书出门，在喜欢的小店坐下来，就能拥有一个安静的下午。\n\n周末不一定要去很远的地方。重新走一遍回家的路，看看树影，和朋友聊聊最近读到的故事。\n\n> 慢下来，才能看见平时错过的风景。\n\n## 这个周末的清单\n\n- 去河边散步，看一场日落\n- 读完床头那本书，记下喜欢的句子\n- 和朋友一起做一顿晚饭\n\n## 分享，是另一种记录\n\n写下今天的小事，不是为了证明什么，只是想在日后翻看的时候，记得这一天的心情。\n\n一段文字、一张照片，都能让普通的日子有迹可循。\n\n## 下次再见\n\n愿我们都能在忙碌之中，找到属于自己的节奏。";
impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let mut feed = Feed::default();
        let media_count: usize = std::env::var("RINX_REVIEW_MEDIA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
            .min(9);
        let mut cache = rinx::media_cache::MediaCache::new(None);
        let assets: Vec<rinx::moments::model::Asset> = (0..media_count).map(|n| {
            let uri: ruma::OwnedMxcUri = format!("mxc://example.org/review{n}").into();
            cache.review_image(uri.clone(), std::sync::Arc::from(include_bytes!("../tools/wechat-ux/fixtures/moments-photo.png").as_slice()));
            serde_json::from_value(json!({"name":format!("Photo {}",n+1),"mimetype":"image/png","size":1024,"file":{"url":uri,"v":"v2","key":{"kty":"oct","key_ops":["decrypt","encrypt"],"alg":"A256CTR","k":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","ext":true},"iv":"AAAAAAAAAAAAAAAAAAAAAA","hashes":{"sha256":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}}})).unwrap()
        }).collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        for (n, name, body) in [
            (
                0,
                "林小溪",
                "周末沿着河边走了一会儿。阳光落在水面上，熟悉的街道也有了新的颜色。\n\n慢下来，才发现生活里还有这么多小美好。",
            ),
            (
                1,
                "陈一鸣",
                "今天的咖啡和新读的书。\n\n读到一句很喜欢的话：好的设计，让人忘记设计的存在。",
            ),
            (
                2,
                "苏然",
                "终于完成了这周的小目标！和朋友一起做了一顿晚饭，聊了很多最近的见闻。\n\n明天继续努力，也记得好好休息。",
            ),
            (3, "周末散步", "天气刚好，出门看看。"),
        ] {
            let room: ruma::OwnedRoomId = format!("!review{n}:example.org").try_into().unwrap();
            let author: ruma::OwnedUserId = format!("@review{n}:example.org").try_into().unwrap();
            let event = format!("$review{n}");
            let mut index = Index::default();
            index.insert(&room, json!({"type":"m.room.message","event_id":event,"sender":author,"origin_server_ts":now - (n+1)*3600000,"content":post_content(body,if n == 0 {&assets}else{&[]})}));
            if n < 2 {
                index.insert(&room, json!({"type":"m.reaction","event_id":format!("$like{n}"),"sender":"@emma:example.org","origin_server_ts":now,"content":{"m.relates_to":{"rel_type":"m.annotation","event_id":event,"key":"❤️"}}}));
                index.insert(&room, json!({"type":"m.room.message","event_id":format!("$comment{n}"),"sender":"@emma:example.org","origin_server_ts":now,"content":rinx::moments::model::comment_content("这样的周末真好，下次一起！", &ruma::OwnedEventId::try_from(event).unwrap())}));
            }
            feed.timelines.insert(
                room.clone(),
                Timeline {
                    room,
                    author,
                    name: name.into(),
                    invited: false,
                    members: vec![],
                    audience: String::new(),
                    index,
                    cursor: None,
                    refresh_cursor: None,
                    exhausted: true,
                    loaded: true,
                },
            );
        }
        self.ui
            .moments_panel(cx, ids!(moments))
            .review_feed(cx, feed);
        self.ui
            .moments_panel(cx, ids!(moments))
            .review_media(cx, cache);
        let documents = [
            "把普通的日子，过成喜欢的样子",
            "周末读书笔记：留一点时间给自己",
            "沿着河边走，遇见城市另一面",
            "一杯咖啡的时间",
            "设计里的小细节",
        ]
        .iter()
        .map(|title| {
            let mut d = Document::from_markdown(title, NOTES).unwrap();
            d.author = "林小溪".into();
            d.summary = "散步、阅读和记录，发现生活里的小美好。".into();
            d
        })
        .collect();
        self.ui
            .article_panel(cx, ids!(articles))
            .review_library(cx, documents);
        if mode() == "reader" {
            for title in ["周末读书笔记", "把普通的日子，过成喜欢的样子"] {
                self.ui.web_browser(cx, ids!(browser)).action(
                    cx,
                    ModalRef::default(),
                    &WebBrowserAction::OpenMarkdown {
                        title: title.into(),
                        source: NOTES.into(),
                        account: None,
                        attachment: None,
                    },
                );
            }
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
        #[cfg(feature = "palpo-instrument")]
        if rinx::performance::begin(cx, event) { return; }
        if let Event::Custom(command) = event {
            if command == "review:fonts" {
                let mut report = Vec::new();
                cx.with_vm(|vm| {
                    for value in [script_eval!(vm, {mod.theme.font_regular}), script_eval!(vm, {mod.widgets.Label.draw_text.text_style})] {
                        let style = TextStyle::script_from_value(vm, value);
                        vm.with_cx_mut(|cx| style.ensure_fonts_loaded(cx));
                        let fonts = vm.with_cx_mut(|cx| cx.get_global::<std::rc::Rc<std::cell::RefCell<makepad_draw::text::fonts::Fonts>>>().clone());
                        let family = fonts.borrow_mut().get_or_load_font_family(style.font_family_id());
                        report.push(json!({"members":style.font_family.member_ids().collect::<Vec<_>>(),"font_ids":format!("{:?}",family.fonts().iter().map(|f| f.id()).collect::<Vec<_>>())}));
                    }
                });
                std::fs::write(
                    rinx::app_data_dir().join("font-review.json"),
                    serde_json::to_vec_pretty(&report).unwrap(),
                )
                .unwrap();
            }
            if command == "review:dark" || command == "review:light" {
                rinx::theme::select(
                    cx,
                    rinx::theme::Selection {
                        appearance: if command == "review:dark" {
                            rinx::theme::Appearance::Dark
                        } else {
                            rinx::theme::Appearance::Light
                        },
                        ..Default::default()
                    },
                )
                .unwrap();
            }
        }
        self.match_event(cx, event);
        #[cfg(feature = "palpo-instrument")]
        rinx::performance::phase(cx, "rinx_match");
        self.ui.handle_event(cx, event, &mut Scope::empty());
        #[cfg(feature = "palpo-instrument")]
        rinx::performance::phase(cx, "rinx_widgets");
        rinx::theme::packages::after_event(cx, event);
        #[cfg(feature = "palpo-instrument")]
        rinx::performance::end(cx);
    }
}
