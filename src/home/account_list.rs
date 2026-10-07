//! Shared account picker for the navigation menu, Settings and the login screen.
use makepad_widgets::*;
use crate::{
    accounts::{AccountAction, SavedAccount},
    sliding_sync::{account_changing, current_user_id},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    mod.widgets.AccountList = set_type_default() do #(AccountList::register_widget(vm)) {
        ..mod.widgets.View
        width: Fill, height: Fit, flow: Down, spacing: 4
        show_add: true
        account_container := View {
            width: Fill, height: Fit, flow: Down
            accounts := PortalList {
                width: Fill, height: 40, flow: Down
                Account := View {
                    width: Fill, height: 40
                    select := RobrixNeutralIconButton {
                        width: Fill, height: 40
                        icon_walk: Walk{width: 0, height: 0}
                        draw_text.text_style: (mod.widgets.RBX_TEXT_META)
                        text: ""
                    }
                }
            }
        }
        add := RobrixNeutralIconButton {
            width: Fill, height: 40
            icon_walk: Walk{width: 0, height: 0}
            text: #(crate::i18n::tr("Add Account")) i18n_text: "Add Account"
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AccountList {
    #[deref]
    view: View,
    #[live(true)]
    show_add: bool,
    #[rust]
    rows: Vec<SavedAccount>,
}
impl Widget for AccountList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            if account_changing() || crate::logout::logout_state_machine::is_logout_in_progress() {
                return;
            }
            if self.show_add && self.view.button(cx, ids!(add)).clicked(actions) {
                cx.action(AccountAction::Add);
            }
            for (index, row) in self
                .view
                .portal_list(cx, ids!(accounts))
                .items_with_actions(actions)
            {
                if row.button(cx, ids!(select)).clicked(actions) {
                    if let Some(account) = self.rows.get(index) {
                        cx.action(AccountAction::Select(account.user_id.clone()));
                    }
                }
            }
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let saved = crate::accounts::saved();
        if self.rows != saved {
            self.view.redraw(cx);
        }
        self.rows = saved;
        self.view
            .view(cx, ids!(account_container))
            .set_visible(cx, !self.rows.is_empty());
        let height = self.rows.len().min(4) as f64 * 40.0;
        let mut list = self.view.portal_list(cx, ids!(accounts));
        // Theme/language reapplication can restore the template height. Check
        // the actual walk rather than caching the last requested height.
        if list.walk(cx).height != Size::Fixed(height) {
            script_apply_eval!(cx, list, { height: #(height) });
        }
        self.view
            .button(cx, ids!(add))
            .set_visible(cx, self.show_add);
        self.view
            .button(cx, ids!(add))
            .set_enabled(cx, !account_changing());
        let active = current_user_id();
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.borrow_mut::<PortalList>() {
                list.set_item_range(cx, 0, self.rows.len());
                while let Some(index) = list.next_visible_item(cx) {
                    let Some(account) = self.rows.get(index) else {
                        continue;
                    };
                    let row = list.item(cx, index, id!(Account));
                    let is_active = active.as_ref() == Some(&account.user_id);
                    let text = if is_active {
                        format!("{} · {}", account.user_id, crate::i18n::tr("Active"))
                    } else {
                        account.user_id.to_string()
                    };
                    row.button(cx, ids!(select)).set_text(cx, &text);
                    row.button(cx, ids!(select))
                        .set_enabled(cx, !is_active && !account_changing());
                    row.draw_all(cx, scope);
                }
            }
        }
        DrawStep::done()
    }
}
