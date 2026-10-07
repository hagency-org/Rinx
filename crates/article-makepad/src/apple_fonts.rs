//! Use the OS-provided PingFang faces on macOS and iOS, without shipping Apple fonts.
//!
//! Makepad's script FontMember currently assumes face zero. PingFang is a TTC, so
//! resolve its installed URL with CoreText and register the matching collection
//! face through Makepad's native font API before widgets inherit the theme.

use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ffi::{CString, OsString, c_char, c_void},
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    ptr,
    rc::Rc,
};
use makepad_widgets::{
    *,
    makepad_draw::text::{font::FontId, font_face::FontFace, fonts::Fonts, loader::FontDefinition},
    makepad_platform::script::res::{CxScriptResource, CxScriptResourceData, CxScriptResourceGc},
};

const REGULAR: &str = "PingFangSC-Regular";
const SEMIBOLD: &str = "PingFangSC-Semibold";

pub fn install(vm: &mut ScriptVm) {
    // widgets_mod applies the selected stylesheet again. Resolve that theme
    // before choosing families, rather than installing into theme_mod's default
    // dark theme and then losing the override when a light stylesheet is applied.
    makepad_widgets::desktop_style::apply_theme(vm);
    // Resolve both weights before changing any families. A missing system asset
    // leaves the existing cross-platform theme usable, including CJK and emoji.
    let faces = [REGULAR, SEMIBOLD].map(load_system_face);
    let [Ok(regular), Ok(semibold)] = faces else {
        log!(
            "PingFang unavailable; keeping bundled fonts: {:?}",
            faces.map(|face| face.err())
        );
        return;
    };
    vm.with_cx_mut(|cx| {
        CxDraw::lazy_construct_fonts(cx);
    });
    let fonts = vm.with_cx_mut(|cx| cx.get_global::<Rc<RefCell<Fonts>>>().clone());
    let regular = register_face(vm, &fonts, REGULAR, regular);
    let semibold = register_face(vm, &fonts, SEMIBOLD, semibold);

    let styles = [
        (script_eval!(vm, { mod.theme.font_label }), regular),
        (script_eval!(vm, { mod.theme.font_regular }), regular),
        (script_eval!(vm, { mod.theme.font_bold }), semibold),
        (script_eval!(vm, { mod.theme.font_italic }), regular),
        (script_eval!(vm, { mod.theme.font_bold_italic }), semibold),
    ];
    for (value, resource) in styles {
        let Some(style) = value.as_object() else {
            continue;
        };
        let Some(fallback) = vm
            .bx
            .heap
            .value(style, id!(font_family).into(), NoTrap)
            .as_object()
        else {
            continue;
        };
        if vm
            .bx
            .heap
            .vec_ref(fallback)
            .first()
            .is_some_and(|member| member.key == id!(apple_system).into())
        {
            continue;
        }
        // Register the face as a real script family member. Overriding only the
        // native family cache is discarded by ensure_fonts_loaded(): its member
        // count no longer matches the script family, including after lazy emoji.
        let family = script_eval!(vm, {mod.text.FontFamily {
            apple_system := mod.text.FontMember {res: #(resource) asc: 0.0 desc: 0.0 weight: 0.0}
        }})
        .as_object()
        .unwrap();
        vm.bx.heap.vec_push_vec(family, fallback, NoTrap);
        vm.bx
            .heap
            .set_value(style, id!(font_family).into(), family.into(), NoTrap);
    }
    // Code and icon families retain their specialized fonts. PingFang has no
    // italic face; italic prose uses the corresponding upright PingFang weight.
    log!(
        "Apple typography: PingFang SC Regular/Semibold for English and Chinese; bundled fallback retained"
    );
}

fn register_face(
    vm: &mut ScriptVm,
    fonts: &Rc<RefCell<Fonts>>,
    name: &str,
    definition: FontDefinition,
) -> ScriptValue {
    let path = format!("rinx-system-font://{name}");
    let heap = vm.bx.heap.heap_key();
    // Match the pinned Makepad FontMember identity: resource path and metrics.
    // The pre-registered definition supplies the correct TTC collection index.
    let font_id = system_member_id(name);
    if let Some(handle) = vm
        .cx()
        .script_data
        .resources
        .get_handle_by_abs_path(heap, &path)
    {
        return handle.into();
    }
    let gc = {
        let resources = &vm.cx().script_data.resources;
        CxScriptResourceGc {
            resources: resources.resources.clone(),
            handles_by_abs_path: resources.handles_by_abs_path.clone(),
            handle: ScriptHandle::ZERO,
            heap_key: heap,
        }
    };
    let handle_type = vm.handle_type(id_lut!(res));
    let handle = vm.bx.heap.new_handle(handle_type, Box::new(gc));
    if vm
        .cx_mut()
        .script_data
        .resources
        .attach_handle_for_path(heap, &path, handle)
    {
        return handle.into();
    }
    if !fonts.borrow().is_font_known(font_id) {
        fonts.borrow_mut().define_font(font_id, definition);
    }
    // This resource is the stable script identity of a native, already defined
    // face. FontDefinition owns its mapped bytes and TTC index; duplicating the
    // entire collection in each weight's script resource wastes tens of MB.
    vm.cx_mut().script_data.resources.insert_resource(
        heap,
        CxScriptResource {
            abs_path: path,
            dependency_path: None,
            web_url: None,
            data: CxScriptResourceData::Loaded(Rc::new(Vec::new())),
            handles: vec![(heap, handle)],
        },
    );
    handle.into()
}

fn system_member_id(name: &str) -> FontId {
    let mut hash = DefaultHasher::new();
    format!("rinx-system-font://{name}").hash(&mut hash);
    for _ in 0..3 {
        0.0_f32.to_bits().hash(&mut hash);
    }
    FontId::from(hash.finish())
}

fn load_system_face(name: &str) -> Result<FontDefinition, String> {
    let path = system_font_path(name)?;
    let data =
        SharedBytes::from_file_mmap_or_read(&path).map_err(|error| format!("{name}: {error}"))?;
    let count = if data.as_slice().starts_with(b"ttcf") {
        let count = data
            .as_slice()
            .get(8..12)
            .ok_or("truncated font collection")?;
        u32::from_be_bytes(count.try_into().unwrap())
    } else {
        1
    };
    for index in 0..count {
        let Some(face) = FontFace::from_data_and_index(data.clone(), index) else {
            continue;
        };
        let matches = face.with_ttf_parser_face(|face| {
            face.names().into_iter().any(|record| {
                // OpenType name ID 6 is the PostScript name, also returned by CoreText.
                record.name_id == 6 && record.to_string().as_deref() == Some(name)
            })
        });
        if matches {
            return Ok(FontDefinition {
                data,
                index,
                ascender_fudge_in_ems: 0.0,
                descender_fudge_in_ems: 0.0,
                weight: None,
                variations: Vec::new(),
            });
        }
    }
    Err(format!("{name}: matching collection face not found"))
}

// Small, private bindings to stable public APIs available on both Apple targets.
type CFRef = *const c_void;
#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFRef, size: f64, matrix: *const c_void) -> CFRef;
    fn CTFontCopyPostScriptName(font: CFRef) -> CFRef;
    fn CTFontCopyAttribute(font: CFRef, attribute: CFRef) -> CFRef;
    static kCTFontURLAttribute: CFRef;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(allocator: CFRef, text: *const c_char, encoding: u32) -> CFRef;
    fn CFEqual(a: CFRef, b: CFRef) -> u8;
    fn CFGetTypeID(value: CFRef) -> usize;
    fn CFURLGetTypeID() -> usize;
    fn CFURLGetFileSystemRepresentation(
        url: CFRef,
        resolve: u8,
        buffer: *mut u8,
        length: isize,
    ) -> u8;
    fn CFRelease(value: CFRef);
}

