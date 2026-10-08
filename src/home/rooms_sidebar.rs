//! The RoomsSideBar is the widget that contains the RoomsList and other items.
//!
//! It differs in what content it includes based on the adaptive view:
//! * On a narrow mobile view, it acts as the root_view of StackNavigation
//!   * It includes a title label, a search bar, and the RoomsList.
//! * On a wide desktop view, it acts as a permanent tab that is on the left side of the dock.
//!   * It only includes a title label and the RoomsList, because the SearcBar
//!     is at the top of the HomeScreen in Desktop view, and spaces are listed
//!     in the navigation rail rather than behind an "All Chats | Spaces" switch.

use makepad_widgets::*;

use crate::home::rooms_list::RoomsListWidgetExt;
use crate::home::rooms_list_header::RoomsListHeaderAction;
use crate::home::joined_spaces::JoinedSpacesWidgetExt;
use crate::home::navigation_tab_bar::{NavigationBarAction, SelectedTab};
use crate::shared::navigation_bar_button::NavigationBarButtonWidgetExt;
use crate::settings::app_preferences::{AppPreferencesGlobal, AppPreferencesAction, ViewModeOverride};
use crate::shared::room_filter_input_bar::{MainFilterAction, RoomFilterInputBarWidgetExt};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let ChatSwitchButton = NavigationBarButton {
        width: Fill height: 32
        draw_bg +: {color_hover: mod.widgets.RINX_HOVER color_active: mod.widgets.RINX_SURFACE border_radius: 5}
        label := Label {
            draw_text +: {color: mod.widgets.RINX_INK text_style: theme.font_regular {font_size: (11.5 * mod.widgets.RINX_TEXT_SCALE)}}
        }
    }
    let ChatViewSwitch = SolidView {
        width: Fill height: 46 flow: Right
        padding: Inset{left: 12 right: 12 bottom: 10 top: 2}
        draw_bg.color: MOBILE_BG
        RoundedView {
            width: Fill height: Fill flow: Right padding: 2
            draw_bg +: {color: mod.widgets.RINX_FIELD border_radius: 6}
            all_chats := ChatSwitchButton {label.text: #(crate::i18n::tr("All Chats")) label.i18n_text: "All Chats"}
            joined_spaces_tab := ChatSwitchButton {label.text: #(crate::i18n::tr("Spaces")) label.i18n_text: "Spaces"}
        }
    }

    mod.widgets.RoomsSideBar = #(RoomsSideBar::register_widget(vm)) {
        Desktop := SolidView {
            padding: Inset{top: 20, left: 10, right: 10}
            flow: Down, spacing: 5
            width: Fill, height: Fill

            draw_bg.color: (RBX_BG_SURFACE)

            CachedWidget {
                rooms_list_header := RoomsListHeader {}
            }
            // No "All Chats | Spaces" switch here: on desktop, spaces live in the
            // navigation rail on the left, and selecting one filters this list.
            file_transfer_entry := MobileRow {height: 46 title.text: #(crate::i18n::tr("File Transfer")) title.i18n_text: "File Transfer" icon.draw_icon.svg: ICON_FILE}
            conversations := View {
                width: Fill height: Fill
                CachedWidget {rooms_list := RoomsList {}}
            }
        },

        Mobile := SolidView {
            width: Fill height: Fill flow: Down
            draw_bg.color: mod.widgets.RINX_SURFACE
            title_bar := MobileTitle {
                title.text: #(crate::i18n::tr("Chats")) title.i18n_text: "Chats"
                controls.right.visible: true
            }
            search_area := SolidView {
                width: Fill height: 48
                padding: Inset{left: 10 right: 10 bottom: 10}
                draw_bg.color: MOBILE_BG
                room_filter_input_bar := RoomFilterInputBar {
                    height: 36
                    draw_bg +: {color: mod.widgets.RINX_SURFACE border_size: 0 border_radius: 5}
                    input +: {
                        empty_text: #(crate::i18n::tr("Search")) i18n_empty_text: "Search"
                        draw_text.text_style.font_size: (11.5 * mod.widgets.RINX_TEXT_SCALE)
                        draw_bg +: {color: mod.widgets.RINX_SURFACE color_hover: mod.widgets.RINX_SURFACE color_focus: mod.widgets.RINX_SURFACE color_empty: mod.widgets.RINX_SURFACE}
                    }
                }
            }
            ChatViewSwitch {}
            file_transfer_entry := MobileRow {height: 46 title.text: #(crate::i18n::tr("File Transfer")) title.i18n_text: "File Transfer" icon.draw_icon.svg: ICON_FILE}
            conversations := View {
                width: Fill height: Fill
                CachedWidget {rooms_list := RoomsList {}}
            }
            spaces_tree := View {
                width: Fill height: Fill visible: false
                CachedWidget {joined_spaces := JoinedSpaces {}}
            }
        }
    }
}

/// Shared across adaptive sidebar replacements, but reset when signing out.
#[derive(Default)]
struct ChatsViewState { spaces: bool }

/// A simple wrapper around `AdaptiveView` that contains several global singleton widgets.
///
/// * In the mobile view, it serves as the root view of the StackNavigation,
///   showing the title label, the search bar, and the RoomsList.
/// * In the desktop view, it is a permanent tab in the dock,
///   showing only the title label and the RoomsList
///   (because the search bar is at the top of the HomeScreen).
#[derive(Script, Widget)]
pub struct RoomsSideBar {
    #[deref] view: AdaptiveView,

    /// The most recently applied view-mode override.
    #[rust] applied_view_mode: ViewModeOverride,

    #[rust] mobile_search_collapsed: bool,
    #[rust] pending_search_focus: bool,
}

impl ScriptHook for RoomsSideBar {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            // Here we set the global singleton for the RoomsList widget,
            // which is used to access the list of rooms from anywhere in the app.
            cx.set_global(self.view.rooms_list(cx, ids!(rooms_list)));

            // The RoomsSideBar is re-instantiated every time the HomeScreen's
            // AdaptiveView switches between Desktop and Mobile view modes
            // (cuz it's not wrapped in a CachedWidget).
            // Thus we just re-read the current value here and apply it.
            let mode = cx.global::<AppPreferencesGlobal>().0.view_mode;
            self.apply_view_mode(mode);
        });
    }
}

