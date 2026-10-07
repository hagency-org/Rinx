//! Standalone selection UI. Hosted instances display the host's ownership.
use makepad_widgets::*;
use crate::theme::{self, Accent, Appearance, Selection};
use crate::theme::packages::Preferences;

fn mode_labels(translated: bool) -> Vec<String> {
    let labels: &[&str] = if theme::system::supported() {
        &["Light", "Dark", "System"]
    } else {
        &["Light", "Dark"]
    };
    labels.iter().map(|label| {
        if translated { crate::i18n::tr(label) } else { label }.to_string()
    }).collect()
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    let AppearanceChoice = mod.widgets.RinxDropDown {
        width: Fill height: RINX_CONTROL_HEIGHT
        align: Align{x: 0.0 y: 0.5}
        margin: 0 padding: Inset{left: 12 right: 32}
        draw_text +: {color: RINX_INK color_hover: RINX_INK color_focus: RINX_INK color_down: RINX_INK text_style: theme.font_regular{font_size: RINX_BODY_SIZE}}
        draw_bg +: {
            color: RINX_SURFACE color_hover: RINX_HOVER color_down: RINX_PRESSED
            border_color: RINX_BORDER border_color_focus: RINX_ACCENT
            arrow_color: RINX_INK border_radius: RINX_RADIUS_SM
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, self.border_radius)
                sdf.fill_keep(self.color.mix(self.color_hover, self.hover).mix(self.color_down, self.down))
                sdf.stroke(self.border_color.mix(self.border_color_focus, self.focus), 1.0)
                let c = vec2(self.rect_size.x - 17.0, self.rect_size.y * 0.5)
                sdf.move_to(c.x - 4.0, c.y - 2.0)
                sdf.line_to(c.x, c.y + 2.0)
                sdf.line_to(c.x + 4.0, c.y - 2.0)
                sdf.stroke(self.arrow_color, 1.5)
                return sdf.result
            }
        }
    }
    mod.widgets.AppearanceSettings = #(AppearanceSettings::register_widget(vm)) {
        width: Fill height: Fit flow: Down spacing: 8 padding: Inset{top: 12 bottom: 16}
        RinxLabel {text: #(crate::i18n::tr("App appearance")) i18n_text: "App appearance" draw_text.text_style: theme.font_bold{font_size: (13 * mod.widgets.RINX_TEXT_SCALE)}}
        choices := View {width: Fill height: Fit flow: Flow.Right{wrap: true} spacing: 12
            mode_choice := View {width: 150 height: Fit flow: Down spacing: 6
                RinxHint {text: #(crate::i18n::tr("Appearance mode")) i18n_text: "Appearance mode"}
                mode := AppearanceChoice {labels: #(mode_labels(true)) i18n_labels: #(mode_labels(false))}
            }
            palette_choice := View {width: 200 height: Fit flow: Down spacing: 6
                RinxHint {text: #(crate::i18n::tr("Color palette")) i18n_text: "Color palette"}
                theme_preset := AppearanceChoice {labels: [#(crate::i18n::tr("Rinx (default)"))]}
            }
            accent_choice := View {width: 150 height: Fit flow: Down spacing: 6
                RinxHint {text: #(crate::i18n::tr("Accent color")) i18n_text: "Accent color"}
                accent := AppearanceChoice {labels: [#(crate::i18n::tr("Teal")), #(crate::i18n::tr("Violet"))] i18n_labels: ["Teal", "Violet"]}
            }
        }
        chat_preview := View {width: Fill height: Fit flow: Down spacing: 6
            RinxHint {text: #(crate::i18n::tr("Chat preview")) i18n_text: "Chat preview"}
            RoundedView {width: Fill{max: 480} height: Fit flow: Down padding: 12 spacing: 8
                draw_bg +: {color: RINX_PAGE border_size: 0 border_radius: RINX_RADIUS_MD}
                RoundedView {width: Fit height: Fit padding: Inset{left: 12 right: 12 top: 8 bottom: 8}
                    draw_bg +: {color: RINX_INCOMING border_size: 0 border_radius: RINX_RADIUS_SM}
                    RinxLabel {text: #(crate::i18n::tr("Incoming message")) i18n_text: "Incoming message"}
                }
                View {width: Fill height: Fit align: Align{x: 1.0}
                    RoundedView {width: Fit height: Fit padding: Inset{left: 12 right: 12 top: 8 bottom: 8}
                        draw_bg +: {color: RINX_OUTGOING border_size: 0 border_radius: RINX_RADIUS_SM}
                        RinxLabel {text: #(crate::i18n::tr("Your message")) i18n_text: "Your message"}
                    }
                }
            }
        }
        customize := RinxButton {text: #(crate::i18n::tr("Customize appearance")) i18n_text: "Customize appearance"}
        host_note := RinxHint {visible: false width: Fill text: #(crate::i18n::tr("Appearance is managed by OctoSense")) i18n_text: "Appearance is managed by OctoSense"}
        error := RinxHint {visible: false width: Fill}
    }
}

#[derive(Script, Widget)]
pub struct AppearanceSettings {
    #[deref]
    view: View,
    #[rust]
    selection: Option<Selection>,
}
impl ScriptHook for AppearanceSettings {
    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        _apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        let preferences = theme::packages::current_for_vm(vm).ok();
        self.show_preferences(vm.cx_mut(), preferences.as_ref());
    }
}

impl AppearanceSettings {
    fn show_preferences(&mut self, cx: &mut Cx, preferences: Option<&Preferences>) {
        self.selection = preferences.map(|p| p.selection);
        self.view
            .view(cx, ids!(choices))
            .set_visible(cx, self.selection.is_some());
        self.view
            .label(cx, ids!(host_note))
            .set_visible(cx, self.selection.is_none());
        self.view
            .button(cx, ids!(customize))
            .set_visible(cx, self.selection.is_some());
        self.view.view(cx, ids!(chat_preview)).set_visible(cx, self.selection.is_some());
        if let Some(p) = preferences {
            let preset = p.package.as_ref().and_then(theme::presets::index);
            let mut labels = vec![crate::i18n::tr("Rinx (default)").to_string()];
            labels.extend(theme::presets::all().iter().map(|p| crate::i18n::tr(&p.name).to_string()));
            let selected = match (p.package.as_ref(), preset) {
                (_, Some(index)) => index + 1,
                (Some(package), None) => {
                    labels.push(crate::i18n::format("Custom: {0}", &[("0", package.name.clone())]));
                    labels.len() - 1
                }
                (None, None) => 0,
            };
            let chooser = self.view.drop_down(cx, ids!(theme_preset));
            chooser.set_labels(cx, labels);
            chooser.set_selected_item(cx, selected);
            // Mode remains available for every palette. Named palettes coordinate
            // their own accents; custom colors are editable in the theme studio.
            self.view.view(cx, ids!(mode_choice)).set_visible(cx, true);
            self.view.view(cx, ids!(accent_choice)).set_visible(cx, p.package.is_none());
            let index = if p.follow_system {
                2
            } else {
                usize::from(p.selection.appearance == Appearance::Dark)
            };
            self.view
                .drop_down(cx, ids!(mode))
                .set_selected_item(cx, index);
            self.view
                .drop_down(cx, ids!(accent))
                .set_selected_item(cx, usize::from(p.selection.accent == Accent::Violet));
        }
    }
}
impl Widget for AppearanceSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(customize)).clicked(actions) {
                cx.action(crate::settings::theme_studio::ThemeStudioAction::Open);
            }
            let Some(mut selection) = self.selection else {
                return;
            };
            let preset = self.view.drop_down(cx, ids!(theme_preset)).changed(actions);
            let mode = self.view.drop_down(cx, ids!(mode)).changed(actions);
            let accent = self.view.drop_down(cx, ids!(accent)).changed(actions);
            if let Some(index) = mode {
                selection.appearance = if index == 1 {
                    Appearance::Dark
                } else {
                    Appearance::Light
                };
            }
            if let Some(index) = accent {
                selection.accent = if index == 1 {
                    Accent::Violet
                } else {
                    Accent::Teal
                };
            }
            if preset.is_some() || mode.is_some() || accent.is_some() {
                let result = theme::packages::current(cx).and_then(|mut p| {
                    if let Some(index) = preset {
                        p = theme::presets::select(index, p).ok_or("Unknown theme preset")?;
                    } else {
                        p.selection = selection;
                        if let Some(index) = mode {
                            p.follow_system = index == 2;
                        }
                    }
                    theme::packages::apply(cx, p)
                });
                let current = theme::packages::current(cx).ok();
                self.show_preferences(cx, current.as_ref());
                match result {
                    Ok(()) => {
                        self.view.label(cx, ids!(error)).set_visible(cx, false);
                    }
                    Err(error) => {
                        self.view.label(cx, ids!(error)).set_text(cx, &error);
                        self.view.label(cx, ids!(error)).set_visible(cx, true);
                    }
                }
            }
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
