#!/usr/bin/env python3
"""Production Rinx Splash/host + Rust Palpo process; explicit loopback Matrix fixture.

No live accounts, JavaScript backend, deployment state or existing app instances.
This proves the mini-app decision boundary, not native Hagency provisioning/chat.
"""
import argparse
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import plistlib
from pathlib import Path
import sqlite3
import subprocess
import threading
import time
from urllib.request import Request, build_opener, ProxyHandler
from urllib.parse import unquote, urlsplit
import uuid

from native_palpo import PalpoApp, port, fill, inspect

SERVICES = ["palpo.session.open", "palpo.inbox.submit", "palpo.inbox.get", "palpo.inbox.list"]
HTTP = build_opener(ProxyHandler({}))


class Matrix(BaseHTTPRequestHandler):
    rooms = {}
    aliases = {}
    events = {}
    guard = threading.Lock()

    def log_message(self, *_):
        pass

    def user(self):
        return {f"Bearer {name}-secret": f"@{name}:example.test" for name in ("owner", "coordinator", "admin", "notices")}.get(self.headers.get("Authorization"))

    def reply(self, value, status=200):
        raw = json.dumps(value).encode()
        self.send_response(status); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw))); self.end_headers(); self.wfile.write(raw)

    def do_POST(self):
        value = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        actor = self.user(); parts = [unquote(p) for p in urlsplit(self.path).path.split('/')]
        with self.guard:
            if parts[-1] == 'createRoom' and actor == '@notices:example.test':
                room = f"!actions{len(self.rooms)}:example.test"
                self.aliases['#' + value['room_alias_name'] + ':example.test'] = room
                self.rooms[room] = value['initial_state'] + [
                    {'type':'m.room.create','state_key':'','sender':actor,'content':{'creator':actor,**value['creation_content']}},
                    {'type':'m.room.power_levels','state_key':'','content':value['power_level_content_override']},
                    {'type':'m.room.member','state_key':actor,'content':{'membership':'join'}},
                    *[{'type':'m.room.member','state_key':u,'content':{'membership':'invite'}} for u in value['invite']]]
                self.reply({'room_id':room}); return
            if len(parts) >= 2 and parts[-2] == 'join' and actor:
                for member in self.rooms.get(parts[-1], []):
                    if member.get('type') == 'm.room.member' and member.get('state_key') == actor:
                        member['content']['membership'] = 'join'; self.reply({'room_id':parts[-1]}); return
        self.reply({'errcode':'M_FORBIDDEN'},403)

    def do_PUT(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        if self.user() != '@notices:example.test': self.reply({'errcode':'M_FORBIDDEN'},403); return
        with self.guard:
            if self.path in self.events and self.events[self.path] != body:
                self.reply({'errcode':'changed_transaction'},409); return
            self.events[self.path] = body
            self.reply({'event_id': '$notice' + digest(self.path)[:24]})

    def do_GET(self):
        user = self.user()
        parts = [unquote(p) for p in urlsplit(self.path).path.split('/')]
        if user == '@notices:example.test':
            with self.guard:
                if 'directory' in parts:
                    room = self.aliases.get(parts[-1]); self.reply({'room_id':room} if room else {'errcode':'M_NOT_FOUND'},200 if room else 404); return
                if parts[-1] == 'joined_rooms': self.reply({'joined_rooms':list(self.rooms)}); return
                if len(parts) > 2 and parts[-1] == 'state' and parts[-2] in self.rooms: self.reply(self.rooms[parts[-2]]); return
        status = 200 if user else 401
        if self.path == "/_matrix/client/v3/account/whoami" and user:
            value = {"user_id": user}
        elif self.path == "/_palpo/admin/v1/appservices" and user == "@admin:example.test":
            value = {"appservices": []}
        else:
            status = 403 if user else 401
            value = {"errcode": "M_FORBIDDEN"}
        raw = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)


def post(endpoint, operation, token, body):
    req = Request(endpoint + "/_palpo/miniapp/v1/" + operation,
                  data=json.dumps(body).encode(), headers={"Content-Type": "application/json", "Authorization": "Bearer " + token})
    with HTTP.open(req, timeout=10) as response:
        return json.load(response)


def call(endpoint, token, service, args):
    return post(endpoint, "call", token, {"service": service, "args": args})


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def machine_update(endpoint, fleet, body):
    req = Request(endpoint + f"/api/fleet/v2/{fleet}/updates", data=json.dumps(body).encode(),
                  headers={"Content-Type": "application/json", "Authorization": "Bearer fixture-machine",
                           "X-Hagency-Generation": "1"})
    with HTTP.open(req, timeout=10) as response:
        return json.load(response)


