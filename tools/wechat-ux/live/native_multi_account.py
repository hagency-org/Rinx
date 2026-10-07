#!/usr/bin/env python3
"""Exercise production Rinx account UI + Matrix SDK against an isolated HTTP fixture.

This is not a federation/E2EE interoperability test. No personal account, profile,
or server is used; tokens and passwords below exist only in this local fixture.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import threading
import time
import urllib.parse
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from native_probe import NativeApp


class MatrixFixture:
    def __init__(self):
        self.tokens = {}
        self.logouts = []
        self.logins = []
        self.counter = 0
        fixture = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                self.route()

            def do_POST(self):
                self.route()

            def do_PUT(self):
                self.route()

            def do_DELETE(self):
                self.route()

            def route(self):
                path = urllib.parse.urlparse(self.path).path
                body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))) or b'{}')
                token = self.headers.get('Authorization', '').removeprefix('Bearer ')
                user = fixture.tokens.get(token)
                status = 200
                value = {}
                if path == '/.well-known/matrix/client':
                    value = {'m.homeserver': {'base_url': fixture.url}}
                elif path == '/_matrix/client/versions':
                    value = {'versions': ['v1.12'], 'unstable_features': {'org.matrix.msc4186': True, 'org.matrix.simplified_msc3575': True}}
                elif path.endswith('/login') and self.command == 'GET':
                    value = {'flows': [{'type': 'm.login.password'}]}
                elif path.endswith('/login'):
                    name = body.get('identifier', {}).get('user', '')
                    if body.get('password') != 'fixture-only':
                        status, value = 403, {'errcode': 'M_FORBIDDEN', 'error': 'Fixture password rejected'}
                    else:
                        fixture.counter += 1
                        user = name if name.startswith('@') else '@' + name + ':rinx-multi.test'
                        token = 'fixture-token-' + str(fixture.counter)
                        fixture.tokens[token] = user
                        fixture.logins.append(user)
                        value = {'user_id': user, 'device_id': body.get('device_id', 'DEVICE' + str(fixture.counter)), 'access_token': token, 'home_server': 'rinx-multi.test'}
                elif path.endswith('/account/whoami'):
                    if user:
                        value = {'user_id': user, 'device_id': 'FIXTURE'}
                    else:
                        status, value = 401, {'errcode': 'M_UNKNOWN_TOKEN', 'error': 'Unknown fixture token'}
                elif path.endswith('/logout'):
                    fixture.logouts.append(user)
                    fixture.tokens.pop(token, None)
                elif path.endswith('/sync'):
                    # Slow the otherwise empty fixture to avoid a busy sync loop.
                    time.sleep(.15)
                    lists = {name: {'count': 0, 'ops': []} for name in body.get('lists', {})}
                    value = {'pos': str(time.time_ns()), 'lists': lists, 'rooms': {}, 'extensions': {
                        'to_device': {'next_batch': str(time.time_ns()), 'events': []},
                        'e2ee': {'device_lists': {'changed': [], 'left': []}, 'device_one_time_keys_count': {}},
                        'account_data': {'global': [], 'rooms': {}},
                        'receipts': {'rooms': {}}, 'typing': {'rooms': {}}
                    }, 'next_batch': str(time.time_ns()), 'device_lists': {'changed': [], 'left': []}, 'account_data': {'events': []}, 'to_device': {'events': []}}
                elif '/profile/' in path:
                    name = urllib.parse.unquote(path.split('/profile/', 1)[1].split('/')[0])
                    value = {'displayname': 'Alice Fixture' if 'alice' in name else 'Bob Fixture', 'avatar_url': None}
                elif path.endswith('/pushrules/') or path.endswith('/pushrules'):
                    value = {'global': {kind: [] for kind in ['override', 'content', 'room', 'sender', 'underride']}}
                elif path.endswith('/keys/query'):
                    value = {'device_keys': {}, 'failures': {}}
                elif path.endswith('/keys/upload'):
                    value = {'one_time_key_counts': {'signed_curve25519': 50}}
                elif path.endswith('/keys/claim'):
                    value = {'one_time_keys': {}, 'failures': {}}
                elif '/account_data/' in path and self.command == 'GET':
                    status, value = 404, {'errcode': 'M_NOT_FOUND', 'error': 'No fixture account data'}
                elif path.endswith('/capabilities'):
                    value = {'capabilities': {'m.change_password': {'enabled': True}}}
                elif path.endswith('/devices'):
                    value = {'devices': []}
                elif path.endswith('/joined_rooms'):
                    value = {'joined_rooms': []}
                elif '/room_keys/version' in path:
                    status, value = 404, {'errcode': 'M_NOT_FOUND', 'error': 'No key backup'}
                data = json.dumps(value).encode()
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                try:
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError):
                    pass

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.url = f'http://127.0.0.1:{self.server.server_port}'
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def stop(self):
        self.server.shutdown()
        self.server.server_close()


class App(NativeApp):
    def request(self, route, **params):
        # Hidden Metal windows can apply input and still fail wait=1. Never
        # replay input; use a separate read-only frame barrier instead.
        barrier = route in {'/click', '/m', '/k', '/t'} and params.get('wait') == 1
        if barrier:
            params['wait'] = 0
        result = super().request(route, **params)
        if barrier:
            time.sleep(.05)
            for _ in range(20):
                try:
                    super().request('/g')
                    break
                except urllib.error.HTTPError as error:
                    if error.code != 404:
                        raise
                    time.sleep(.1)
        return result

    def launch(self, binary):
        self.root.mkdir(parents=True, exist_ok=True)
        self.output.mkdir(parents=True, exist_ok=True)
        self.log = (self.output / 'native.log').open('w')
        profile = self.root / 'profile'
        profile.mkdir(mode=0o700, exist_ok=True)
        (profile / 'window_geom_state.json').write_text(json.dumps({'inner_size': list(self.size), 'position': [50, 50], 'is_fullscreen': False}))
        env = dict(os.environ, RINX_DATA_DIR=str(profile), MAKEPAD_REMOTE=str(self.port), MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1', RUST_BACKTRACE='1')
        env.pop('MAKEPAD_FOCUS', None)
        self.process = subprocess.Popen([str(binary.resolve())], env=env, stdout=self.log, stderr=subprocess.STDOUT)
        for _ in range(150):
            if self.process.poll() is not None:
                raise RuntimeError(f'Rinx exited; see {self.output}')
            try:
                if self.request('/s')['w']:
                    return
            except OSError:
                pass
            time.sleep(.1)
        raise RuntimeError(f'Native bridge did not start; see {self.output}')

    def fill(self, widget, value):
        self.click_id(widget)
        self.request('/k', c='KeyA', cmd=1, wait=0)
        self.request('/t', t=value, wait=0)
        time.sleep(.1)

    def login(self, fixture, name, password='fixture-only'):
        self.fill('homeserver_input', fixture.url)
        self.click_id('continue_server_button')
        self.wait_text('Selected server')
        self.fill('user_id_input', '@' + name + ':rinx-multi.test')
        self.fill('password_input', password)
        self.click_id('login_button')

    def account_menu(self):
        self.click_id('account_switcher_button')
        self.wait_text('Account Settings')

    def select(self, name):
        identity = '@' + name + ':rinx-multi.test'
        for _ in range(20):
            rows = [w for w in self.snap() if w.get('i') == 'select' and w.get('t') == identity and w['r'][1] >= 32]
            if rows:
                x, y, width, height = rows[0]['r']
                self.click(x + width / 2, y + height / 2)
                return
            self.request('/m', k='scroll', x=500, y=350, dy=-1500, wait=1)
        raise AssertionError(f'Saved account row unavailable: {identity}')

    def active(self, name):
        for _ in range(120):
            widgets = self.snap()
            if any(w.get('i') == 'account_switcher_button' for w in widgets):
                self.account_menu()
                if any(w.get('i') == 'active_user_id' and w.get('t') == '@' + name + ':rinx-multi.test' for w in self.snap()):
                    return
                self.request('/k', c='Escape', wait=0)
            time.sleep(.2)
        raise AssertionError(f'Account did not become active: {name}; see {self.output}')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=Path('target/debug/rinx'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        bridge = probe.getsockname()[1]
    fixture = MatrixFixture()
    app = App(args.output.resolve(), port=bridge, size=(1000, 800), auto_login=False)
    report = {'passed': False, 'checks': [], 'fixture': 'isolated loopback Matrix HTTP fixture; production UI and SDK'}
    try:
        app.launch(args.binary)
        app.wait_text('Sign in to Rinx')
        app.login(fixture, 'alice')
        app.active('alice')
        app.capture('alice-active')
        app.click_text('Add Account')
        app.wait_text('Sign in to Rinx')
        app.login(fixture, 'bob', 'incorrect')
        app.wait_text('Fixture password rejected')
        app.click_text('Okay')
        app.select('alice')
        app.active('alice')
        report['checks'].append('failed add returns to saved Alice without another password')
        app.click_text('Add Account')
        app.wait_text('Sign in to Rinx')
        app.login(fixture, 'bob')
        app.active('bob')
        app.capture('bob-active')
        sessions = list((app.root / 'profile').glob('account_*/persistent_state/session'))
        assert len(sessions) == 2
        stores = {json.loads(p.read_text())['user_session']['user_id']: json.loads(p.read_text())['client_session']['db_path'] for p in sessions}
        assert len(set(stores.values())) == 2
        assert all(p.stat().st_mode & 0o777 == 0o600 for p in sessions)
        report['checks'].append('two accounts have separate token files and encrypted SDK stores')
        app.select('alice')
        app.active('alice')
        app.select('bob')
        app.active('bob')
        assert fixture.logins == ['@alice:rinx-multi.test', '@bob:rinx-multi.test']
        assert fixture.logouts == []
        report['checks'].append('A→B→A→B keeps both devices; switch sends no Matrix logout or password login')
        app.capture('switch-roundtrip')
        app.click_text('Account Settings')
        app.capture('settings-accounts')
        app.select('alice')
        app.active('alice')
        app.select('bob')
        app.active('bob')
        report['checks'].append('settings uses the same picker and closes when switching accounts')
        # Logout confirmation only applies to the active account.
        app.click_text('Log Out')
        app.wait_text('Log out')
        app.click_text('Log out')
        for _ in range(100):
            sessions = list((app.root / 'profile').glob('account_*/persistent_state/session'))
            if len(sessions) == 1:
                break
            time.sleep(.2)
        assert len(sessions) == 1 and json.loads(sessions[0].read_text())['user_session']['user_id'] == '@alice:rinx-multi.test'
        assert fixture.logouts == ['@bob:rinx-multi.test']
        report['checks'].append('logout revokes Bob only; Alice remains selectable')
        app.wait_text('Sign in to Rinx')
        app.select('alice')
        app.active('alice')
        app.stop()
        app.launch(args.binary)
        app.active('alice')
        assert fixture.logins == ['@alice:rinx-multi.test', '@bob:rinx-multi.test']
        report['checks'].append('restart restores the remaining account without password login')
        report['passed'] = True
    except Exception:
        try:
            app.capture('failure')
            (args.output / 'failure-snapshot.json').write_text(json.dumps(app.snap(), indent=2))
        except Exception:
            pass
        raise
    finally:
        app.stop()
        fixture.stop()
        args.output.mkdir(parents=True, exist_ok=True)
        (args.output / 'result.json').write_text(json.dumps(report, indent=2))
        print(json.dumps(report))


if __name__ == '__main__':
    main()
