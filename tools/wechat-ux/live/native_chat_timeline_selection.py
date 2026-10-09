#!/usr/bin/env python3
"""Check message selection and scrolling together in the production timeline."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import uuid
from native_probe import NativeApp


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/examples/chat_timeline_selection"))
    parser.add_argument("--touch-input", action="store_true", help="Exercise widget touch events on the native host; not a physical-device test")
    args = parser.parse_args()
    root = Path("target/chat-timeline-selection-regressions") / uuid.uuid4().hex
    root.mkdir(parents=True)
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    app = NativeApp(root, port, auto_login=False)
    app.output.mkdir(parents=True)
    app.log = (app.output / "native.log").open("w")
    app.process = subprocess.Popen([str(args.binary.resolve()), *(["--touch-input"] if args.touch_input else [])],
        env=dict(os.environ, MAKEPAD_REMOTE=str(port), MAKEPAD_HIDE_WINDOWS="1", MAKEPAD_NO_FOCUS="1",
                 RINX_DATA_DIR=str((root / "profile").resolve()), RAYON_NUM_THREADS="1"),
        stdin=subprocess.DEVNULL, stdout=app.log, stderr=subprocess.STDOUT)
    report = {"passed": False, "checks": []}
    try:
        for _ in range(80):
            if app.process.poll() is not None:
                raise RuntimeError("Timeline fixture exited")
            try:
                if app.request("/s")["pid"] == app.process.pid:
                    break
            except OSError:
                pass
            time.sleep(.25)
        app.wait_text("Ready")
        app.capture("ready")

        def state():
            app.click_id("inspect")
            return json.loads(next(w["t"] for w in app.snap() if w["i"] == "result"))

        def position(s):
            return s["first_id"], s["scroll"]

        def selection(s, index):
            return next(row["text"] for row in s["selections"] if row["index"] == index)

        def body(index):
            snap = app.snap()
            label = next(w["r"] for w in snap if w.get("t") == f"Row {index}")
            return next(w["r"] for w in snap if w["i"] == "body" and label[1] <= w["r"][1] < label[1] + 50)

        def drag(points):
            time.sleep(.55)
            app.request("/m", k="down", x=points[0][0], y=points[0][1], wait=1)
            # A held press followed by several vertical moves reproduces the conflict.
            time.sleep(.2)
            for x, y in points[1:]:
                app.request("/m", k="move", x=x, y=y, wait=1)
            app.request("/m", k="up", x=points[-1][0], y=points[-1][1], wait=1)

        if args.touch_input:
            before = state()
            x, y, w, h = body(6)
            app.request("/m", k="down", x=x + 15, y=y + 8, wait=1)
            app.request("/k", c="F8", wait=1)
            app.request("/m", k="up", x=x + 15, y=y + 8, wait=1)
            selected = state()
            assert selection(selected, 6) == "Alpha" and position(selected) == position(before), selected
            handles = next(row["handles"] for row in selected["selections"] if row["index"] == 6)
            assert handles
            app.capture("touch-long-press-word")
            report["checks"].append("touch_long_press_selects_word_and_shows_handles")
            end = handles[1]
            knob = (end[0] + 16, end[1] + end[3] - 8)
            drag([knob, (knob[0] + 80, knob[1] + 10), (knob[0] + 150, knob[1] + 30)])
            after = state()
            assert position(after) == position(before), (before, after)
            assert "中文" in selection(after, 6) and "\n" in selection(after, 6), after
            report["checks"].append("touch_handle_drag_extends_across_lines_without_scrolling")
            app.capture("touch-range-handles")

            x, y, w, h = body(5)
            app.request("/m", k="down", x=x + 10, y=y + 12, wait=1)
            app.request("/k", c="F8", wait=1)
            app.request("/m", k="up", x=x + 10, y=y + 12, wait=1)
            after = state()
            assert selection(after, 5) == "Selectable" and after["link_clicks"] == 0, after
            report["checks"].append("touch_long_press_on_link_selects_text_without_opening_url")

            x, y, _, _ = body(6)
            drag([(x + 200, y + 10), (x + 200, y - 30), (x + 200, y - 70)])
            after = state()
            assert position(after) != position(before), after
            report["checks"].append("ordinary_touch_swipe_over_text_still_scrolls")
            report["passed"] = True
            return

        before = state()
        x, y, w, h = body(6)
        drag([(x, y + 8), (x + 50, y + 15), (x + 180, y + h - 3)])
        after = state()
        report["plain_drag"] = {"before": before, "after": after}
        app.capture("plain-drag")
        assert position(after) == position(before), report["plain_drag"]
        assert "中文" in selection(after, 6) and "\n" in selection(after, 6), after
        report["checks"].append("held_multiline_plaintext_drag_selects_without_scrolling_timeline")

        x, y, w, h = body(5)
        drag([(x + 1, y + 12), (x + 70, y + 20), (x + 180, y + h - 3)])
        after = state()
        assert position(after) == position(before), after
        assert "Selectable link text" in selection(after, 5) and "Second line" in selection(after, 5), after
        assert after["link_clicks"] == 0, after
        app.capture("rich-link-drag")
        report["checks"].append("rich_link_drag_selects_without_scrolling_or_opening_link")

        x, y, w, h = body(6)
        drag([(x, y + 8), (x + w + 10, y + h + 50)])
        after = state()
        assert position(after) == position(before) and selection(after, 6).endswith("Third line ends here."), after
        report["checks"].append("drag_outside_message_keeps_selection_capture_and_timeline_position")

        time.sleep(.55)
        app.request("/click", x=x + 15, y=y + 8, wait=1)
        app.request("/click", x=x + 15, y=y + 8, wait=1)
        after = state()
        assert position(after) == position(before) and selection(after, 6) == "Alpha", after
        report["checks"].append("double_click_selects_word_inside_timeline")

        x, y, _, _ = body(5)
        app.click(x + 15, y + 12)
        assert state()["link_clicks"] == 1
        report["checks"].append("stationary_link_click_still_opens_url")

        x, y, _, _ = body(6)
        app.request("/m", k="scroll", x=x + 30, y=y + 10, dy=140, wait=1)
        time.sleep(.5)
        assert position(state()) != position(before)
        report["checks"].append("mouse_wheel_still_scrolls_over_message_text")

        # Empty timeline margins still allow mouse drag scrolling.
        app.click_id("reset")
        time.sleep(.6)
        before = state()
        rect = next(w["r"] for w in app.snap() if w["i"] == "list")
        x, y = rect[0] + 10, rect[1] + 250
        drag([(x, y), (x, y - 50), (x, y - 100)])
        assert position(state()) != position(before)
        report["checks"].append("empty_timeline_margin_still_allows_drag_scrolling")

        app.click_id("reset")
        before = state()
        for _ in range(3):
            app.request("/m", k="scroll", x=x, y=y, dy=180, wait=1)
        app.click_id("reset")
        after = state()
        assert after["row6_uid"] == before["row6_uid"], (before, after)
        report["checks"].append("recent_message_widget_is_retained_when_scrolling_back")

        for _ in range(14):
            app.request("/m", k="scroll", x=x, y=y, dy=1000, wait=1)
            after = state()
            # The 32 recent identities plus the visible viewport are bounded.
            assert after["retained_items"] <= 38, after
        assert after["first_id"] > 40, after
        assert after["row6_uid"] is None, after
        app.click_id("reset")
        after = state()
        assert after["row6_uid"] != before["row6_uid"], (before, after)
        assert not selection(after, 6), after
        report["checks"].append("cache_evicts_old_messages_and_recreated_message_has_no_stale_selection")
        report["passed"] = True
    finally:
        app.stop()
        (root / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False))
        (root / "trace.json").write_text(json.dumps(app.trace, indent=2))
        print(json.dumps({"report": str(root / "report.json"), **report}, ensure_ascii=False))


if __name__ == "__main__":
    main()
