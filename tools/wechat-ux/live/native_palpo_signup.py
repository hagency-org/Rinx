#!/usr/bin/env python3
"""Real Rust signup worker and native mini-app; isolated Matrix HTTP fixture.

Tests the source-event handoff, not an actual Matrix SDK invitation join.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import threading
import time
from urllib.parse import unquote, urlsplit, parse_qs
from urllib.request import Request
import uuid

from native_palpo import port
from native_palpo_coordinator import HTTP, launch


class Matrix(BaseHTTPRequestHandler):
    guard = threading.RLock()
    state = []
    events = []
    transactions = {}
    users = {"@admin:example.test": {"admin": True}}
    devices = {}
    registrations = 0

    def log_message(self, *_):
        pass

    def actor(self):
        return {f"Bearer {a}-secret": f"@{a}:example.test" for a in ("admin", "owner", "signup")}.get(self.headers.get("Authorization"))

    def reply(self, value, status=200):
        raw = json.dumps(value).encode()
        self.send_response(status); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw))); self.end_headers(); self.wfile.write(raw)

    def do_GET(self):
        parts = [unquote(p) for p in urlsplit(self.path).path.split('/')]
        actor = self.actor()
        with self.guard:
            if not actor: self.reply({}, 401); return
            if parts[-1] == 'whoami': self.reply({'user_id': actor}); return
            if parts[-1] == 'appservices': self.reply({}, 200 if actor == '@admin:example.test' else 403); return
            if parts[1:5] == ['_palpo', 'admin', 'v2', 'users']:
                value = self.users.get(parts[-1]); self.reply(value or {}, 200 if value else 404); return
            if 'whois' in parts:
                device = self.devices.get(parts[-1]); self.reply({'devices': {device: {}} if device else {}}); return
            if 'directory' in parts: self.reply({'room_id': '!signup:example.test'} if self.state else {}, 200 if self.state else 404); return
            if parts[-1] == 'state': self.reply(self.state); return
            if parts[-1] == 'messages':
                query = parse_qs(urlsplit(self.path).query)
                assert query['dir'] == ['f']
                start = int(query.get('from', ['0'])[0]); end = min(start + 100, len(self.events))
                self.reply({'chunk': self.events[start:end], 'end': str(end)}); return
            if 'event' in parts:
                event = next((e for e in self.events if e['event_id'] == parts[-1]), None)
                self.reply(event or {}, 200 if event else 404); return
        self.reply({}, 404)

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        with self.guard:
            if self.path.endswith('/createRoom') and self.actor() == '@signup:example.test':
                self.state.extend(body['initial_state'] + [
                    {'type': 'm.room.join_rules', 'state_key': '', 'content': {'join_rule': 'invite'}},
                    {'type': 'm.room.member', 'state_key': '@signup:example.test', 'content': {'membership': 'join'}},
                    {'type': 'm.room.member', 'state_key': '@admin:example.test', 'content': {'membership': 'join'}}])
                self.reply({'room_id': '!signup:example.test'}); return
            if self.path.endswith('/register'):
                type(self).registrations += 1
                if 'auth' not in body: self.reply({'session': 'fixture', 'flows': [{'stages': ['m.login.registration_token']}]}, 401); return
                assert body['auth']['token'] == 'registration-fixture'
                user = '@' + body['username'] + ':example.test'
                self.users[user] = {'admin': False}; self.devices[user] = body['device_id']
                self.reply({'user_id': user, 'device_id': body['device_id'], 'access_token': 'temporary-fixture-session'}); return
            if self.path.endswith('/logout'): self.reply({}); return
        self.reply({}, 403)

    def do_PUT(self):
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        parts = [unquote(p) for p in urlsplit(self.path).path.split('/')]
        with self.guard:
            if 'send' in parts and self.actor() == '@signup:example.test':
                txn = parts[-1]
                if txn in self.transactions:
                    event = self.transactions[txn]; assert event['content'] == body
                else:
                    event = {'event_id': '$signup' + str(len(self.events)), 'type': 'm.room.message', 'sender': self.actor(), 'content': body}
                    self.transactions[txn] = event; self.events.append(event)
                self.reply({'event_id': event['event_id']}); return
            if parts[-1] == 'displayname': self.reply({}); return
        self.reply({}, 403)


def request(endpoint, path, body=None):
    req = Request(endpoint + path, data=json.dumps(body).encode() if body is not None else None,
                  headers={'Content-Type': 'application/json'})
    with HTTP.open(req, timeout=15) as response:
        return json.load(response)


def wait(check, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            value = check()
            if value: return value
        except OSError:
            pass
        time.sleep(.1)
    raise AssertionError('Condition did not become true')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/fast/examples/palpo_miniapp'))
    args = parser.parse_args()
    root = Path('target/palpo-signup-validation') / uuid.uuid4().hex; root.mkdir(parents=True)
    matrix = ThreadingHTTPServer(('127.0.0.1', 0), Matrix)
    threading.Thread(target=matrix.serve_forever, daemon=True).start()
    endpoint = f'http://127.0.0.1:{port()}'
    config = root / 'accounts.json'
    config.write_text(json.dumps({'botMxid': '@signup:example.test', 'botToken': 'signup-secret', 'adminToken': 'admin-secret',
        'approvers': ['@admin:example.test'], 'passwordKey': '12' * 32, 'registrationToken': 'registration-fixture'})); config.chmod(0o600)
    env = {k: v for k, v in os.environ.items() if not k.startswith('PALPO_')}
    env.update(PALPO_SERVER_NAME='example.test', PALPO_URL=f'http://127.0.0.1:{matrix.server_port}',
               PUBLIC_ORIGIN=endpoint, PALPO_OPERATIONS_LISTEN=endpoint.removeprefix('http://'),
               PALPO_ADMIN_DATABASE=str((root / 'admin.sqlite').resolve()), PALPO_ACCOUNT_CONFIG=str(config.resolve()))
    log = (root / 'backend.log').open('w')
    server = subprocess.Popen([str(args.backend.resolve())], env=env, stdout=log, stderr=subprocess.STDOUT)
    app = None
    checks = []
    try:
        wait(lambda: request(endpoint, '/api/account-access')['ready'])
        data = {'id': uuid.uuid4().hex, 'receipt': 'c' * 64, 'username': 'signupfixture', 'password': 'Isolated signup fixture!',
                'displayName': 'Signup fixture', 'reason': 'Review this isolated native signup request.'}
        request(endpoint, '/api/account-requests', data)
        receipt = {'id': data['id'], 'receipt': data['receipt']}
        wait(lambda: request(endpoint, '/api/account-requests/status', receipt)['request']['status'] == 'pending')
        app = launch(root / 'admin', args.binary, endpoint, 'admin')
        app.wait_text('Signups'); app.click_id('accounts'); app.wait_text('Signup fixture · pending')
        app.capture('signup-pending')
        app.click_id('open_signup'); app.wait_text('Opening the original approval request in Rinx.')
        navfile = root / 'admin/profile/navigation.json'
        target = wait(lambda: json.loads(navfile.read_text()) if navfile.exists() else None)
        with Matrix.guard:
            original = next(e for e in Matrix.events if e.get('content', {}).get('org.octos.approval_request', {}).get('request_id') == data['id'])
            assert target == {'v': 1, 'requestId': data['id'], 'account': '@admin:example.test', 'roomId': '!signup:example.test', 'eventId': original['event_id']}
            assert Matrix.registrations == 0
        checks.extend(['native signup list uses Rust account projection', 'Open signup resolves the verified original event', 'navigation sends no approval verdict'])
        app.capture('signup-original-handoff')
        with Matrix.guard:
            Matrix.events.append({'event_id': '$decision', 'type': 'm.room.message', 'sender': '@admin:example.test', 'content': {
                'msgtype': 'm.text', 'body': 'Approve', 'm.relates_to': {'m.in_reply_to': {'event_id': original['event_id']}},
                'org.octos.approval_response': {'request_id': data['id'], 'source_event_id': original['event_id'],
                    'tool_args_digest': original['content']['org.octos.approval_request']['tool_args_digest'], 'decision': 'approve'}}})
        wait(lambda: request(endpoint, '/api/account-requests/status', receipt)['request']['status'] == 'registered')
        app.click_id('refresh'); app.wait_text('Signup fixture · registered'); app.capture('signup-registered')
        with Matrix.guard: assert Matrix.registrations == 2
        checks.extend(['Rust worker consumes bound Matrix verdict and completes UIAA registration', 'native list refresh shows terminal registration'])
        report = {'passed': True, 'checks': checks, 'scope': 'real Rust worker and Rinx host; Matrix fixture; no real invitation join',
                  'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'backend_sha256': hashlib.sha256(args.backend.read_bytes()).hexdigest()}
        (root / 'report.json').write_text(json.dumps(report, indent=2)); print(json.dumps({'evidence': str(root.resolve()), **report}, indent=2))
    finally:
        if app: app.stop()
        server.terminate(); server.wait(timeout=15); log.close(); matrix.shutdown(); matrix.server_close()


if __name__ == '__main__':
    main()
