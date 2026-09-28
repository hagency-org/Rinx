//! Native Octos action menu shared by desktop and mobile message layouts.
use makepad_widgets::*;
use super::{octos::Context, approval::ActionStyle};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    mod.widgets.OctosActionCard = #(OctosActionCard::register_widget(vm)) {
        ..mod.widgets.RoundedView
        visible: false width: Fill height: Fit flow: Down spacing: 8
        margin: Inset{top: 8 right: 10} padding: 12
        draw_bg +: {color: (mod.widgets.RBX_BG_SURFACE) border_radius: 4}
        octos_title := Label {width: Fill height: Fit flow: Flow.Right{wrap: true} draw_text.color: (mod.widgets.RBX_FG_PRIMARY)}
        octos_summary := Label {width: Fill height: Fit flow: Flow.Right{wrap: true} draw_text.color: (mod.widgets.RBX_FG_PRIMARY)}
        octos_status := Label {width: Fill height: Fit flow: Flow.Right{wrap: true} draw_text.color: (mod.widgets.RBX_FG_SECONDARY)}
        octos_buttons := View {width: Fill height: Fit flow: Flow.Right{wrap: true} spacing: 8
            octos_0 := AgentApprovalSecondaryButton {}
            octos_1 := AgentApprovalSecondaryButton {}
            octos_2 := AgentApprovalSecondaryButton {}
            octos_3 := AgentApprovalSecondaryButton {}
            octos_4 := AgentApprovalSecondaryButton {}
            octos_5 := AgentApprovalSecondaryButton {}
        }
    }
}
const SLOTS: [&[LiveId]; 6] = [
    ids!(octos_0),
    ids!(octos_1),
    ids!(octos_2),
    ids!(octos_3),
    ids!(octos_4),
    ids!(octos_5),
];
#[derive(Script, ScriptHook, Widget)]
pub struct OctosActionCard {
    #[source]
    source: ScriptObjectRef,
    #[deref]
    view: View,
    #[rust]
    context: Option<Context>,
    #[rust]
    expiry: Timer,
}
impl OctosActionCard {
    fn sync(&mut self, cx: &mut Cx) {
        let Some(context) = &self.context else {
            self.view.set_visible(cx, false);
            return;
        };
        self.view.set_visible(cx, true);
        let status = context.status();
        self.view
            .label(cx, ids!(octos_status))
            .set_visible(cx, status.is_some());
        self.view
            .label(cx, ids!(octos_status))
            .set_text(cx, crate::i18n::tr(status.unwrap_or_default()));
        let approval = context.payload.approval.as_ref();
        for (path, text) in [
            (ids!(octos_title), approval.map(|a| a.title.as_str())),
            (ids!(octos_summary), approval.map(|a| a.summary.as_str())),
        ] {
            self.view.label(cx, path).set_visible(cx, text.is_some());
            self.view
                .label(cx, path)
                .set_text(cx, text.unwrap_or_default());
        }
        for (i, path) in SLOTS.into_iter().enumerate() {
            let mut button = self.view.button(cx, path);
            let action = context.payload.actions.get(i);
            button.set_visible(cx, action.is_some());
            if let Some(action) = action {
                button.set_text(cx, crate::i18n::tr(&action.label));
                button.set_enabled(cx, status.is_none());
                let color = match action.style {
                    ActionStyle::Primary => vec4(0.05, 0.46, 0.30, 1.0),
                    ActionStyle::Danger => vec4(0.75, 0.13, 0.17, 1.0),
                    ActionStyle::Secondary => vec4(0.25, 0.28, 0.32, 1.0),
                };
                script_apply_eval!(cx,button,{draw_text +: {color: #(color) color_hover: #(color) color_down: #(color)} draw_bg +: {border_color: #(color) border_color_hover: #(color) border_color_down: #(color)}});
            }
        }
        self.view.redraw(cx);
    }
}
impl Widget for OctosActionCard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if self.expiry.is_event(event).is_some() {
            self.sync(cx);
        }
        if let Event::Actions(actions) = event {
            if actions
                .iter()
                .any(|a| a.downcast_ref::<super::octos::Changed>().is_some())
            {
                self.sync(cx);
            }
            for (i, path) in SLOTS.into_iter().enumerate() {
                if self.view.button(cx, path).clicked(actions)
                    && let Some(context) = &self.context
                    && let Some(action) = context.payload.actions.get(i)
                {
                    context.send(action);
                    self.sync(cx);
                    break;
                }
            }
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl OctosActionCardRef {
    pub fn populate(
        &self,
        cx: &mut Cx,
        room: &matrix_sdk::ruma::RoomId,
        event: &matrix_sdk_ui::timeline::EventTimelineItem,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            cx.stop_timer(inner.expiry);
            inner.context = Context::from_event(room, event);
            if let Some(expiry) = inner
                .context
                .as_ref()
                .and_then(|c| c.payload.approval.as_ref())
                .map(|a| a.expires)
            {
                let now = super::approval::current_unix_time_millis();
                if expiry > now {
                    inner.expiry = cx.start_timeout((expiry - now) as f64 / 1000.0);
                }
            }
            inner.sync(cx);
        }
    }
}
