//! UI translations only. Message bodies, names, IDs and wire content are never translated.
use std::{
    collections::BTreeMap,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN", alias = "zh", alias = "zh-Hans")]
    Chinese,
}
impl Language {
    pub fn index(self) -> usize {
        if self == Self::Chinese { 1 } else { 0 }
    }
    pub fn from_index(index: usize) -> Self {
        if index == 1 {
            Self::Chinese
        } else {
            Self::English
        }
    }
    pub fn name(self) -> &'static str {
        if self == Self::Chinese {
            "简体中文"
        } else {
            "English"
        }
    }
}
static LANGUAGE: OnceLock<AtomicU8> = OnceLock::new();
fn language_cell() -> &'static AtomicU8 {
    LANGUAGE.get_or_init(|| {
        // Unit tests see the English UI whatever language the person picked:
        // they never read the real data folder's `ui-language.json`.
        if cfg!(test) {
            return AtomicU8::new(Language::English.index() as u8);
        }
        let language: Language = std::fs::read(crate::app_data_dir().join("ui-language.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        AtomicU8::new(language.index() as u8)
    })
}
pub fn language() -> Language {
    Language::from_index(language_cell().load(Ordering::Relaxed) as usize)
}
fn catalog() -> &'static BTreeMap<String, String> {
    static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../resources/i18n/zh-CN.json"))
            .expect("validated Chinese UI catalog")
    })
}
pub fn text_for(language: Language, english: &str) -> &str {
    if language == Language::Chinese {
        catalog()
            .get(english)
            .map(String::as_str)
            .unwrap_or(english)
    } else {
        english
    }
}
pub fn tr(english: &str) -> &str {
    text_for(language(), english)
}
pub fn plural_suffix(count: usize) -> &'static str {
    if language() == Language::Chinese || count == 1 {
        ""
    } else {
        "s"
    }
}
pub fn time_format() -> &'static str {
    if language() == Language::Chinese {
        "%H:%M"
    } else {
        "%-I:%M %P"
    }
}
pub fn date_format() -> &'static str {
    if language() == Language::Chinese {
        "%Y年%-m月%-d日"
    } else {
        "%a %b %-d, %Y"
    }
}
pub fn full_date_format() -> &'static str {
    if language() == Language::Chinese {
        "%Y年%-m月%-d日 %H:%M:%S"
    } else {
        "%a %b %-d, %Y, %r"
    }
}

/// Declare source-only binding metadata on text widget prototypes. Makepad
/// validates properties against Rust types; named source slots provide an
/// extension point without changing those external types or their layouts.
pub fn install(vm: &mut makepad_widgets::ScriptVm) {
    use makepad_widgets::*;
    let prototypes = [
        Label::script_proto(vm),
        Button::script_proto(vm),
        CheckBox::script_proto(vm),
        RadioButton::script_proto(vm),
        TextInput::script_proto(vm),
        Html::script_proto(vm),
        DropDown::script_proto(vm),
        LinkLabel::script_proto(vm),
        script_eval!(vm, {mod.widgets.Label}),
        script_eval!(vm, {mod.widgets.Button}),
        script_eval!(vm, {mod.widgets.ButtonFlat}),
        script_eval!(vm, {mod.widgets.CheckBox}),
        script_eval!(vm, {mod.widgets.CheckBoxFlat}),
        script_eval!(vm, {mod.widgets.ToggleFlat}),
        script_eval!(vm, {mod.widgets.RadioButton}),
        script_eval!(vm, {mod.widgets.TextInput}),
        script_eval!(vm, {mod.widgets.Html}),
        script_eval!(vm, {mod.widgets.DropDown}),
        script_eval!(vm, {mod.widgets.DropDownFlat}),
        script_eval!(vm, {mod.widgets.LinkLabel}),
    ];
    for prototype in prototypes {
        if let Some(object) = prototype.as_object() {
            for key in [
                id!(i18n_text),
                id!(i18n_empty_text),
                id!(i18n_body),
                id!(i18n_labels),
            ] {
                vm.bx.heap.set_value_vec(object, key.into(), NIL, NoTrap);
            }
        }
    }
}

