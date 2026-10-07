//! Opt-in Makepad timing capture for an isolated instrument run. No UI content,
//! account identifiers or credentials are recorded.
use makepad_widgets::*;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

#[derive(Default)]
struct Probe {
    active: bool,
    started: Option<Instant>,
    event: Option<(&'static str, Instant)>,
    phase: Option<Instant>,
    samples: BTreeMap<String, Vec<f64>>,
}

fn output() -> Option<&'static PathBuf> {
    static PATH: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("RINX_PERF_OUTPUT").map(PathBuf::from))
        .as_ref()
}

pub fn begin(cx: &mut Cx, event: &Event) -> bool {
    let Some(path) = output() else { return false };
    if let Event::Custom(command) = event {
        if command == "rinx.perf.start" {
            *cx.global::<Probe>() = Probe {
                active: true,
                started: Some(Instant::now()),
                ..Default::default()
            };
            cx.perf_monitor = Default::default();
            cx.perf_monitor.set_enabled(true);
            return true;
        }
        if command == "rinx.perf.stop" {
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
            return true;
        }
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