impl RoomsSideBar {
    fn apply_view_mode(&mut self, mode: ViewModeOverride) {
        self.view.set_variant_selector(mode.variant_selector());
        self.applied_view_mode = mode;
    }

    fn active_mobile_list(&self, cx: &mut Cx) -> PortalListRef {
        if cx.global::<ChatsViewState>().spaces {
            self.view.portal_list(cx, ids!(joined_spaces.list))
        } else {
            self.view.portal_list(cx, ids!(rooms_list.list))
        }
    }

    fn update_mobile_search(&mut self, cx: &mut Cx) {
        if crate::home::home_screen::effective_is_desktop(cx) {
            self.pending_search_focus = false;
            return;
        }
        let list = self.active_mobile_list(cx);
        let collapsed = list.first_id() != 0 || list.scroll_position() < -0.5;
        self.view.view(cx, ids!(search_area)).set_visible(cx, !collapsed);
        self.view.button(cx, ids!(title_bar.controls.search)).set_visible(cx, collapsed);
        if collapsed != self.mobile_search_collapsed {
            self.mobile_search_collapsed = collapsed;
            if collapsed {
                let input = self.view.text_input(cx, ids!(room_filter_input_bar.input));
                if cx.has_key_focus(input.area()) {
                    cx.set_key_focus(Area::Empty);
                    cx.hide_text_ime();
                }
            }
            // Redraw the enclosing layout only when the header changes. The
            // list keeps its per-scroll actions disabled on the hot path.
            self.view.redraw(cx);
        }
    }

    fn open_mobile_search(&mut self, cx: &mut Cx) {
        if crate::home::home_screen::effective_is_desktop(cx) { return; }
        self.pending_search_focus = true;
        // This API cancels a running fling before returning to the beginning.
        self.active_mobile_list(cx).smooth_scroll_to(cx, 0, 1000.0, Some(1), 0.0);
        self.view.redraw(cx);
    }
}

