#!/usr/bin/env python3
"""Native server selection/routing checks; no public accounts are created.

Uses hidden, isolated Rinx instances and loopback fixtures. --live-public also
checks public discovery and the presented registration actions, never submits
credentials or opens a browser. Reuses the native user-intervention guard.
"""
import argparse
import json
from pathlib import Path
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from native_server_history import App


class Server:
    def __init__(self, mode="native", delay=0):
        self.mode = mode
        self.requests = []
        self.login_requested = threading.Event()
        fixture = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                fixture.requests.append(self.path)
                if self.path.endswith("/versions"):
                    value = {"versions": ["v1.12"], "unstable_features": {}}
                elif self.path == "/.well-known/matrix/client":
                    value = {"m.homeserver": {"base_url": fixture.url}}
                elif self.path.endswith("/login"):
                    fixture.login_requested.set()
                    time.sleep(delay)
                    flows = [{"type": "m.login.password"}]
                    if fixture.mode != "native":
                        flows.append({"type": "m.login.sso", "oauth_aware_preferred": True})
                    value = {"flows": flows}
                elif self.path.endswith("/auth_metadata"):
                    value = {"prompt_values_supported": ["login", "create"] if fixture.mode == "browser" else ["login"]}
                else:
                    value = {}
                data = json.dumps(value).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                try:
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError):
                    pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.url = f"http://127.0.0.1:{self.server.server_port}/"
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def stop(self):
        self.server.shutdown()
        self.server.server_close()


def widget(app, name):
    return next(w for w in app.snap() if w["i"] == name)


def choose(app, name):
    row = next(w for w in app.snap() if w["i"] == "public_server_button" and w.get("t") == name)
    x, y, width, height = row["r"]
    assert height >= 44
    app.click(x + width / 2, y + height / 2)
    assert widget(app, "homeserver_input")["t"] == name


def fill_field(app, name, value):
    app.click_id(name)
    app.request("/k", c="KeyA", cmd=1, wait=0)
    app.request("/k", c="Backspace", wait=0)
    app.request("/t", t=value, wait=1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--live-public", action="store_true")
    args = parser.parse_args()
    servers = [Server(), Server(), Server("browser"), Server("disabled"), Server(delay=2)]
    first, second, browser, disabled, slow = servers
    report = {"passed": False, "checks": [], "live_discovery_seconds": {}}
    app = None
    try:
        for name, size in [("desktop", (1000, 820)), ("phone", (390, 844))]:
            with socket.socket() as port:
                port.bind(("127.0.0.1", 0))
                bridge_port = port.getsockname()[1]
            app = App(args.output / name, port=bridge_port, size=size, auto_login=False)
            app.launch(args.binary)
            app.wait_text("Mozilla community")
            # The initial title snapshot can precede PortalList materialization.
            # Wait for its rows and the first Metal frame before inspection.
            time.sleep(1)
            public = sorted((w for w in app.snap() if w["i"] == "public_server_button"), key=lambda w: w["r"][1])
            assert [w["t"] for w in public] == ["matrix.org", "tchncs.de", "mozilla.org"], public
            for row in public + [widget(app, "continue_server_button")]:
                x, y, width, height = row["r"]
                assert x >= 0 and x + width <= size[0] and y >= 0 and y + height <= size[1], row
            app.capture("server-choices-en")
            pixels = " ".join(row["text"] for row in app.ocr())
            for description in ["Matrix.org Foundation", "Independent community server", "Mozilla community"]:
                assert description in pixels, pixels
            for entry in ["matrix.org", "tchncs.de", "mozilla.org"]:
                choose(app, entry)
            assert not (app.root / "profile/homeserver_history.json").exists()
            report["checks"].append(f"{name}: all choices and Continue fit; selecting fills the address without adding unverified history")

            app.click_id("login_language_zh")
            app.wait_text("选择公共服务器")
            app.wait_text("独立社区服务器")
            app.capture("server-choices-zh")
            app.click_id("login_language_en")

            app.fill(first.url)
            app.continue_server()
            assert widget(app, "selected_server")["t"] == first.url.rstrip("/")
            app.click_id("register_option_button")
            app.wait_text("Create an account on this server.")
            fill_field(app, "registration_username", "local_fixture")
            fill_field(app, "registration_password", "fixture-password")
            app.capture("native-registration")
            app.click_id("edit_server_button")
            app.fill(second.url)
            app.continue_server()
            app.click_id("register_option_button")
            assert widget(app, "registration_username")["t"] == "Choose a username"
            assert widget(app, "registration_password")["t"] == "Choose a password"
            app.click_id("back_to_login_button")
            assert widget(app, "register_option_button")["t"] == "Create an account"
            report["checks"].append(f"{name}: custom native registration, return to sign-in, and clearing details on server switch")

            app.click_id("edit_server_button")
            app.fill(browser.url)
            app.continue_server()
            assert widget(app, "register_option_button")["t"] == "Create account in browser"
            app.capture("browser-registration")
            app.click_id("edit_server_button")
            app.fill(disabled.url)
            app.continue_server()
            app.wait_text("has not advertised an account creation method")
            assert not any(w["i"] == "register_option_button" for w in app.snap())
            report["checks"].append(f"{name}: browser and unavailable registration follow live capabilities, not password compatibility")

            app.click_id("edit_server_button")
            app.fill(slow.url)
            slow.login_requested.clear()
            app.click_id("continue_server_button")
            assert slow.login_requested.wait(5)
            choose(app, "tchncs.de")
            time.sleep(2.2)
            assert widget(app, "homeserver_input")["t"] == "tchncs.de"
            assert not any(w["i"] == "selected_server" for w in app.snap())
            assert slow.url not in app.history()
            report["checks"].append(f"{name}: selecting another server invalidates a pending discovery response")

            if args.live_public and name == "desktop":
                for server, action in [("matrix.org", "Create account in browser"), ("tchncs.de", "Create account in browser"), ("mozilla.org", "Join on the server website")]:
                    choose(app, server)
                    started = time.monotonic()
                    app.continue_server()
                    report["live_discovery_seconds"][server] = round(time.monotonic() - started, 2)
                    assert widget(app, "selected_server")["t"] == server
                    assert widget(app, "register_option_button")["t"] == action
                    app.capture(server.replace(".", "-") + "-registration")
                    app.click_id("edit_server_button")
                report["checks"].append("public servers: live SDK discovery reaches all three selected services and offers the expected action; no account submissions")
            app.stop()
            app = None
        report["passed"] = True
    except Exception:
        if app:
            try:
                app.capture("failure")
            except Exception:
                pass  # Preserve the original failure if no frame exists yet.
        raise
    finally:
        if app:
            app.stop()
        for server in servers:
            server.stop()
        args.output.mkdir(parents=True, exist_ok=True)
        (args.output / "result.json").write_text(json.dumps(report, indent=2))
        print(json.dumps(report))


if __name__ == "__main__":
    main()
