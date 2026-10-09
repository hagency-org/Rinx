//! Native fixture adapter: mouse coordinates drive touch events without a phone.
//! F8 injects a long press while held. This tests widget dispatch, not OS input.
use makepad_widgets::{makepad_platform::event::finger::{TouchPoint, TouchState, TouchUpdateEvent}, *};
use std::cell::Cell;

#[derive(Default)]
pub struct TouchInput {
    held: Option<(WindowId, DVec2, f64)>,
    uid: u64,
}

impl TouchInput {
    pub fn dispatch(&mut self, cx: &mut Cx, event: &Event, ui: &WidgetRef, area: Area) -> bool {
        if !std::env::args().any(|a| a == "--touch-input") { return false; }
        let (window_id, abs, time, state) = match event {
            Event::MouseDown(e) if e.button.is_primary() && area.clipped_rect(cx).contains(e.abs) => {
                self.uid += 1;
                self.held = Some((e.window_id, e.abs, e.time));
                (e.window_id, e.abs, e.time, TouchState::Start)
            }
            Event::MouseMove(e) if self.held.is_some() => {
                let (window, _, _) = self.held.unwrap();
                self.held = Some((window, e.abs, e.time));
                (window, e.abs, e.time, TouchState::Move)
            }
            Event::MouseUp(e) if e.button.is_primary() && self.held.is_some() => {
                self.held = None;
                (e.window_id, e.abs, e.time, TouchState::Stop)
            }
            Event::KeyDown(e) if e.key_code == KeyCode::F8 && self.held.is_some() => {
                let (window_id, abs, time) = self.held.unwrap();
                ui.handle_event(cx, &Event::LongPress(makepad_platform::event::finger::LongPressEvent {
                    window_id, abs, time: time + 0.6, uid: self.uid,
                }), &mut Scope::empty());
                return true;
            }
            _ => return false,
        };
        let mut touch = Event::TouchUpdate(TouchUpdateEvent {
            window_id, time, modifiers: KeyModifiers::default(),
            touches: vec![TouchPoint {state, abs, time, uid: self.uid, rotation_angle: 0.0,
                force: 1.0, radius: dvec2(1.0, 1.0), handled: Cell::new(Area::Empty), sweep_lock: Cell::new(Area::Empty)}],
        });
        ui.handle_event(cx, &touch, &mut Scope::empty());
        if state == TouchState::Stop {
            // OS backends normally release these captures after dispatch.
            let areas = cx.fingers.digit_capture_areas(live_id_num!(touch, self.uid).into());
            if let Event::TouchUpdate(e) = &mut touch { e.touches[0].state = TouchState::Start; }
            for area in areas { touch.unhandle(cx, &area); }
        }
        true
    }
}