impl Widget for RoomsSideBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // If the main room filter input bar changed keywords, re-emit that action
        // as a MainFilterAction so that other widgets can handle it.
        if let Event::Actions(actions) = event {
            if self.view.navigation_bar_button(cx, ids!(file_transfer_entry)).clicked(actions) {
                cx.action(crate::moments::ui::MomentsAction::FileTransfer);
            }
            if self.view.navigation_bar_button(cx, ids!(all_chats)).clicked(actions) {
                cx.global::<ChatsViewState>().spaces = false;
                cx.action(NavigationBarAction::GoToHome);
                self.view.redraw(cx);
            }
            if self.view.navigation_bar_button(cx, ids!(joined_spaces_tab)).clicked(actions) {
                cx.global::<ChatsViewState>().spaces = true;
                cx.action(NavigationBarAction::GoToHome);
                self.view.redraw(cx);
            }
            if self.view.button(cx, ids!(title_bar.controls.right)).clicked(actions) {
                cx.action(crate::home::navigation_tab_bar::NavigationBarAction::GoToTab(
                    crate::home::navigation_tab_bar::SelectedTab::Contacts,
                ));
            }
            if self.view.button(cx, ids!(title_bar.controls.search)).clicked(actions) {
                self.open_mobile_search(cx);
            }
            if let Some(keywords) = self.view.room_filter_input_bar(cx, ids!(room_filter_input_bar)).changed(actions) {
                cx.action(MainFilterAction::Changed(keywords));
            }

            for action in actions {
                if let Some(crate::logout::logout_confirm_modal::LogoutAction::ClearAppState {..}) = action.downcast_ref() {
                    cx.global::<ChatsViewState>().spaces = false;
                    self.pending_search_focus = false;
                }
                // A legacy desktop Space shortcut still opens its filtered list.
                if let Some(NavigationBarAction::TabSelected(SelectedTab::Space {..})) = action.downcast_ref() {
                    cx.global::<ChatsViewState>().spaces = false;
                }
                // The header's search icon: on mobile the filter bar lives right
                // here, so focus it. (Desktop hosts the bar in the HomeScreen.)
                if let Some(RoomsListHeaderAction::OpenRoomFilterModal) = action.downcast_ref() {
                    self.open_mobile_search(cx);
                }
                if let Some(AppPreferencesAction::ViewModeChanged(new_mode)) = action.downcast_ref() {
                    if *new_mode != self.applied_view_mode {
                        self.apply_view_mode(*new_mode);
                        self.view.redraw(cx);
                    }
                }
            }
        }
        self.view.handle_event(cx, event, scope);
        self.update_mobile_search(cx);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // The desktop layout has no spaces tree (the rail lists spaces instead).
        let spaces = cx.global::<ChatsViewState>().spaces
            && !crate::home::home_screen::effective_is_desktop(cx);
        self.view.view(cx, ids!(conversations)).set_visible(cx, !spaces);
        self.view.view(cx, ids!(spaces_tree)).set_visible(cx, spaces);
        self.view.navigation_bar_button(cx, ids!(all_chats)).set_selected(cx, !spaces);
        self.view.navigation_bar_button(cx, ids!(joined_spaces_tab)).set_selected(cx, spaces);
        self.view.joined_spaces(cx, ids!(joined_spaces)).set_active(cx, spaces);
        self.update_mobile_search(cx);
        let search_drawn = !self.mobile_search_collapsed;
        let step = self.view.draw_walk(cx, scope, walk);
        if step.is_done() {
            // Drawing may normalize/clamp the list after filtering or loading.
            self.update_mobile_search(cx);
            if self.pending_search_focus && search_drawn && !self.mobile_search_collapsed {
                self.pending_search_focus = false;
                self.view.text_input(cx, ids!(room_filter_input_bar.input)).take_key_focus(cx);
            }
        }
        step
    }
}
