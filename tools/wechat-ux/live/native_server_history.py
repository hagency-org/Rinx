#!/usr/bin/env python3
"""Test homeserver history through production native UI and Matrix discovery.

Uses temporary device-local data and loopback HTTP fixtures, no real accounts.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import threading
import time
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from native_probe import NativeApp


class Server:
    def __init__(self, valid=True):
        fixture = self
        self.requests = 0
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_GET(self):
                fixture.requests += 1
                if not valid:
                    value = {'not_matrix': True}
                elif self.path == '/.well-known/matrix/client':
                    value = {'m.homeserver': {'base_url': fixture.url}}
                elif self.path.endswith('/versions'):
                    value = {'versions': ['v1.12'], 'unstable_features': {}}
                elif self.path.endswith('/login'):
                    value = {'flows': [{'type': 'm.login.password'}]}
                else:
                    value = {}
                data = json.dumps(value).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                try:
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError):
                    pass
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.url = f'http://127.0.0.1:{self.server.server_port}/'
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
    def stop(self):
        self.server.shutdown()
        self.server.server_close()


class App(NativeApp):
    def request(self, route, **params):
        wait = params.get('wait') == 1
        if wait:
            params['wait'] = 0
        result = super().request(route, **params)
        if wait:
            # Read-only barrier after input; do not replay an applied click.
            for _ in range(50):
                try:
                    super().request('/g')
                    break
                except urllib.error.HTTPError as error:
                    if error.code != 404:
                        raise
                    time.sleep(.05)
        return result
    def launch(self, binary):
        self.root.mkdir(parents=True, exist_ok=True)
        self.output.mkdir(parents=True, exist_ok=True)
        Path('target').mkdir(exist_ok=True)
        profile = self.root / 'profile'
        profile.mkdir(exist_ok=True, mode=0o700)
        (profile / 'window_geom_state.json').write_text(json.dumps({'inner_size': list(self.size), 'position': [50, 50], 'is_fullscreen': False}))
        self.log = (self.output / 'native.log').open('w')
        self.process = subprocess.Popen([str(binary.resolve()), '--login-screen'], env=dict(os.environ,
            RINX_DATA_DIR=str(profile), MAKEPAD_REMOTE=str(self.port), MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1', RUST_BACKTRACE='1'),
            stdout=self.log, stderr=subprocess.STDOUT)
        for _ in range(150):
            if self.process.poll() is not None:
                raise RuntimeError(f'App exited; see {self.output}')
            try:
                state = self.request('/s')
                if state['pid'] != self.process.pid:
                    raise RuntimeError('Bridge belongs to another process')
                if state['w']:
                    self.wait_text('Sign in to Rinx')
                    return
            except OSError:
                pass
            time.sleep(.1)
        raise RuntimeError('Native bridge did not start')
    def fill(self, value):
        self.click_id('homeserver_input')
        self.request('/k', c='KeyA', cmd=1, wait=0)
        self.request('/k', c='Backspace', wait=0)
        if value:
            self.request('/t', t=value, wait=0)
        time.sleep(.15)
        self.request('/g')
    def rows(self):
        return sorted((widget for widget in self.snap() if widget['i'] == 'history_server'), key=lambda widget: widget['r'][1])
    def choose(self, url):
        row = next(widget for widget in self.rows() if widget.get('t') == url.rstrip('/'))
        x, y, w, h = row['r']
        self.click(x+w/2, y+h/2)
    def continue_server(self):
        self.click_id('continue_server_button')
        self.wait_text('Selected server')
    def history(self):
        return json.loads((self.root / 'profile/homeserver_history.json').read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=Path('target/debug/rinx'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    with socket.socket() as port:
        port.bind(('127.0.0.1', 0))
        bridge_port = port.getsockname()[1]
    servers = [Server(), Server(), Server(False)]
    first, second, invalid = [server.url for server in servers]
    app = App(args.output, port=bridge_port, size=(900, 800), auto_login=False)
    report = {'passed': False, 'checks': []}
    try:
        app.launch(args.binary)
        app.click_id('homeserver_input')
        assert not app.rows()
        app.fill(first)
        app.continue_server()
        assert app.history() == [first]
        report['checks'].append('successful Matrix discovery persists a valid server')
        app.click_id('edit_server_button')
        assert app.rows()
        app.continue_server()
        assert app.history() == [first]
        report['checks'].append('Continue works while history is expanded and avoids duplicates')
        app.click_id('edit_server_button')
        app.fill(second)
        app.continue_server()
        app.click_id('edit_server_button')
        app.fill('')
        assert [row['t'] for row in app.rows()] == [second.rstrip('/'), first.rstrip('/')]
        app.capture('desktop-history')
        report['checks'].append('focus shows recent-first destinations and empty input shows all')
        before = app.history()
        app.fill(invalid)
        app.click_id('continue_server_button')
        app.wait_text('Could not check this server')
        assert app.history() == before
        report['checks'].append('failed Matrix discovery does not enter history')
        app.fill(str(servers[0].server.server_port))
        assert [row['t'] for row in app.rows()] == [first.rstrip('/')]
        old_requests = servers[0].requests
        app.choose(first)
        assert not app.rows()
        assert any(widget['i'] == 'homeserver_input' and widget.get('t') == first for widget in app.snap())
        assert servers[0].requests == old_requests
        app.continue_server()
        assert servers[0].requests > old_requests and app.history() == [first, second]
        report['checks'].append('typing filters; selection only fills; Continue rechecks and updates recency')
        app.click_id('edit_server_button')
        app.fill('')
        app.request('/k', c='Escape', wait=1)
        assert not app.rows()
        app.click_id('homeserver_input')
        assert len(app.rows()) == 2
        app.click_id('logo_image')
        assert not app.rows()
        report['checks'].append('Escape/outside click dismiss; repeat input click reopens')
        app.stop()
        app.launch(args.binary)
        app.click_id('homeserver_input')
        assert len(app.rows()) == 2
        app.capture('restart-history')
        report['checks'].append('restart retains history without login credentials')
        app.stop()
        app.size = (406, 776)
        app.launch(args.binary)
        app.click_id('homeserver_input')
        assert len(app.rows()) == 2
        assert all(row['r'][0] >= 0 and row['r'][0] + row['r'][2] <= 406 for row in app.rows())
        app.capture('phone-history')
        report['checks'].append('history fits narrow phone layout')
        app.stop()
        seeded = app.history() + [f'http://127.0.0.1:{port}/' for port in range(11000, 11010)]
        (app.root / 'profile/homeserver_history.json').write_text(json.dumps(seeded))
        app.launch(args.binary)
        app.click_id('homeserver_input')
        rows = app.rows()
        x, y, w, h = rows[0]['r']
        app.request('/m', k='scroll', x=x+w/2, y=y+h, dy=600, wait=1)
        assert [row['t'] for row in app.rows()] != [row['t'] for row in rows]
        app.fill(str(servers[0].server.server_port))
        assert [row['t'] for row in app.rows()] == [first.rstrip('/')]
        report['checks'].append('filtering a scrolled long list returns to the first matching result')
        report['passed'] = True
    except Exception:
        try:
            app.capture('failure')
        except Exception:
            pass
        raise
    finally:
        app.stop()
        for server in servers:
            server.stop()
        args.output.mkdir(parents=True, exist_ok=True)
        (args.output / 'result.json').write_text(json.dumps(report, indent=2))
        print(json.dumps(report))


if __name__ == '__main__':
    main()