/// Rebake intentionally preserves Makepad string fields. Refresh only fields
/// explicitly tagged by our UI declarations; never translate arbitrary text
/// from a widget tree or reapply whole widgets (which would reset inputs).
pub fn refresh_ui(cx: &mut makepad_widgets::Cx, root: &makepad_widgets::WidgetRef) {
    refresh_ui_in_language(cx, root, language());
}
fn refresh_ui_in_language(
    cx: &mut makepad_widgets::Cx,
    root: &makepad_widgets::WidgetRef,
    language: Language,
) {
    use makepad_widgets::*;
    let mut stack = vec![root.clone()];
    let mut seen = std::collections::HashSet::new();
    while let Some(widget) = stack.pop() {
        if !seen.insert(widget.widget_uid()) {
            continue;
        }
        widget.children(&mut |_, child| stack.push(child));
        let source = widget.script_source();
        if source == ScriptObject::ZERO {
            continue;
        }
        let keys = cx.with_vm(|vm| {
            [id!(i18n_text), id!(i18n_empty_text), id!(i18n_body)].map(|id| {
                let value = vm.bx.heap.value(source, id.into(), NoTrap);
                if value.is_string_like() {
                    String::script_from_value(vm, value)
                } else {
                    String::new()
                }
            })
        });
        for key in [&keys[0], &keys[2]] {
            if key.is_empty() {
                continue;
            }
            let current = widget.text();
            // An imperative controller owns a field once it replaces its
            // declared label. Leave those values to that controller.
            if current == *key || current == text_for(Language::Chinese, key) {
                widget.set_text(cx, text_for(language, key));
            }
        }
        if !keys[1].is_empty() {
            let text = text_for(language, &keys[1]).to_owned();
            let mut target = widget.clone();
            script_apply_eval!(cx, target, {empty_text: #(text)});
        }
        let labels = cx.with_vm(|vm| {
            let value = vm.bx.heap.value(source, id!(i18n_labels).into(), NoTrap);
            if value.as_object().is_some() {
                Vec::<String>::script_from_value(vm, value)
            } else {
                Vec::new()
            }
        });
        if !labels.is_empty() {
            widget.as_drop_down().set_labels(
                cx,
                labels
                    .iter()
                    .map(|key| text_for(language, key).to_owned())
                    .collect(),
            );
        }
    }
    cx.redraw_all();
}

/// Substitute named arguments once, after translating the template. Arguments
/// are opaque: a room name containing braces or English UI words stays intact.
pub fn format(english: &str, args: &[(&str, String)]) -> String {
    interpolate(tr(english), args)
}
fn interpolate(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start + 1..].find('}').map(|n| n + start + 1) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let key = &rest[start + 1..end];
        if let Some((_, value)) = args.iter().find(|(name, _)| *name == key) {
            out.push_str(value);
        } else {
            out.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}
pub fn set_language(cx: &mut makepad_widgets::Cx, language: Language) -> anyhow::Result<()> {
    if language == self::language() {
        return Ok(());
    }
    use std::io::Write;
    std::fs::create_dir_all(crate::app_data_dir())?;
    let path = crate::app_data_dir().join("ui-language.json");
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec(&language)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    language_cell().store(language.index() as u8, Ordering::Relaxed);
    // Makepad's Rebake reruns DSL expressions while preserving imperative
    // widget state, including focused text inputs and unsent drafts.
    cx.request_live_edit();
    cx.redraw_all();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_bindings_translate_labels_without_rewriting_data_or_inputs() {
        use makepad_widgets::*;
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let ui = cx.with_vm(|vm| {
            makepad_widgets::script_mod(vm);
            install(vm);
            crate::shared::cached_widget::script_mod(vm);
            let value = script_eval!(vm, {
                use mod.prelude.widgets.*
                View {
                    heading := Label {text: "Chats" i18n_text: "Chats"}
                    data := Label {text: "Chats"}
                    input := TextInput {text: "Chats {count} 中文" empty_text: "Password" i18n_empty_text: "Password"}
                    cached := CachedWidget {
                        cached_input := TextInput {text: "Saved draft 中文"}
                    }
                }
            });
            let mut ui = WidgetRef::script_from_value(vm, value);
            let before = ui.text_input(vm.cx_mut(), ids!(cached_input));
            before.set_text(vm.cx_mut(), "Edited draft 中文");
            for apply in [Apply::Rebake, Apply::ScriptReapply, Apply::Rebake] {
                ui.script_apply(vm, &apply, &mut Scope::empty(), value);
                let after = ui.text_input(vm.cx_mut(), ids!(cached_input));
                assert_eq!(before.widget_uid(), after.widget_uid());
                assert_eq!(after.text(), "Edited draft 中文");
            }
            ui
        });
        assert_eq!(ui.label(&mut cx, ids!(heading)).text(), "Chats");
        refresh_ui_in_language(&mut cx, &ui, Language::Chinese);
        assert_eq!(ui.label(&mut cx, ids!(heading)).text(), "聊天");
        assert_eq!(ui.label(&mut cx, ids!(data)).text(), "Chats");
        assert_eq!(
            ui.text_input(&mut cx, ids!(input)).text(),
            "Chats {count} 中文"
        );
        refresh_ui_in_language(&mut cx, &ui, Language::English);
        assert_eq!(ui.label(&mut cx, ids!(heading)).text(), "Chats");
        assert_eq!(
            ui.text_input(&mut cx, ids!(input)).text(),
            "Chats {count} 中文"
        );
    }
    #[test]
    fn catalog_is_complete_and_preserves_placeholders() {
        let en: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../resources/i18n/en.json")).unwrap();
        assert_eq!(en.len(), catalog().len());
        for (key, value) in en {
            assert_eq!(key, value);
            let zh = catalog().get(&key).expect("missing translation");
            assert!(!zh.trim().is_empty());
            fn placeholders(text: &str) -> std::collections::BTreeSet<&str> {
                text.split('{')
                    .skip(1)
                    .filter_map(|s| s.split_once('}').map(|p| p.0))
                    .collect()
            }
            assert_eq!(placeholders(&key), placeholders(zh), "{key}");
        }
    }
    #[test]
    fn formatting_preserves_user_content_and_unknown_keys_fall_back() {
        assert_eq!(
            interpolate(
                "{name}: {count}",
                &[("name", "Chats {count} 中文".into()), ("count", "2".into())]
            ),
            "Chats {count} 中文: 2"
        );
        assert_eq!(text_for(Language::English, "Chats"), "Chats");
        assert_eq!(text_for(Language::Chinese, "Chats"), "聊天");
        assert_eq!(
            text_for(Language::Chinese, "Unknown server error"),
            "Unknown server error"
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"zh-Hans\"").unwrap(),
            Language::Chinese
        );
        assert!(serde_json::from_str::<Language>("\"unsupported\"").is_err());
    }
}
