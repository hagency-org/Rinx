//! Opt-in Makepad timing capture for an isolated instrument run. No UI content,
//! account identifiers or credentials are recorded.
use makepad_widgets::*;
#[cfg(target_os = "android")]
use makepad_widgets::makepad_platform::event::finger::TouchState;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    fn ATrace_isEnabled() -> bool;
    fn ATrace_beginSection(name: *const std::ffi::c_char);
    fn ATrace_endSection();
}

/// Paired Android trace sections for a bounded, explicitly enabled capture.
/// Keep the guard on the event thread, including through early returns.
pub struct EventTrace {
    #[cfg(target_os = "android")]
    enabled: bool,
    _same_thread: std::marker::PhantomData<*mut ()>,
}

impl EventTrace {
    pub fn phase(&mut self, name: &'static std::ffi::CStr) {
        #[cfg(target_os = "android")]
        if self.enabled {
            unsafe {
                ATrace_endSection();
                ATrace_beginSection(name.as_ptr());
            }
        }
        #[cfg(not(target_os = "android"))]
        let _ = name;
    }
}

impl Drop for EventTrace {
    fn drop(&mut self) {
        #[cfg(target_os = "android")]
        if self.enabled {
            unsafe {
                ATrace_endSection(); // Current phase.
                ATrace_endSection(); // Event.
            }
        }
    }
}

pub fn trace_event(cx: &mut Cx, event: &Event) -> EventTrace {
    #[cfg(target_os = "android")]
    let enabled =
        output().is_some() && cx.global::<Probe>().active && unsafe { ATrace_isEnabled() };
    #[cfg(target_os = "android")]
    if enabled {
        let name = match event {
            Event::Draw(_) => c"rinx.draw",
            Event::NextFrame(_) => c"rinx.next_frame",
            Event::Actions(_) => c"rinx.actions",
            Event::Signal => c"rinx.signal",
            Event::Timer(_) => c"rinx.timer",
            Event::TextInput(_) => c"rinx.text_input",
            Event::TouchUpdate(_) => c"rinx.touch",
            Event::KeyDown(_) | Event::KeyUp(_) => c"rinx.key",
            _ => c"rinx.other",
        };
        unsafe {
            ATrace_beginSection(name.as_ptr());
            ATrace_beginSection(c"rinx.prepare".as_ptr());
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (cx, event);
    EventTrace {
        #[cfg(target_os = "android")]
        enabled,
        _same_thread: std::marker::PhantomData,
    }
}

#[derive(Default)]
struct Probe {
    active: bool,
    stop_timer: Timer,
    started: Option<Instant>,
    event: Option<(&'static str, Instant)>,
    phase: Option<Instant>,
    samples: BTreeMap<String, Vec<f64>>,
}

fn output() -> Option<&'static PathBuf> {
    static PATH: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        if let Some(path) = std::env::var_os("RINX_PERF_OUTPUT") {
            return Some(PathBuf::from(path));
        }
        // Android activities do not inherit shell environment variables. This
        // explicit intent passthrough is available only in instrument builds.
        #[cfg(target_os = "android")]
        if let Ok(config) = std::env::var("MAKEPAD_APP_CONFIG") {
            let value: serde_json::Value = serde_json::from_str(&config).ok()?;
            return value.get("rinx_perf_output")?.as_str().map(PathBuf::from);
        }
        None
    })
    .as_ref()
}

fn start(cx: &mut Cx) {
    let old_timer = cx.global::<Probe>().stop_timer;
    cx.stop_timer(old_timer);
    *cx.global::<Probe>() = Probe {
        active: true,
        started: Some(Instant::now()),
        ..Default::default()
    };
    cx.perf_monitor = Default::default();
    cx.perf_monitor.set_enabled(true);
}

fn stop(cx: &mut Cx, path: &PathBuf) {
    let mut frames = Vec::new();
    cx.perf_monitor.read(&mut frames);
    let channels: Vec<_> = cx
        .perf_monitor
        .channels()
        .iter()
        .map(|ch| ch.name.clone())
        .collect();
    let frame_count = cx.perf_monitor.frames_painted();
    cx.perf_monitor.set_enabled(false);
    let probe = cx.global::<Probe>();
    probe.active = false;
    let report = json!({
        "elapsed_ms": probe.started.map(|s| s.elapsed().as_secs_f64() * 1000.0),
        "events_and_phases_ms": probe.samples,
        "frames_painted": frame_count,
        "channels": channels,
        "recent_frames": frames.into_iter().filter(|f| f.gap_ms > 0.0).map(|f| json!({
            "gap_ms": f.gap_ms, "channel_us": f.channel_us,
        })).collect::<Vec<_>>(),
    });
    if let Err(error) = std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()) {
        error!("Could not write performance capture: {error}");
    }
}

pub fn begin(cx: &mut Cx, event: &Event) -> bool {
    let Some(path) = output() else { return false };
    if let Event::Custom(command) = event {
        if command == "rinx.perf.start" {
            start(cx);
            return true;
        }
        if command == "rinx.perf.stop" {
            let timer = cx.global::<Probe>().stop_timer;
            cx.stop_timer(timer);
            stop(cx, path);
            return true;
        }
    }
    // Capture a bounded native-touch run, including the fling after finger-up.
    // The measurement timer never changes the gesture's event delivery.
    #[cfg(target_os = "android")]
    if let Event::TouchUpdate(update) = event {
        if !cx.global::<Probe>().active
            && update.touches.iter().any(|t| t.state == TouchState::Start)
        {
            start(cx);
            let timer = cx.start_timeout(12.0);
            cx.global::<Probe>().stop_timer = timer;
        }
    }
    if cx.global::<Probe>().active
        && cx.global::<Probe>().stop_timer.0 != 0
        && cx.global::<Probe>().stop_timer.is_event(event).is_some()
    {
        stop(cx, path);
        return true;
    }
    let probe = cx.global::<Probe>();
    if probe.active {
        let kind = match event {
            Event::Draw(_) => "draw",
            Event::NextFrame(_) => "next_frame",
            Event::Actions(_) => "actions",
            Event::Signal => "signal",
            Event::Timer(_) => "timer",
            Event::TextInput(_) => "text_input",
            Event::MouseMove(_) => "mouse_move",
            Event::Scroll(_) => "scroll",
            Event::TouchUpdate(_) => "touch",
            Event::KeyDown(_) | Event::KeyUp(_) => "key",
            _ => "other",
        };
        let now = Instant::now();
        probe.event = Some((kind, now));
        probe.phase = Some(now);
    }
    false
}

pub fn phase(cx: &mut Cx, name: &str) {
    if output().is_none() {
        return;
    }
    let probe = cx.global::<Probe>();
    if !probe.active {
        return;
    }
    let Some(start) = probe.phase.replace(Instant::now()) else {
        return;
    };
    let us = start.elapsed().as_micros() as u64;
    let samples = probe.samples.entry(format!("phase.{name}")).or_default();
    if samples.len() < 100_000 {
        samples.push(us as f64 / 1000.0);
    }
    let channel = cx.perf_monitor.channel(name, 0x6699ff);
    cx.perf_monitor.add(channel, us);
}

pub fn end(cx: &mut Cx) {
    if output().is_none() {
        return;
    }
    let probe = cx.global::<Probe>();
    if !probe.active {
        return;
    }
    if let Some((kind, start)) = probe.event.take() {
        let samples = probe.samples.entry(format!("event.{kind}")).or_default();
        if samples.len() < 100_000 {
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
}
