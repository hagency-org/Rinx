#!/usr/bin/env python3
"""Exercise native link cards with offline metadata and real pointer events."""
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
    parser.add_argument("--binary", type=Path, default=Path("target/debug/examples/chat_link_preview"))
    args = parser.parse_args()
    root = Path("target/chat-link-preview-regressions") / uuid.uuid4().hex
    root.mkdir(parents=True)
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    app = NativeApp(root, port, auto_login=False)
    app.output.mkdir(parents=True)
    app.log = (app.output / "native.log").open("w")
    app.process = subprocess.Popen([str(args.binary.resolve())],
        env=dict(os.environ, MAKEPAD_REMOTE=str(port), MAKEPAD_HIDE_WINDOWS="1", MAKEPAD_NO_FOCUS="1",
                 RINX_DATA_DIR=str((root / "profile").resolve()), RAYON_NUM_THREADS="1"),
        stdin=subprocess.DEVNULL, stdout=app.log, stderr=subprocess.STDOUT)
    report = {"passed": False, "checks": []}
    try:
        for _ in range(80):
            if app.process.poll() is not None:
                raise RuntimeError("Native link preview fixture exited")
            try:
                if app.request("/s")["pid"] == app.process.pid:
                    break
            except OSError:
                pass
            time.sleep(.25)
        app.wait_text("Ready")
        app.request("/g")  # Settle the initial layout and GPU frame.
        app.capture("cards")

        def widget(name):
            return next(w for w in app.snap() if w["i"] == name)

        def titles():
            return [w for w in app.snap() if w["i"] == "title_label"]

        def assert_inside(inner, outer):
            x, y, w, h = inner
            ox, oy, ow, oh = outer
            assert x >= ox - 1 and y >= oy - 1 and x + w <= ox + ow + 1 and y + h <= oy + oh + 1, (inner, outer)

        assert [w["t"] for w in titles()] == ["Preview title 1", "Preview title 2"]
        card = widget("card")["r"]
        for title in titles():
            assert_inside(title["r"], card)
        for description in [w for w in app.snap() if w["i"] == "description_label"]:
            assert_inside(description["r"], card)
        assert any(w["ty"] == "Image" for w in app.snap())
        assert widget("after")["r"][1] >= card[1] + card[3]
        report["checks"].append("metadata_cards_are_inside_widget_bounds_and_do_not_overlap_next_message")

        app.click_id("title_label")
        app.wait_text("https://example.org/1")
        report["checks"].append("card_opens_original_url")
        app.click_id("expand_button")
        assert len(titles()) == 3
        app.click_id("collapse_button")
        assert len(titles()) == 2
        report["checks"].append("expand_collapse_updates_layout_and_hides_extra_cards")

        app.click_id("replace")
        assert [w["t"] for w in titles()] == ["Replacement card"]
        assert not any(w["ty"] == "Image" for w in app.snap())
        app.click_id("title_label")
        app.wait_text("https://example.org/replacement")
        report["checks"].append("reused_card_updates_metadata_and_click_target")

        app.click_id("narrow")
        for title in titles():
            assert_inside(title["r"], widget("card")["r"])
        assert widget("card")["r"][2] == 240
        app.capture("narrow-card")
        report["checks"].append("narrow_card_keeps_text_inside_bounds")

        app.click_id("empty")
        assert [w["t"] for w in titles()] == ["example.org"]
        assert widget("card")["r"][3] < 80
        assert not any(w["i"] in {"description_label", "site_name_label"} for w in app.snap())
        app.capture("empty-preview")
        report["checks"].append("empty_metadata_uses_compact_hostname_without_blank_summary_or_thumbnail")

        app.click_id("clear")
        assert not titles()
        assert widget("after")["r"][1] < card[1] + card[3]
        report["checks"].append("clearing_links_removes_cards_and_reserved_height")
        app.capture("cleared")

        app.click_id("layouts")
        app.request("/g")
        app.capture("message-layouts")
        app.click_id("inspect")
        layout = json.loads(widget("result")["t"])
        for name, max_width in (("desktop", 760), ("compact", 760), ("own", 620), ("mobile", 240)):
            row = layout[name]
            left, width = row["text"]
            row_left, row_width = row["row"]
            assert 0 < width <= max_width + 1, (name, row)
            assert left >= row_left + 12 and left + width <= row_left + row_width - 12, (name, row)
        app.capture("message-layouts")
        report["checks"].append("desktop_compact_and_bubble_messages_have_bounded_width_and_right_gutter")
        report["passed"] = True
    finally:
        app.process.terminate()
        app.process.wait(timeout=10)
        app.log.close()
        (root / "report.json").write_text(json.dumps(report, indent=2))
        (root / "trace.json").write_text(json.dumps(app.trace, indent=2))
        print(json.dumps({"report": str(root / "report.json"), **report}))


if __name__ == "__main__":
    main()
