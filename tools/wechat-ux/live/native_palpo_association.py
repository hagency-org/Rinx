#!/usr/bin/env python3
"""Real native Rinx, Rust Palpo and Hagency association/probe; Matrix HTTP fixture."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import threading
import time
from urllib.parse import urlsplit, unquote, parse_qs
from urllib.request import Request
import uuid

from native_palpo_coordinator import HTTP, call, launch, post, port, fill
from native_palpo import inspect


def save_dialog(app, destination=None):
    """Operate only the owned Rinx process's AppKit panel, never another app."""
    app.click_id('export', modal=True)
    helper = Path(__file__).with_name('native_save_panel.swift')
    executable = app.output / 'native-save-panel'
    subprocess.run(['swiftc', str(helper), '-o', str(executable)], check=True, capture_output=True, timeout=60)
    capture = app.output / ('native-save-panel-save.png' if destination else 'native-save-panel-cancel.png')
    result = subprocess.run([str(executable), str(app.process.pid), str(capture)] + ([str(destination)] if destination else []),
                            text=True, capture_output=True, timeout=25)
    if result.returncode:
        raise AssertionError(result.stderr.strip())
    if result.stdout.strip() != 'done':
        raise AssertionError('System save panel automation did not finish: ' + result.stdout[:2000])
    app.wait_text('Configuration saved' if destination else 'Save cancelled')