def launch(root, binary, endpoint, role, board=False, visible=False, action=None, narrow=None, ux_review=False, bundle_source=None):
    profile = root / "profile"
    (profile / "app").mkdir(parents=True)
    app = PalpoApp(root, port=port(), auto_login=False)
    app.output.mkdir(parents=True)
    app.log = (app.output / "native.log").open("w")
    env = dict(os.environ, RINX_DATA_DIR=str(profile.resolve()), MAKEPAD_HIDE_WINDOWS="1",
               MAKEPAD_NO_FOCUS="1", MAKEPAD_REMOTE=str(app.port), PALPO_FIXTURE_URL=endpoint)
    env.pop("PALPO_LIVE_SESSION_FILE", None)
    env.pop("MAKEPAD_FOCUS", None)
    env.pop("PALPO_FIXTURE_ACTION", None)
    env.pop("PALPO_FIXTURE_BUNDLE", None)
    if bundle_source:
        env['PALPO_FIXTURE_BUNDLE'] = str(bundle_source.resolve())
    if action: env['PALPO_FIXTURE_ACTION'] = action
    if visible:
        env.pop("MAKEPAD_HIDE_WINDOWS", None)
        env.pop("MAKEPAD_NO_FOCUS", None)
        env['MAKEPAD_FOCUS'] = '1'
        # AppKit's document panel service needs a real application identity.
        # Keep the tested executable unchanged inside an isolated native bundle.
        contents = root / "Palpo Acceptance.app" / "Contents"
        executable = contents / "MacOS" / "palpo_miniapp"
        executable.parent.mkdir(parents=True)
        os.link(binary.resolve(), executable)
        (contents / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleIdentifier": "im.rinx.palpo-acceptance", "CFBundleName": "Palpo Acceptance",
            "CFBundleExecutable": "palpo_miniapp", "CFBundlePackageType": "APPL",
        }))
        binary = executable
    args = [str(binary.resolve())] + ([] if role == "owner" else ["--" + role])
    if (narrow if narrow is not None else role == "owner"):
        args.append("--narrow")
    if ux_review:
        args.append("--ux-review")
    app.process = subprocess.Popen(args, env=env, stdout=app.log, stderr=subprocess.STDOUT)
    try:
        for _ in range(150):
            if app.process.poll() is not None:
                raise RuntimeError("Native fixture exited; inspect native.log")
            try:
                if app.request("/s")["w"]:
                    break
            except OSError:
                pass
            time.sleep(.1)
        app.wait_text("Requested by " if action else "Pending actions stay here", timeout=45)
        # The Inbox shell draws before its initial HTTP result. Input while the
        # bundle is busy is intentionally ignored; wait for that first result.
        for _ in range(150):
            if (board or inspect(app)["pending"] == 0) and not any(w.get("i") == "status" and w.get("t") == "Working…" for w in app.snap()):
                break
            time.sleep(.1)
        else:
            raise AssertionError("Initial Inbox request did not settle")
        return app
    except Exception:
        app.stop()
        raise


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=Path("target/fast/examples/palpo_miniapp"))
    parser.add_argument("--board-binary", type=Path)
    args = parser.parse_args()
    root = Path("target/palpo-coordinator-validation") / uuid.uuid4().hex
    root.mkdir(parents=True)
    matrix = ThreadingHTTPServer(("127.0.0.1", 0), Matrix)
    threading.Thread(target=matrix.serve_forever, daemon=True).start()
    endpoint = f"http://127.0.0.1:{port()}"
    env = {k: v for k, v in os.environ.items() if not k.startswith("PALPO_")}
    env.update(PALPO_SERVER_NAME="example.test", PALPO_URL=f"http://127.0.0.1:{matrix.server_port}",
               PALPO_ADMIN_DATABASE=str((root / "admin.sqlite").resolve()), PUBLIC_ORIGIN=endpoint,
               PALPO_OPERATIONS_LISTEN=endpoint.removeprefix("http://"), PALPO_TRANSPORT_ORIGIN=endpoint,
               PALPO_RELAY_ORIGIN=endpoint)
    notice_token = root / "notice.token"
    notice_token.write_text("notices-secret"); notice_token.chmod(0o600)
    env.update(PALPO_ACTIONS_BOT_MXID="@notices:example.test", PALPO_ACTIONS_BOT_TOKEN_FILE=str(notice_token.resolve()), PALPO_ACTIONS_PUBLIC_ORIGIN=endpoint)
    fleet = "hf_" + "a" * 32
    now = int(time.time() * 1000)
    snapshot = {"engagements": {fleet: {"id": fleet, "server": "example.test", "owner": "@provider:example.test",
        "coordinator": "@coordinator:example.test", "registrationGeneration": 1, "delegationRevision": 1,
        "delegationExpiresAtMs": now + 3600000, "state": "verified", "allowSelfApproval": False, "coordinatorApprovalV1": True}},
        "resources": {"grant_a": {"id": "grant_a", "serverEngagementId": fleet, "revision": 1, "allocatedTokens": 1000000,
            "eligibleManagers": ["@owner:example.test"]}},
        "projects": {"project_one": {"projectId": "project_one", "serverEngagementId": fleet, "revision": 1,
            "owner": "@owner:example.test", "resourceAllocations": ["grant_a"], "state": "ready"}}}
    authority = root / "authority.json"
    authority.write_text(json.dumps(snapshot))
    subprocess.run([str(args.backend.resolve()), "import-authority", str(authority.resolve())], env=env, check=True,
                   stdout=subprocess.DEVNULL)
    # Seed an installed transport fixture while no process owns this fixture DB.
    with sqlite3.connect(root / "admin.sqlite") as db:
        state = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
        state["fleets"][fleet] = {"id": fleet, "state": "ready", "installation": "installed",
            "representativeMxid": f"@{fleet}_representative:example.test", "registrationGeneration": 1,
            "transport": {"mode": "outbound", "generation": 1, "sequence": 0, "token": "fixture-machine"},
            "capabilities": {"coordinatorApprovalV1": True, "coordinatorAgentControlV1": True, "coordinatorAgentProfileV1": True, "coordinatorProjectSetupV1": True}}
        db.execute("UPDATE state SET body=? WHERE id=1", [json.dumps(state)])
    log = (root / "backend.log").open("w")
    server = subprocess.Popen([str(args.backend.resolve())], env=env, stdout=log, stderr=subprocess.STDOUT)
    apps = []
    report = {"passed": False, "evidence": str(root.resolve()), "checks": [], "scope": "Rinx decision UI and Rust Palpo; Matrix fixture; no live Hagency",
              "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              "backend_sha256": hashlib.sha256(args.backend.read_bytes()).hexdigest()}
    try:
        for _ in range(100):
            if server.poll() is not None:
                raise RuntimeError("Rust service exited; inspect backend.log")
            try:
                with HTTP.open(endpoint + "/healthz", timeout=1):
                    break
            except OSError:
                time.sleep(.1)
        opened = post(endpoint, "session", "owner-secret", {"appId": "im.palpo.operations", "bundleDigest": "c" * 64, "services": SERVICES})
        token = opened["sessionToken"]
        definition = {"v": 1, "fleetId": fleet, "requestId": "a" * 40, "targetProjectId": "project_one",
            "targetRoomId": "!project:example.test", "sourceRoomId": "!reception:example.test", "sourceEventId": "$fixture-request",
            "ownerMxid": "@owner:example.test", "requesterMxid": "@owner:example.test", "ownerDmRoomId": "!private:example.test",
            "role": "developer", "requestedTokens": 100000, "agentDefinition": {"name": "Littlewhite", "instructions": "Help with the project"}}
        request = {"kind": "agent", "request": {"id": "a" * 40, "revision": 1, "serverEngagementId": fleet,
            "projectId": "project_one", "projectRevision": 1, "resourceAllocationId": "grant_a", "projectOwner": "@owner:example.test",
            "requester": "@owner:example.test", "definitionDigest": digest(definition), "requestedTokens": 100000}, "definition": definition}
        action = call(endpoint, token, "palpo.inbox.submit", request)["action"]
        if args.board_binary:
            report["board_binary_sha256"] = hashlib.sha256(args.board_binary.read_bytes()).hexdigest()
            board = launch(root / "board", args.board_binary, endpoint, "coordinator", board=True); apps.append(board)
            board.wait_text("Littlewhite"); board.capture("action-board-light")
            # Unrelated Matrix/background signals must not repaint the app.
            time.sleep(1)
            before = inspect(board)
            for _ in range(20):
                board.request('/event', data='palpo:signal', wait=0)
                time.sleep(.05)
            after = inspect(board)
            assert after['signals'] - before['signals'] >= 20
            assert after['draws'] - before['draws'] <= 2, (before, after)
            report['signal_redraws'] = {'signals': after['signals'] - before['signals'], 'draws': after['draws'] - before['draws']}
            assert not any(w.get("i") in {"projects", "resources", "actions_room"} for w in board.snap())
            board.click_id("review"); board.wait_text("Approve"); board.capture("action-board-latest-action")
            board.click_id("toggle"); board.wait_text("Ordinary chat timeline fallback")
            board.click_id("toggle"); board.wait_text("Littlewhite")
            board.click_id("dark"); board.wait_text("Littlewhite"); board.capture("action-board-dark")
            board.click_id("switch_account"); board.wait_text("Ordinary chat timeline fallback")
            assert not any(w.get("i") in {"review", "approve", "toggle"} for w in board.snap())
            board.capture("action-board-account-switch")
        owner = launch(root / "owner", args.binary, endpoint, "owner"); apps.append(owner)
        owner.click_id("waiting"); owner.wait_text("Littlewhite")
        owner.click_id("review"); owner.wait_text("Requested by @owner:example.test")
        assert not any(w.get("t") == "Approve" for w in owner.snap())
        owner.capture("owner-waiting")
        admin = launch(root / "admin", args.binary, endpoint, "admin"); apps.append(admin)
        admin.wait_text("No requests in this view")
        assert not any(w.get("t") == "Contribute resources" for w in admin.snap())
        admin.capture("admin-no-agent-authority")
        coordinator = launch(root / "coordinator", args.binary, endpoint, "coordinator"); apps.append(coordinator)
        coordinator.wait_text("Littlewhite"); coordinator.click_id("review"); coordinator.wait_text("Approve")
        coordinator.click_id("approve"); coordinator.wait_text("Decision reason")
        fill(coordinator, "Decision reason", "Approved within the engagement quota")
        before = inspect(coordinator)
        for theme in ("dark", "violet", "light"):
            coordinator.request("/event", data="palpo:" + theme, wait=1)
            coordinator.wait_text("Approved within the engagement quota")
            after = inspect(coordinator)
            assert (before["heap"], before["calls"]) == (after["heap"], after["calls"])
            coordinator.capture("coordinator-decision-" + theme)
        draft = json.loads((coordinator.root / "profile/app/draft.json").read_text())
        coordinator.click_id("submit"); coordinator.wait_text("No requests in this view")
        latest = call(endpoint, token, "palpo.inbox.get", {"id": action["id"]})["action"]
        assert latest["state"] == "approved" and latest["execution"] == "pending", latest
        coordinator_session = post(endpoint, "session", "coordinator-secret", {"appId": "im.palpo.operations", "bundleDigest": "d" * 64, "services": ["palpo.inbox.decide"]})
        replay = call(endpoint, coordinator_session["sessionToken"], "palpo.inbox.decide", draft["payload"])["action"]
        assert replay["id"] == action["id"] and replay["state"] == "approved"
        owner.click_id("latest"); owner.wait_text("Approved"); owner.wait_text("Waiting for Hagency")
        owner.capture("owner-approved-awaiting-hagency")
        owner.click_id("requests"); owner.wait_text("Littlewhite"); owner.wait_text("Waiting for Hagency")
        owner.wait_text("Allocation confirmation pending"); owner.wait_text("Token consumption not reported yet")
        owner.capture("owner-agents-pending")
        detail = launch(root / "owner-detail", args.binary, endpoint, "owner", action=action["id"]); apps.append(detail)
        detail.wait_text("Waiting for Hagency")
        owner.click_id("projects"); owner.wait_text("project_one")
        assert not any(w.get("t") == "Request agent" for w in owner.snap())
        owner.click_id("requests"); owner.wait_text("Littlewhite")
        admin.click_id("requests"); admin.wait_text("Your agent requests will appear here")
        assert not any(w.get("t") == "Littlewhite" for w in admin.snap())
        admin.capture("admin-no-agent-list-access")
        with sqlite3.connect(root / "admin.sqlite") as db:
            state = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
            assert len(state["rustWorkflows"]["outbox"]) == 1
            command = next(iter(state["rustWorkflows"]["outbox"].values()))["command"]
            assert command["context"]["actor"] == "@coordinator:example.test"
            assert db.execute("SELECT COUNT(*) FROM fleet_delivery WHERE lane='work'").fetchone()[0] == 1
        # Explicit native-provider fixtures exercise real authenticated update
        # ingestion and UI metering; they do not execute a Hagency agent.
        receipt = {"kind": "receipt", "registrationGeneration": 1, "delegationRevision": 1,
                   "commandId": command["context"]["commandId"], "state": "applied", "agentId": "en_littlewhite",
                   "commandDigest": digest({"operation": "coordinator_agent_approval", "command": command})}
        observed = dict(definition, engagementId="en_littlewhite", state="active",
                        agentMxid=f"@{fleet}_en_littlewhite:example.test", allocatedTokens=100000,
                        consumedTokens=42, usageObservedAtMs=int(time.time() * 1000), usageEvidence="host_attributed_lower_bound",
                        usageComplete=False, quotaPaused=False, bound=True, ready=True,
                        fulfillment={"phase": "complete", "incomplete": False}, observedAt=datetime.now(timezone.utc).isoformat())
        provisioning = dict(observed, ready=False, lifecycle={"provisionEffect": "pending"})
        update = {"v": 2, "generation": 1, "sequence": 1, "heartbeat": True, "statuses": [provisioning],
                  "coordinatorUpdates": [{"id": "command_" + receipt["commandId"], "payload": receipt, "digest": digest(receipt)}]}
        machine_update(endpoint, fleet, update)
        for app in (owner, detail):
            app.wait_text("Preparing agent")
        unavailable = dict(observed, ready=False, lifecycle={"provisionEffect": "complete", "matrixReady": False, "runtimeAvailability": "available"})
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": 2, "heartbeat": True, "statuses": [unavailable]})
        for app in (owner, detail):
            app.wait_text("Needs attention")
            app.capture("agent-unavailable-auto")
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": 3, "heartbeat": True, "statuses": [observed]})
        for app in (owner, detail):
            app.wait_text("Ready to chat")
            assert not any(w.get("t") == "Agent setup needs attention" for w in app.snap())
            app.capture("agent-ready-auto")
        owner.wait_text("Consumed: at least 42 tokens")
        owner.capture("owner-agents-current-usage")
        owner.click_id("more_tokens"); owner.wait_text("Additional tokens")
        fill(owner, "Additional tokens", "20000")
        top_up_draft = json.loads((owner.root / "profile/app/draft.json").read_text())
        owner.capture("owner-request-more-tokens")
        owner.click_id("submit"); owner.wait_text("More tokens · Littlewhite")
        coordinator.click_id("refresh"); coordinator.wait_text("More tokens · Littlewhite")
        coordinator.click_id("review"); coordinator.wait_text("Request 20000 additional tokens")
        coordinator.click_id("approve"); coordinator.wait_text("Decision reason")
        fill(coordinator, "Decision reason", "Additional capacity approved")
        coordinator.capture("coordinator-top-up-approval")
        coordinator.click_id("submit"); coordinator.wait_text("No requests in this view")
        with sqlite3.connect(root / "admin.sqlite") as db:
            state = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
            assert db.execute("SELECT COUNT(*) FROM fleet_delivery WHERE lane='work'").fetchone()[0] == 2
            top_up = next(r for r in state["rustWorkflows"]["outbox"].values() if r["id"] != command["context"]["commandId"])
            top_up_command = top_up["command"]
            assert top_up_command["request"]["agentAllocationId"] == "en_littlewhite"
            assert top_up_command["request"]["expectedAllocatedTokens"] == 100000
            assert top_up_command["additionalTokens"] == 20000
            assert top_up_command["context"]["actor"] == "@coordinator:example.test"
        top_up_receipt = dict(receipt, commandId=top_up_command["context"]["commandId"],
                              commandDigest=digest({"operation": "coordinator_token_top_up", "command": top_up_command}))
        observed["allocatedTokens"] = 120000
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": 4, "heartbeat": True, "statuses": [observed],
            "coordinatorUpdates": [{"id": "command_" + top_up_receipt["commandId"], "payload": top_up_receipt, "digest": digest(top_up_receipt)}]})
        retried = call(endpoint, token, "palpo.inbox.submit", top_up_draft["payload"])["action"]
        assert retried["id"] == top_up["actionId"] and retried["execution"] == "done"
        owner.click_id("requests"); owner.wait_text("Allocated: 120000 tokens")
        owner.capture("owner-top-up-applied")
        observed["observedAt"] = "2020-01-01T00:00:00Z"
        observed["usageObservedAtMs"] = 1577836800000
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": 5, "heartbeat": True, "statuses": [observed]})
        owner.click_id("refresh"); owner.wait_text("Usage sample is out of date")
        assert not any(w.get("t") == "Ready to chat" for w in owner.snap())
        assert not any(w.get("t") == "Request more tokens" for w in owner.snap())
        owner.capture("owner-agents-stale-usage")
        refused_request = json.loads(json.dumps(request))
        refused_request["request"]["id"] = "b" * 40
        refused_request["definition"]["requestId"] = "b" * 40
        refused_request["definition"]["agentDefinition"]["name"] = "CapacityRefused"
        refused_request["request"]["definitionDigest"] = digest(refused_request["definition"])
        refused_action = call(endpoint, token, "palpo.inbox.submit", refused_request)["action"]
        coordinator.click_id("refresh"); coordinator.wait_text("CapacityRefused")
        coordinator.click_id("review"); coordinator.wait_text("Approve")
        coordinator.click_id("approve"); coordinator.wait_text("Decision reason")
        fill(coordinator, "Decision reason", "Review the remaining capacity")
        coordinator.click_id("submit"); coordinator.wait_text("No requests in this view")
        with sqlite3.connect(root / "admin.sqlite") as db:
            state = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
            refused_command = next(r["command"] for r in state["rustWorkflows"]["outbox"].values() if r["actionId"] == refused_action["id"])
        refusal = {"kind": "receipt", "registrationGeneration": 1, "delegationRevision": 1,
            "commandId": refused_command["context"]["commandId"], "state": "refused", "reason": "insufficient_capacity",
            "commandDigest": digest({"operation": "coordinator_agent_approval", "command": refused_command})}
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": 6, "heartbeat": True,
            "coordinatorUpdates": [{"id": "command_" + refusal["commandId"], "payload": refusal, "digest": digest(refusal)}]})
        owner.click_id("refresh")
        for _ in range(5):
            if any(w.get("t") == "CapacityRefused" for w in owner.snap()):
                break
            owner.request('/m', k='scroll', x=210, y=500, dy=220, wait=1)
        owner.wait_text("CapacityRefused")
        for _ in range(8):
            if any(w.get("t") == "Allocation refused" for w in owner.snap()):
                break
            owner.request('/m', k='scroll', x=210, y=500, dy=100, wait=1)
        owner.wait_text("Allocation refused")
        for _ in range(5):
            if any(w.get("t") == "Not enough capacity remains in this allocation." for w in owner.snap()):
                break
            owner.request('/m', k='scroll', x=210, y=500, dy=100, wait=1)
        owner.wait_text("Not enough capacity remains in this allocation.")
        owner.capture("owner-agent-allocation-refused")
        # Native forms and real Palpo receipts; this provider fixture does not
        # claim to execute runtime cleanup (covered by native worker tests).
        sequence = 7
        observed.update(observedAt=datetime.now(timezone.utc).isoformat(), usageObservedAtMs=int(time.time() * 1000),
            lifecycle={"runtimeState": "active", "paused": False, "cleanup": "not_required", "cleanupEffect": None})
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": sequence, "heartbeat": True, "statuses": [observed]})
        for operation, widget_id, title in [("rename", "rename_agent", "Rename agent"), ("stop", "pause_agent", "Pause agent"), ("start", "resume_agent", "Resume agent"), ("retire", "remove_agent", "Remove agent")]:
            owner.click_id("refresh")
            owner.request('/m', k='scroll', x=210, y=420, dy=-2600, wait=1)
            if any(w.get("i") == "manage_agent" and w.get("t") == "Manage agent" for w in owner.snap()):
                owner.click_id("manage_agent")
            for _ in range(18):
                if any(w.get("i") == widget_id for w in owner.snap()):
                    break
                owner.request('/m', k='scroll', x=210, y=420, dy=140, wait=1)
            owner.click_id(widget_id); owner.wait_text(title)
            if operation == "rename":
                fill(owner, "Display name", "Friendly little white")
            owner.capture("owner-control-" + operation)
            control_draft = json.loads((owner.root / "profile/app/draft.json").read_text())
            owner.click_id("submit")
            with sqlite3.connect(root / "admin.sqlite") as db:
                control = None
                for _ in range(100):
                    stored = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
                    control = stored["rustWorkflows"].get("agentControls", {}).get(control_draft["payload"]["commandId"])
                    if control:
                        break
                    time.sleep(.03)
            assert control and control["state"] == "pending", control
            control_receipt = {"kind": "receipt", "registrationGeneration": 1, "delegationRevision": 1,
                "commandId": control_draft["payload"]["commandId"], "commandDigest": control["commandDigest"],
                "agentId": "en_littlewhite", "operation": operation, "state": "applied"}
            observed["lifecycle"]["paused"] = operation == "stop"
            sequence += 1
            machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": sequence, "heartbeat": True, "statuses": [observed],
                "coordinatorUpdates": [{"id": "command_" + control_receipt["commandId"], "payload": control_receipt, "digest": digest(control_receipt)}]})
            if operation == "rename":
                owner.click_id("refresh")
                owner.request('/m', k='scroll', x=210, y=420, dy=-2600, wait=1)
                for _ in range(18):
                    if any(w.get("t") == "Name saved · waiting for Matrix verification" for w in owner.snap()):
                        break
                    owner.request('/m', k='scroll', x=210, y=420, dy=140, wait=1)
                owner.wait_text("Name saved · waiting for Matrix verification")
                owner.capture("owner-rename-pending")
                assert control["command"]["displayName"] == "Friendly little white"
                observed["lifecycle"]["matrixProfile"] = {"desiredName": "Friendly little white", "observedName": "Friendly little white", "state": "verified"}
                sequence += 1
                machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": sequence, "heartbeat": True, "statuses": [observed]})
                owner.click_id("refresh")
                owner.request('/m', k='scroll', x=210, y=420, dy=-2600, wait=1)
                owner.wait_text("Friendly little white")
                owner.capture("owner-rename-verified")
        observed.update(state="ended", ready=False, bound=False)
        observed["lifecycle"].update(runtimeState="revoked", cleanup="complete", cleanupEffect="complete", settlement={"state": "awaiting_final_usage"})
        sequence += 1
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": sequence, "heartbeat": True, "statuses": [observed]})
        owner.click_id("refresh")
        owner.request('/m', k='scroll', x=210, y=420, dy=-2600, wait=1)
        for _ in range(18):
            if any("Agent removed · cleanup verified" in w.get("t", "") for w in owner.snap()):
                break
            owner.request('/m', k='scroll', x=210, y=420, dy=100, wait=1)
        owner.wait_text("Agent removed · cleanup verified. Usage settlement is tracked separately.")
        owner.capture("owner-agent-retired")
        project_definition = {"name": "Recovery project", "roomId": "!recovery:example.test", "ownerDmRoomId": "!recoveryprivate:example.test"}
        project_request = {"kind": "project", "request": {"id": "recover_request", "revision": 1, "serverEngagementId": fleet,
            "projectId": "recover_project", "owner": "@owner:example.test", "requester": "@owner:example.test", "definitionDigest": digest(project_definition), "resourceAllocations": ["grant_a"]}, "definition": project_definition}
        project_action = call(endpoint, token, "palpo.inbox.submit", project_request)["action"]
        coordinator.click_id("inbox"); coordinator.click_id("needs"); coordinator.wait_text("Recovery project")
        coordinator.click_id("review"); coordinator.click_id("approve"); coordinator.wait_text("Decision reason")
        coordinator.click_id("submit"); coordinator.wait_text("No requests in this view")
        with sqlite3.connect(root / "admin.sqlite") as db:
            state = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
            project_command = next(r["command"] for r in state["rustWorkflows"]["outbox"].values() if r["actionId"] == project_action["id"])
        source = project_command["context"]["commandId"]
        project_grant = {"projectId": "recover_project", "serverEngagementId": fleet, "revision": 1, "owner": "@owner:example.test", "resourceAllocations": ["grant_a"], "state": "approved"}
        projection = {"kind": "project", "registrationGeneration": 1, "delegationRevision": 1, "project": project_grant}
        receipt = {"kind": "receipt", "registrationGeneration": 1, "delegationRevision": 1, "commandId": source, "commandDigest": digest({"operation": "coordinator_project_approval", "command": project_command, "definition": project_definition}), "projectId": "recover_project", "state": "applied"}
        setup = {"kind": "project_setup", "registrationGeneration": 1, "delegationRevision": 1, "projectId": "recover_project", "projectRevision": 1,
            "approvalCommandId": source, "attemptId": source, "revision": 1, "state": "failed", "reason": "private_membership_pending", "observedAtMs": int(time.time() * 1000)}
        def projection_row(identity, body):
            return {"id": identity, "payload": body, "digest": digest(body)}
        sequence += 1
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": sequence, "heartbeat": True,
            "coordinatorUpdates": [projection_row("project_recover_project", projection), projection_row("command_" + source, receipt), projection_row("project_setup_recover_project", setup)]})
        owner.click_id("projects"); owner.wait_text("Recovery project")
        owner.wait_text("The approval bot could not join your private room. Check its invitation, then retry.")
        owner.capture("owner-project-setup-failed")
        owner.click_id("retry_setup"); owner.wait_text("Retry project setup"); owner.capture("owner-project-setup-retry")
        retry_draft = json.loads((owner.root / "profile/app/draft.json").read_text())
        owner.click_id("submit"); owner.wait_text("Project setup is waiting for Hagency and Matrix verification.")
        setup.update(attemptId=retry_draft["payload"]["commandId"], revision=2, state="ready", reason=None, observedAtMs=int(time.time() * 1000))
        project_grant["state"] = "ready"
        sequence += 1
        machine_update(endpoint, fleet, {"v": 2, "generation": 1, "sequence": sequence, "heartbeat": True,
            "coordinatorUpdates": [projection_row("project_recover_project", projection), projection_row("project_setup_recover_project", setup)]})
        owner.click_id("refresh"); owner.wait_text("Project rooms verified"); owner.capture("owner-project-setup-ready")
        with sqlite3.connect(root / "admin.sqlite") as db:
            stored = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
            assert stored["rustWorkflows"]["actions"][project_action["id"]]["decision"]["commandId"] == source
            assert stored["rustWorkflows"]["projectRetries"][retry_draft["payload"]["commandId"]]["command"]["approvalCommandId"] == source
        coordinator.click_id("more"); coordinator.click_id("notifications"); coordinator.wait_text("Action notifications: On")
        coordinator.click_id("notifications_enabled"); coordinator.wait_text("Action notifications: Off")
        coordinator.click_id("reminders_enabled"); coordinator.wait_text("Reminders: Off")
        for _ in range(16):
            if any(w.get("i") == "save_notifications" for w in coordinator.snap()):
                break
            coordinator.request('/m', k='scroll', x=400, y=420, dy=100, wait=1)
        coordinator.click_id("save_notifications")
        coordinator.wait_text("Notification settings saved for your account")
        coordinator.request('/m', k='scroll', x=400, y=420, dy=-2000, wait=1)
        coordinator.wait_text("Action notifications: Off"); coordinator.capture("coordinator-notifications-disabled")
        coordinator.click_id("inbox"); coordinator.wait_text("Pending actions stay here")
        coordinator.click_id("more"); coordinator.click_id("notifications"); coordinator.wait_text("Action notifications: Off")
        owner.click_id("more"); owner.click_id("notifications"); owner.wait_text("Action notifications: On")
        with sqlite3.connect(root / "admin.sqlite") as db:
            stored = json.loads(db.execute("SELECT body FROM state WHERE id=1").fetchone()[0])
            preferences = stored["actionInbox"]["preferences"]["@coordinator:example.test"]
            assert preferences["revision"] == 1 and preferences["value"]["enabled"] is False
            assert preferences["value"]["remindersEnabled"] is False
        report["checks"] = ["manager cannot approve own agent", "Matrix admin has no implicit agent approval",
            "coordinator approves from the actual OctoScript form", "theme changes preserve draft and request count",
            "same command retry queues exactly one Hagency delivery", "owner sees approved and pending execution separately",
            "resource contribution is absent from the mini app", "role-scoped Projects and Agents navigation",
            "agent list distinguishes pending allocation and unknown consumption",
            "agent list and action detail automatically follow pending, provisioning, unavailable and ready without Refresh",
            "authenticated provider fixture shows current lower-bound usage", "old provider observations do not claim live readiness",
            "owner requests additional tokens through the native form", "coordinator approves the allocation-bound top-up",
            "top-up retry after provider execution returns the original result",
            "owner sees a terminal provider refusal without losing the approved decision",
            "native project setup failure and recovery retain the original approval", "scoped rename waits for Matrix observation", "owner pauses and resumes through actual native forms", "removal receipt is distinct from runtime cleanup",
            "retired history remains visible with usage settlement separate",
            "native notification preferences persist after reopening", "notification settings stay isolated per Matrix account"]
        assert hashlib.sha256(args.binary.read_bytes()).hexdigest() == report["binary_sha256"], "native binary changed during acceptance"
        assert hashlib.sha256(args.backend.read_bytes()).hexdigest() == report["backend_sha256"], "backend binary changed during acceptance"
        report["passed"] = True
        if args.board_binary:
            report["checks"] += ["private My Actions mounts the actual installed app after server verification", "action board opens the latest delegated approval", "board theme and chat-history toggle retain the correct scope", "account switch revokes the board and restores chat history"]
    except Exception:
        for app in apps:
            try:
                app.capture("failure")
                (app.root / "failure-snapshot.json").write_text(json.dumps(app.request("/snap", all=1), indent=2))
            except Exception:
                pass
        raise
    finally:
        for app in apps:
            app.stop()
        server.terminate()
        try:
            server.wait(timeout=15)
        except subprocess.TimeoutExpired:
            server.kill(); server.wait(timeout=5)
        log.close()
        matrix.shutdown(); matrix.server_close()
        (root / "report.json").write_text(json.dumps(report, indent=2))
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