struct OwnedCf(CFRef);
impl OwnedCf {
    fn checked(value: CFRef) -> Result<Self, String> {
        if value.is_null() {
            Err("CoreText returned no font resource".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for OwnedCf {
    fn drop(&mut self) {
        // SAFETY: each instance owns one non-null Create/Copy-rule reference.
        unsafe { CFRelease(self.0) }
    }
}

fn system_font_path(name: &str) -> Result<PathBuf, String> {
    let name = CString::new(name).map_err(|error| error.to_string())?;
    // SAFETY: CoreFoundation objects remain alive throughout their use. All
    // pointers are checked, the attribute's type is validated, and CFURL receives
    // the actual writable buffer length. RAII releases every owned reference.
    unsafe {
        let requested = OwnedCf::checked(CFStringCreateWithCString(
            ptr::null(),
            name.as_ptr(),
            0x08000100,
        ))?;
        let font = OwnedCf::checked(CTFontCreateWithName(requested.0, 16.0, ptr::null()))?;
        let actual = OwnedCf::checked(CTFontCopyPostScriptName(font.0))?;
        if CFEqual(requested.0, actual.0) == 0 {
            return Err("CoreText substituted a different font".into());
        }
        let url = OwnedCf::checked(CTFontCopyAttribute(font.0, kCTFontURLAttribute))?;
        if CFGetTypeID(url.0) != CFURLGetTypeID() {
            return Err("font URL is not a CFURL".into());
        }
        let mut buffer = vec![0; libc::PATH_MAX as usize];
        if CFURLGetFileSystemRepresentation(url.0, 1, buffer.as_mut_ptr(), buffer.len() as isize)
            == 0
        {
            return Err("font URL has no readable filesystem path".into());
        }
        let length = buffer
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("unterminated font path")?;
        buffer.truncate(length);
        Ok(PathBuf::from(OsString::from_vec(buffer)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_pingfang_faces_cover_english_and_chinese() {
        let mut indices = Vec::new();
        for name in [REGULAR, SEMIBOLD] {
            let definition = load_system_face(name).expect("installed PingFang face");
            indices.push(definition.index);
            let face = FontFace::from_data_and_index(definition.data, definition.index).unwrap();
            face.with_ttf_parser_face(|face| {
                for character in "Robrix Chats 0123 中文聊天简体繁體，。！？".chars() {
                    assert!(
                        face.glyph_index(character).is_some(),
                        "{name} missing {character}"
                    );
                    if !character.is_whitespace() {
                        assert!(
                            face.glyph_bounding_box(face.glyph_index(character).unwrap())
                                .is_some(),
                            "{name} has no drawable outline for {character}"
                        );
                    }
                }
            });
        }
        assert_ne!(
            indices[0], indices[1],
            "regular and semibold must select distinct TTC faces"
        );
    }

    #[test]
    fn missing_font_does_not_silently_use_coretext_substitution() {
        assert!(system_font_path("Robrix-DeliberatelyMissingFont").is_err());
    }

    #[test]
    fn theme_prefers_pingfang_and_preserves_fallback_after_widget_registration() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(|vm| {
            makepad_widgets::theme_mod(vm);
            script_eval!(vm, { mod.theme = mod.themes.light });
            install(vm);
            makepad_widgets::widgets_mod(vm);
            for (value, expected) in [
                (script_eval!(vm, { mod.theme.font_regular }), REGULAR),
                (script_eval!(vm, { mod.theme.font_bold }), SEMIBOLD),
                (
                    script_eval!(vm, { mod.widgets.Label.draw_text.text_style }),
                    REGULAR,
                ),
                (
                    script_eval!(vm, { mod.widgets.TextInput.draw_text.text_style }),
                    REGULAR,
                ),
            ] {
                let style = TextStyle::script_from_value(vm, value);
                vm.with_cx_mut(|cx| style.ensure_fonts_loaded(cx));
                let fonts = vm.with_cx_mut(|cx| cx.get_global::<Rc<RefCell<Fonts>>>().clone());
                let family = fonts
                    .borrow_mut()
                    .get_or_load_font_family(style.font_family_id());
                assert_eq!(family.fonts()[0].id(), system_member_id(expected));
                assert!(
                    family.fonts().len() >= 2,
                    "PingFang plus the bundled Latin fallback; CJK and emoji may load lazily"
                );
                // Drawing calls this again. It must not silently discard the
                // system face when validating the script member count.
                vm.with_cx_mut(|cx| style.ensure_fonts_loaded(cx));
                assert_eq!(
                    fonts
                        .borrow_mut()
                        .get_or_load_font_family(style.font_family_id())
                        .fonts()[0]
                        .id(),
                    system_member_id(expected)
                );
            }
        });
    }

    #[test]
    fn stylesheet_switch_keeps_the_system_face_on_repeated_draws() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(|vm| {
            for dark in [false, true, false] {
                makepad_widgets::theme_mod(vm);
                makepad_widgets::desktop_style::install(
                    vm,
                    makepad_widgets::desktop_style::StyleSheet::load_with_appearance(
                        makepad_widgets::desktop_style::DesktopStyle::Macos,
                        dark,
                    ),
                );
                install(vm);
                makepad_widgets::widgets_mod(vm);
                makepad_widgets::desktop_style::apply_widgets(vm);
                let value = script_eval!(vm, {mod.widgets.Label.draw_text.text_style});
                let style = TextStyle::script_from_value(vm, value);
                for _ in 0..2 {
                    vm.with_cx_mut(|cx| style.ensure_fonts_loaded(cx));
                    let fonts = vm.with_cx_mut(|cx| cx.get_global::<Rc<RefCell<Fonts>>>().clone());
                    assert_eq!(
                        fonts
                            .borrow_mut()
                            .get_or_load_font_family(style.font_family_id())
                            .fonts()[0]
                            .id(),
                        system_member_id(REGULAR)
                    );
                }
            }
        });
    }
}
