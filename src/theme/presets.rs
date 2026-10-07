//! Coordinated light/dark palettes as portable theme data, rendered by Makepad.
//! Sources and role mappings: resources/themes/{octoscode,community}/provenance.json.
use super::packages::Preferences;
use octosense_theme_contract::ThemePackage;
use std::sync::OnceLock;

pub fn all() -> &'static [ThemePackage] {
    static PRESETS: OnceLock<Vec<ThemePackage>> = OnceLock::new();
    PRESETS.get_or_init(|| {
        [
            include_bytes!("../../resources/themes/octoscode/codex.octotheme").as_slice(),
            include_bytes!("../../resources/themes/octoscode/claude.octotheme").as_slice(),
            include_bytes!("../../resources/themes/octoscode/slate.octotheme").as_slice(),
            include_bytes!("../../resources/themes/octoscode/solarized.octotheme").as_slice(),
            include_bytes!("../../resources/themes/rinx/sage.octotheme").as_slice(),
            include_bytes!("../../resources/themes/rinx/rose.octotheme").as_slice(),
            include_bytes!("../../resources/themes/community/catppuccin.octotheme").as_slice(),
            include_bytes!("../../resources/themes/community/nord.octotheme").as_slice(),
            include_bytes!("../../resources/themes/community/dracula.octotheme").as_slice(),
            include_bytes!("../../resources/themes/community/gruvbox.octotheme").as_slice(),
            include_bytes!("../../resources/themes/community/tokyo-night.octotheme").as_slice(),
        ]
        .into_iter()
        .map(|bytes| ThemePackage::parse(bytes).expect("bundled palette must be valid"))
        .collect()
    })
}

/// Edited/imported packages must not masquerade as an unmodified preset by ID.
pub fn index(package: &ThemePackage) -> Option<usize> {
    all().iter().position(|preset| preset == package)
}

/// Upgrade only the exact, unedited dark-only packages shipped before light variants.
/// Imported packages with the same ID but any different data remain user-owned.
pub fn upgrade(preferences: &mut Preferences) {
    let Some(package) = preferences.package.as_ref() else {
        return;
    };
    for preset in all().iter().take(4) {
        let mut legacy = preset.clone();
        legacy.tokens = legacy.variants.dark.clone();
        legacy.variants.light = serde_json::json!({});
        legacy.variants.dark = serde_json::json!({});
        if package == &legacy {
            preferences.package = Some(preset.clone());
            return;
        }
    }
}

/// Palette and mode are independent. Index zero restores the native palette.
pub fn select(index: usize, mut preferences: Preferences) -> Option<Preferences> {
    preferences.package = if index == 0 {
        None
    } else {
        Some(all().get(index - 1)?.clone())
    };
    Some(preferences)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{packages, Appearance, Selection};
    use makepad_widgets::{desktop_style::DesktopStyle, Cx};
    use octosense_theme_contract as contract;

    #[test]
    fn source_palettes_validate_for_every_platform_and_round_trip() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        for (i, package) in all().iter().enumerate() {
            assert_eq!(
                ThemePackage::parse(&package.bytes().unwrap()).unwrap(),
                *package
            );
            let preferences = select(i + 1, Preferences::default()).unwrap();
            assert_eq!(preferences.selection.appearance, Appearance::Light);
            assert!(!preferences.follow_system);
            for family in [
                DesktopStyle::Macos,
                DesktopStyle::Windows,
                DesktopStyle::Ios,
                DesktopStyle::Android,
            ] {
                packages::stylesheet(&mut cx, &preferences, family).unwrap();
                for appearance in [Appearance::Light, Appearance::Dark] {
                    let tokens = packages::resolve(
                        &mut cx,
                        package,
                        Selection {
                            appearance,
                            ..Default::default()
                        },
                        family,
                    )
                    .unwrap();
                    contract::validate_resolved(&tokens).unwrap();
                    assert_ne!(tokens["color.surface.page"], tokens["color.chat.incoming"]);
                    assert_ne!(tokens["color.surface.page"], tokens["color.chat.outgoing"]);
                    assert_ne!(tokens["color.chat.incoming"], tokens["color.chat.outgoing"]);
                    // Preserve OctosCode's original dark fills. All new variants
                    // enforce separation between the canvas and both bubbles.
                    if appearance == Appearance::Light || !package.id.starts_with("octoscode-") {
                        for (a, b) in [
                            ("color.surface.page", "color.chat.incoming"),
                            ("color.surface.page", "color.chat.outgoing"),
                            ("color.chat.incoming", "color.chat.outgoing"),
                        ] {
                            assert!(
                                contract::contrast(
                                    contract::rgb(&tokens[a]).unwrap(),
                                    contract::rgb(&tokens[b]).unwrap()
                                ) >= 1.15,
                                "{} {appearance:?}: {a} and {b} must be visually distinct",
                                package.name
                            );
                        }
                    }
                }
                let light =
                    packages::resolve(&mut cx, package, Selection::default(), family).unwrap();
                let dark = packages::resolve(
                    &mut cx,
                    package,
                    Selection {
                        appearance: Appearance::Dark,
                        ..Default::default()
                    },
                    family,
                )
                .unwrap();
                assert_ne!(light["color.surface.page"], dark["color.surface.page"]);
            }
        }
    }

    #[test]
    fn palette_changes_preserve_mode_and_system_preference() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            for follow_system in [false, true] {
                let preferences = Preferences {
                    selection: Selection {
                        appearance,
                        ..Default::default()
                    },
                    follow_system,
                    ..Default::default()
                };
                for i in 0..=all().len() {
                    let selected = select(i, preferences.clone()).unwrap();
                    assert_eq!(selected.selection, preferences.selection);
                    assert_eq!(selected.follow_system, follow_system);
                }
            }
        }
    }

    #[test]
    fn edited_presets_are_custom_and_default_restores_native_palette() {
        let mut preferences = select(1, Preferences::default()).unwrap();
        assert_eq!(index(preferences.package.as_ref().unwrap()), Some(0));
        preferences.package.as_mut().unwrap().name = "My Codex".into();
        assert_eq!(index(preferences.package.as_ref().unwrap()), None);
        assert!(select(0, preferences).unwrap().package.is_none());
        assert!(select(99, Preferences::default()).is_none());
    }
}