class Matrix(BaseHTTPRequestHandler):
    registrations = {}
    rooms = {}
    aliases = {}
    events = {}
    endpoint = ""

    def log_message(self, *_):
        pass

    def reply(self, value, status=200):
        raw = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        self.handle_request()

    def do_POST(self):
        self.handle_request()

    def do_PUT(self):
        self.handle_request()

    def handle_request(self):
        uri = urlsplit(self.path)
        path = [unquote(p) for p in uri.path.split("/")[1:]]
        token = self.headers.get("Authorization", "").removeprefix("Bearer ")
        user = next((f"@{name}:example.test" for name in ("owner", "coordinator", "admin") if token == name + "-secret"), None)
        registration = next((v for v in self.registrations.values() if v["as_token"] == token), None)
        if registration:
            if registration.get("disabled"):
                return self.reply({"errcode": "M_UNKNOWN_TOKEN"}, 401)
            user = parse_qs(uri.query).get("user_id", [f'@{registration["sender_localpart"]}:example.test'])[0]
        if not user:
            return self.reply({"errcode": "M_UNKNOWN_TOKEN"}, 401)
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"]))) if self.headers.get("Content-Length") else {}
        if path[-1] == "whoami":
            return self.reply({"user_id": user})
        if path[:4] == ["_matrix", "client", "v3", "profile"]:
            return self.reply({"displayname": path[-1].split(":")[0][1:]})
        if path[:4] == ["_palpo", "admin", "v1", "appservices"]:
            if user != "@admin:example.test":
                return self.reply({"errcode": "M_FORBIDDEN"}, 403)
            if len(path) == 4:
                if self.command == "POST":
                    self.registrations[body["id"]] = body
                    return self.reply({})
                return self.reply({"appservices": [{"id": key} for key in self.registrations]})
            value = self.registrations.get(path[4])
            if value and len(path) == 6:
                if path[5] in ("disable", "enable") and self.command == "POST":
                    value["disabled"] = path[5] == "disable"
                    return self.reply({})
                if path[5] == "url" and self.command == "PUT":
                    assert value["url"] == body["expected_url"]
                    value["url"] = body["url"]
                    return self.reply({})
            return self.reply(value or {"errcode": "M_NOT_FOUND"}, 200 if value else 404)
        if path[:4] == ["_palpo", "admin", "v2", "users"]:
            identity = path[4]
            appservice = next((key for key in self.registrations if identity.startswith("@" + key + "_")), None)
            return self.reply({"name": identity, "appservice_id": appservice, "deactivated": False, "locked": False})
        if path[-1] == "createRoom":
            room = f"!room{len(self.rooms)+1}:example.test"
            state = body["initial_state"] + [{"type": "m.room.create", "state_key": "", "sender": user, "content": {"room_version": "11"}},
                {"type": "m.room.power_levels", "state_key": "", "content": body["power_level_content_override"]},
                {"type": "m.room.member", "state_key": user, "content": {"membership": "join"}}]
            state += [{"type": "m.room.member", "state_key": member, "content": {"membership": "invite"}} for member in body["invite"]]
            self.rooms[room] = state
            self.aliases["#" + body["room_alias_name"] + ":example.test"] = room
            return self.reply({"room_id": room})
        if "directory" in path:
            room = self.aliases.get(path[-1])
            return self.reply({"room_id": room} if room else {"errcode": "M_NOT_FOUND"}, 200 if room else 404)
        if "join" in path:
            room = path[-1]
            for event in self.rooms[room]:
                if event["type"] == "m.room.member" and event["state_key"] == user:
                    event["content"]["membership"] = "join"
            return self.reply({"room_id": room})
        if path[-1] == "joined_rooms":
            return self.reply({"joined_rooms": list(self.rooms)})
        if "rooms" in path:
            room = path[4]
            if path[5] == "state":
                if len(path) == 6:
                    return self.reply(self.rooms.get(room, []))
                event = next((e for e in self.rooms.get(room, []) if e["type"] == path[6] and e["state_key"] == (path[7] if len(path) > 7 else "")), None)
                return self.reply(event["content"] if event else {"errcode": "M_NOT_FOUND"}, 200 if event else 404)
            if path[5] == "joined_members":
                return self.reply({"joined": {e["state_key"]: {} for e in self.rooms.get(room, []) if e["type"] == "m.room.member" and e["content"]["membership"] == "join"}})
            if path[5] == "send":
                key = (room, path[6], path[7])
                if key not in self.events:
                    self.events[key] = {"type": path[6], "sender": user, "room_id": room, "event_id": "$" + uuid.uuid4().hex, "content": body}
                event = self.events[key]
                if event["type"] == "com.hagency.connection.probe.v1":
                    def relay():
                        url = self.endpoint + f'/api/relay/v2/{registration["id"]}/transactions/{path[7]}'
                        for _ in range(20):
                            try:
                                with HTTP.open(Request(url, data=json.dumps({"events": [event]}).encode(), method="PUT",
                                        headers={"Content-Type": "application/json", "Authorization": "Bearer " + registration["hs_token"]}), timeout=5):
                                    return
                            except OSError:
                                time.sleep(.1)
                        raise RuntimeError("Probe relay never acknowledged")
                    threading.Thread(target=relay, daemon=True).start()
                return self.reply({"event_id": event["event_id"]})
            if path[5] == "event":
                event = next((e for e in self.events.values() if e["event_id"] == path[6] and e["room_id"] == room), None)
                return self.reply(event or {"errcode": "M_NOT_FOUND"}, 200 if event else 404)
        return self.reply({"errcode": "M_NOT_FOUND"}, 404)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--hagency", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--credential-controls", action="store_true")
    parser.add_argument("--native-export", action="store_true")
    args = parser.parse_args()
    root = Path("target/palpo-association-validation") / uuid.uuid4().hex
    root.mkdir(parents=True)
    root = root.resolve()
    backend, hagency = args.backend.resolve(), args.hagency.resolve()
    matrix = ThreadingHTTPServer(("127.0.0.1", 0), Matrix)
    threading.Thread(target=matrix.serve_forever, daemon=True).start()
    matrix_origin = f"http://127.0.0.1:{matrix.server_port}"
    endpoint = f"http://127.0.0.1:{port()}"
    Matrix.endpoint = endpoint
    env = {k: v for k, v in os.environ.items() if not k.startswith("PALPO_")}
    env.update(PALPO_SERVER_NAME="example.test", PALPO_URL=matrix_origin,
               PALPO_ADMIN_DATABASE=str(root / "admin.sqlite"), PUBLIC_ORIGIN=endpoint,
               PALPO_OPERATIONS_LISTEN=endpoint.removeprefix("http://"), PALPO_TRANSPORT_ORIGIN=endpoint,
               PALPO_RELAY_ORIGIN=endpoint, PALPO_ASSOCIATION_ADMIN="@admin:example.test")
    log = (root / "backend.log").open("w")
    processes = [subprocess.Popen([str(backend)], env=env, stdout=log, stderr=subprocess.STDOUT)]
    apps = []
    report = {"passed": False, "scope": "Actual Rinx, Rust Palpo and native Hagency processes; isolated Matrix HTTP fixture", "checks": []}
    artifacts = {'rinx': args.binary, 'palpo': backend, 'hagency': hagency}
    report['binaries'] = {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in artifacts.items()}
    try:
        for _ in range(100):
            try:
                with HTTP.open(endpoint + "/healthz", timeout=1):
                    break
            except OSError:
                time.sleep(.1)
        state = root / "hagency"
        subprocess.run([str(hagency), "init", "--state-dir", str(state)], check=True, capture_output=True)
        token_file = state / "owner.token"
        token_file.write_text("owner-secret"); token_file.chmod(0o600)
        command = [str(hagency), "association", "--state-dir", str(state), "--palpo-origin", endpoint, "--homeserver", matrix_origin,
                   "--matrix-token-file", str(token_file), "--request-id", "native_association", "--name", "Native Hagency association",
                   "--coordinator", "@coordinator:example.test", "--export-mxid", "@owner:example.test",
                   "--delegation-expires-at-ms", str(int(time.time()*1000)+86400000)]
        requested = json.loads(subprocess.run(command, check=True, capture_output=True, text=True).stdout)
        assert json.loads(subprocess.run(command, check=True, capture_output=True, text=True).stdout) == requested
        admin = launch(root / "admin", args.binary, endpoint, "admin"); apps.append(admin)
        admin.wait_text("Native Hagency association"); admin.click_id("review")
        admin.wait_text("Coordinator · @coordinator:example.test")
        admin.capture("admin-association-review")
        admin.click_id("approve"); admin.wait_text("Decision reason")
        fill(admin, "Decision reason", "Authorize this runtime association")
        admin.click_id("submit"); admin.wait_text("No requests in this view")
        session = post(endpoint, "session", "owner-secret", {"appId": "im.palpo.operations", "bundleDigest": "c"*64,
                      "services": ["palpo.fleets.export", "palpo.fleets.list", "palpo.fleets.connect"]})["sessionToken"]
        profile = call(endpoint, session, "palpo.fleets.export", {"fleetId": requested["fleetId"]})
        saved = state / "approved-profile.json"
        owner = launch(root / "owner", args.binary, endpoint, "owner", visible=args.native_export); apps.append(owner)
        owner.click_id("more"); owner.click_id("fleets"); owner.wait_text("Native Hagency association")
        if args.native_export:
            save_dialog(owner)
            assert not saved.exists()
            owner.capture('owner-export-cancelled')
            save_dialog(owner, saved)
            assert json.loads(saved.read_text()) == profile
            assert saved.stat().st_mode & 0o777 == 0o600
            owner.capture('owner-native-export-saved')
            for secret in [profile['registration']['as_token'], profile['registration']['hs_token'], profile['transport']['token']]:
                assert secret not in json.dumps(owner.snap())
                for path in (root / 'owner').rglob('*'):
                    if path.is_file():
                        assert secret.encode() not in path.read_bytes(), 'Credential escaped into Rinx profile/evidence'
        else:
            saved.write_text(json.dumps(profile)); saved.chmod(0o600)
        subprocess.run([str(hagency), "registration", "--state-dir", str(state), "import", "--file", str(saved), "--homeserver", matrix_origin],
                       check=True, capture_output=True, text=True)
        # Hold the runtime offline so the waiting state can be observed and
        # clicked repeatedly before any authenticated proof can arrive.
        owner.click_id("verify"); owner.wait_text("Connection probe sent")
        owner.wait_text("Verifying…")
        owner.capture("owner-verification-pending")
        for _ in range(3): owner.click_id("verify")
        assert inspect(owner)['connects'] == 1
        assert len([e for e in Matrix.events.values() if e['type'] == 'com.hagency.connection.probe.v1']) == 1
        assert any(w.get('i') == 'verify' and w.get('t') == 'Verifying…' for w in owner.snap())
        detail = launch(root / "owner-detail", args.binary, endpoint, "owner", action=requested['actionId']); apps.append(detail)
        detail.wait_text("Verifying…")
        runtime_log = (root / "hagency.log").open("w")
        processes.append(subprocess.Popen([str(hagency), "serve", "--state-dir", str(state), "--listen", f"127.0.0.1:{port()}", "--palpo-transport"],
                         stdout=runtime_log, stderr=subprocess.STDOUT))
        for _ in range(150):
            fleets = call(endpoint, session, "palpo.fleets.list", {})["fleets"]
            fleet = next(f for f in fleets if f["id"] == requested["fleetId"])
            if fleet["connectionVerified"] and fleet["connectivity"] == "online":
                break
            if processes[-1].poll() is not None:
                raise RuntimeError("Hagency exited; inspect hagency.log")
            time.sleep(.2)
        assert fleet["connectionVerified"] and fleet["connectivity"] == "online", fleet
        owner.wait_text("Connected", timeout=15); owner.wait_text("Runtime · online")
        detail.wait_text("Connected", timeout=15)
        detail.capture("owner-detail-automatically-connected")
        owner.click_id("verify")
        assert inspect(owner)['connects'] == 1
        assert len([e for e in Matrix.events.values() if e['type'] == 'com.hagency.connection.probe.v1']) == 1
        owner.capture("owner-authenticated-probe-verified")
        report["checks"] = ["native Hagency persists owner intent before the request", "same intent retry returns one association",
            "actual Rinx administrator approves association", "explicitly authorized owner retrieves engagement-scoped profile",
            "native Hagency imports its matching pending association", "actual Rinx owner initiates probe",
            "native Hagency consumes relayed Matrix event and work before verification", "Rinx shows proof time and current runtime connectivity",
            "pending verification disables repeated clicks", "engagement list and action detail update to Connected without refresh"]
        if args.native_export:
            report['checks'] += ['native Save cancellation writes no profile', 'native Save produces owner-only credentials importable by Hagency',
                'native export returns no credential bytes to Splash, the app jail or logs']
        if args.credential_controls:
            def wait_verified(ids):
                for _ in range(200):
                    rows = call(endpoint, session, "palpo.fleets.list", {})["fleets"]
                    if all(any(row['id'] == identity and row['connectionVerified'] and row['connectivity'] == 'online' for row in rows) for identity in ids):
                        return rows
                    if processes[-1].poll() is not None:
                        raise RuntimeError('Hagency exited during credential recovery')
                    time.sleep(.2)
                raise AssertionError('Current generation was not verified: ' + json.dumps(rows))

            admin.click_id('fleets'); admin.wait_text('Native Hagency association')
            admin.click_id('pause_fleet'); admin.wait_text('pause Matrix access'); admin.click_id('submit')
            admin.wait_text('paused · @owner:example.test'); admin.capture('admin-credentials-paused')
            assert Matrix.registrations[requested['fleetId']]['disabled'] is True
            admin.click_id('resume_fleet'); admin.wait_text('resume Matrix access'); admin.click_id('submit')
            admin.wait_text('Renew credentials')
            assert not call(endpoint, session, 'palpo.fleets.list', {})['fleets'][0]['connectionVerified']
            admin.click_id('rotate_fleet'); admin.wait_text('Renew connection credentials'); admin.click_id('submit')
            admin.wait_text('Save configuration')
            renewed = call(endpoint, session, 'palpo.fleets.export', {'fleetId': requested['fleetId']})
            assert renewed['transport']['generation'] == profile['transport']['generation'] + 1
            assert renewed['transport']['token'] != profile['transport']['token']
            assert renewed['registration'] == profile['registration']
            running = processes.pop(); running.terminate(); running.wait(timeout=15)
            saved.write_text(json.dumps(renewed))
            subprocess.run([str(hagency), 'registration', '--state-dir', str(state), 'import', '--file', str(saved), '--homeserver', matrix_origin], check=True, capture_output=True, text=True)
            stale = state / 'previous-profile.json'; stale.write_text(json.dumps(profile)); stale.chmod(0o600)
            rejected = subprocess.run([str(hagency), 'registration', '--state-dir', str(state), 'import', '--file', str(stale), '--homeserver', matrix_origin], capture_output=True, text=True)
            assert rejected.returncode != 0, 'Old transport profile overwrote the rotated generation'
            processes.append(subprocess.Popen([str(hagency), 'serve', '--state-dir', str(state), '--listen', f'127.0.0.1:{port()}', '--palpo-transport'], stdout=runtime_log, stderr=subprocess.STDOUT))
            owner.click_id('refresh'); owner.wait_text('Verify connection'); owner.click_id('verify'); owner.wait_text('Connection probe sent')
            wait_verified([requested['fleetId']])
            owner.click_id('refresh'); owner.wait_text('Connection verified'); owner.capture('owner-rotated-profile-verified')
            report['checks'] += ['native administrator pauses and resumes Matrix credentials', 'resume requires a fresh connection proof',
                'native administrator rotates only this transport credential', 'native Hagency reimports the new generation and verifies after restart',
                'an old imported profile cannot restore previous credentials']

            second_command = command.copy()
            second_command[second_command.index('--request-id') + 1] = 'native_association_second'
            second_command[second_command.index('--name') + 1] = 'Independent second engagement'
            second = json.loads(subprocess.run(second_command, check=True, capture_output=True, text=True).stdout)
            admin.click_id('inbox'); admin.wait_text('Independent second engagement'); admin.click_id('review')
            admin.wait_text('Coordinator · @coordinator:example.test'); admin.click_id('approve'); admin.wait_text('Decision reason')
            fill(admin, 'Decision reason', 'Authorize the independent second engagement')
            admin.click_id('submit'); admin.wait_text('No requests in this view')
            second_profile = call(endpoint, session, 'palpo.fleets.export', {'fleetId': second['fleetId']})
            assert second_profile['transport']['token'] != renewed['transport']['token']
            second_file = state / 'second-profile.json'; second_file.write_text(json.dumps(second_profile)); second_file.chmod(0o600)
            running = processes.pop(); running.terminate(); running.wait(timeout=15)
            subprocess.run([str(hagency), 'registration', '--state-dir', str(state), 'import', '--file', str(second_file), '--homeserver', matrix_origin], check=True, capture_output=True, text=True)
            processes.append(subprocess.Popen([str(hagency), 'serve', '--state-dir', str(state), '--listen', f'127.0.0.1:{port()}', '--palpo-transport'], stdout=runtime_log, stderr=subprocess.STDOUT))
            call(endpoint, session, 'palpo.fleets.connect', {'fleetId': second['fleetId']})
            wait_verified([requested['fleetId'], second['fleetId']])
            owner.click_id('refresh'); owner.wait_text('Independent second engagement'); owner.capture('owner-two-same-server-engagements')
            same_first = call(endpoint, session, 'palpo.fleets.export', {'fleetId': requested['fleetId']})
            assert same_first['transport'] == renewed['transport']
            report['checks'] += ['one native runtime imports two independent profiles for the same Matrix server',
                'both profiles retain authenticated proof and current connectivity after restart', 'importing the second profile preserves the first transport identity']
        assert report['binaries'] == {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in artifacts.items()}, 'A tested executable changed during acceptance'
        report["passed"] = True
    except Exception as error:
        report['error'] = str(error)
        for index, app in enumerate(apps):
            try:
                app.capture('failed-acceptance')
                (root / f'failed-app-{index}.json').write_text(json.dumps(app.snap(), indent=2))
            except Exception:
                pass
        raise
    finally:
        for app in apps:
            app.stop()
        for process in reversed(processes):
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill(); process.wait(timeout=5)
        matrix.shutdown(); matrix.server_close(); log.close()
        (root / "report.json").write_text(json.dumps(report, indent=2))
        print(root / "report.json")


if __name__ == "__main__":
    main()
