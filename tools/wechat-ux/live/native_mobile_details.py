#!/usr/bin/env python3
"""Exercise mobile details using an isolated Palpo profile, never a personal account.

Captures Settings, Personal Information, preferences, and both contact pages.
Verifies an actual Matrix display-name write, cancel, preference persistence,
photo/delete and logout confirmations, and blocked-user actions. No visual score
is inferred from successful functional checks.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import uuid
import time
import urllib.parse

from native_probe import NativeApp
from seed import checked


def main():
    os.environ.update(MAKEPAD_HIDE_WINDOWS="1", MAKEPAD_NO_FOCUS="1")
    os.environ.pop("MAKEPAD_FOCUS", None)
    source = Path("lab/wechat-ux/evidence/live")
    root = source / "mobile-details"
    root.mkdir(mode=0o700, exist_ok=True)
    shutil.copyfile(source / "fixture.json", root / "fixture.json")
    os.chmod(root / "fixture.json", 0o600)
    # This directory belongs only to this fixture runner. Never change a live
    # instrumented profile or touch the visible personal account.
    with socket.socket() as probe:
        if probe.connect_ex(("127.0.0.1", 8199)) == 0:
            raise RuntimeError("Native bridge already occupied")
    def restore_fixture_preferences():
        for state_path in (root / "profile").rglob("latest_app_state.json"):
            state = json.loads(state_path.read_text())
            state["app_prefs"]["read_receipts_privacy"] = "Everyone"
            state["app_prefs"]["thumbnail_max_height"] = "Medium"
            state["app_prefs"]["ui_zoom"] = 1.0
            state_path.write_text(json.dumps(state))
    restore_fixture_preferences()
    fixture = json.loads((root / "fixture.json").read_text())
    user = fixture["users"]["alex"]
    profile_route = "profile/" + urllib.parse.quote(user["user_id"], safe="") + "/displayname"
    initial_name = checked(fixture["url"], "GET", profile_route)["displayname"]
    name_changed = False
    app = NativeApp(root, size=(375, 812))
    result = {"passed": False, "checks": [], "evidence": str(app.output)}

    def passed(name):
        result["checks"].append({"name": name, "passed": True})
        print("PASS", name, flush=True)

    def native_text(text):
        app.wait_text(text, pixels=True)

    def replace_name(value):
        app.click_id("display_name_input")
        app.request("/k", c="A", cmd=1, wait=1)
        app.request("/t", t=value, wait=1)

    def wait_name(value):
        for _ in range(60):
            if checked(fixture["url"], "GET", profile_route).get("displayname") == value:
                return
            time.sleep(.25)
        raise AssertionError("Display name did not reach Matrix")

    def back():
        app.click_id("back")

    try:
        app.start()
        app.wait_text("Emma Wilson", timeout=90)
        app.click_text("Me")
        app.click_id("settings")
        native_text("Account Settings")
        app.capture("settings-account")
        replace_name("Cancelled draft")
        app.click_id("cancel_display_name_button")
        assert checked(fixture["url"], "GET", profile_route)["displayname"] == initial_name
        passed("cancel_name_keeps_matrix_profile")
        native_text(initial_name)
        replacement = "Alex 陈 · mobile"
        replace_name(replacement)
        app.capture("name-edit")
        app.click_id("accept_display_name_button")
        name_changed = True
        wait_name(replacement)
        app.wait_text(replacement)
        app.capture("name-saved")
        passed("save_bilingual_name_round_trips_through_matrix")
        replace_name(initial_name)
        app.click_id("accept_display_name_button")
        wait_name(initial_name)
        name_changed = False
        app.click_id("delete_avatar_button")
        native_text("Delete Avatar")
        app.click_text("Cancel")
        native_text("Account Settings")
        passed("photo_removal_requires_existing_confirmation")
        app.click_id("category_privacy_button")
        native_text("Privacy Settings")
        app.capture("privacy")
        app.click_id("category_preferences_button")
        native_text("App appearance")
        app.capture("preferences")
        def scroll_to(widget):
            for _ in range(16):
                if any(w["i"]==widget and 160<w["r"][1] and w["r"][1]+w["r"][3]<760 for w in app.snap()):
                    return
                app.request("/m",k="scroll",x=220,y=440,dy=240,wait=1)
                time.sleep(.3)
            raise AssertionError("Setting not reachable: "+widget)
        scroll_to("ui_zoom_plus_button")
        app.click_id("ui_zoom_plus_button")
        time.sleep(1)
        app.wait_text("110%")
        app.click_id("ui_zoom_minus_button")
        app.wait_text("100%")
        time.sleep(1)
        app.capture("zoom-restored")
        scroll_to("read_receipts_privacy_dropdown")
        app.click_id("read_receipts_privacy_dropdown")
        app.request("/k",c="ArrowDown",wait=1)
        app.request("/m",k="scroll",x=220,y=440,dy=-10000,wait=1)
        scroll_to("thumb_small_radio")
        app.click_id("thumb_small_radio")
        app.click_id("category_about_button")
        native_text("Version")
        app.capture("about")
        app.click_id("category_account_button")
        scroll_to("logout_button")
        app.click_id("logout_button")
        native_text("Cancel")
        app.click_text("Cancel")
        native_text("Settings")
        passed("logout_uses_existing_confirmation")
        app.click_id("close_button")
        app.click_id("own_profile")
        native_text("Account Settings")
        app.capture("direct-personal")
        app.click_id("close_button")
        native_text("Me")
        passed("own_profile_opens_account_settings_and_close_returns_to_me")
        app.click_text("Contacts")
        native_text("Emma Wilson")
        app.click_text("Emma Wilson")
        native_text("Contact Info")
        app.capture("contact")
        app.click_id("contact_copy")
        native_text("Profile link copied.")
        app.click_id("contact_block")
        native_text("Block this user?")
        app.click_text("Cancel")
        native_text("Contact Info")
        passed("contact_copy_and_block_confirmation")
        marker = "Profile detail check " + uuid.uuid4().hex[:6]
        checked(fixture["url"], "PUT", f"rooms/{fixture['rooms']['emma']}/send/m.room.message/{uuid.uuid4().hex}",
                {"msgtype": "m.text", "body": marker}, token=fixture["users"]["emma"]["access_token"])
        app.click_id("message")
        native_text("Message (unencrypted)")
        native_text(marker)
        # StackNavigation's pushed views are not traversed by this Makepad
        # revision's /snap. Locate the fixture message in the actual pixels and
        # click its adjacent incoming avatar with native pointer input.
        incoming = next(row for row in app.ocr() if marker in row["text"])
        y = (incoming["box"][1] + incoming["box"][3] / 2) * 812
        app.click(32, y)
        native_text("Contact Info")
        native_text("Last Read Message")
        time.sleep(.7)
        app.capture("chat-avatar-details")
        title = next(row for row in app.ocr() if row["text"] == "Contact Info")
        x, y, w, h = title["box"]
        assert abs((x+w/2)*375 - 187.5) < 10, title
        assert (y+h/2)*812 < 80, "Chat header still above the mobile detail page"
        app.click(24, 56)
        native_text("Message (unencrypted)")
        passed("chat_avatar_opens_full_width_mobile_details_and_closes")
        app.stop()
        states = list((root / "profile").rglob("latest_app_state.json"))
        prefs = json.loads(states[0].read_text())["app_prefs"]
        assert prefs["read_receipts_privacy"] == "OnlyMyDevices", prefs["read_receipts_privacy"]
        assert prefs["thumbnail_max_height"] == "Small", prefs["thumbnail_max_height"]
        assert prefs["ui_zoom"] == 1.0
        passed("privacy_image_size_and_zoom_preferences_persist")
        result["passed"] = True
    finally:
        app.stop()
        restore_fixture_preferences()
        if name_changed:
            checked(fixture["url"], "PUT", profile_route, {"displayname": initial_name}, token=user["access_token"])
        result["trace"] = app.trace
        (source / "native-mobile-details.json").write_text(json.dumps(result, indent=2))
        (app.output / "result.json").write_text(json.dumps(result, indent=2))
        print("receipt", source / "native-mobile-details.json", flush=True)


if __name__ == "__main__":
    main()
