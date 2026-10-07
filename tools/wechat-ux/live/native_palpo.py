#!/usr/bin/env python3
"""Shared native Palpo instrument helpers and coordinator test entry point.

The executable scenario uses Rust Palpo with an explicit loopback Matrix fixture.
SQLite workflows, HTTP sessions, Rinx transport, widgets and inputs are real.
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.error
from native_probe import NativeApp

class PalpoApp(NativeApp):
    def snap(self):
        return [w for w in super().snap() if w.get('ty') != 'Splash']

    def click_id(self, widget_id, modal=False):
        window_width, window_height = self.request('/s')['w'][0]['sz']
        def target():
            return next((w for w in self.request('/snap', all=1)['s']
                         if w.get('i') == widget_id and w['r'][2] > 0 and w['r'][3] > 0
                         and w['r'][1] >= 30 and w['r'][1] + w['r'][3] <= window_height - 15), None)
        found = target()
        if found is None:
            x, y = min(300, window_width - 30), min(650, window_height - 40)
            self.request('/m', k='scroll', x=x, y=y, dy=-3000, wait=1)
            for _ in range(24):
                found = target()
                if found is not None:
                    break
                self.request('/m', k='scroll', x=x, y=y, dy=130, wait=1)
        if found is None:
            raise AssertionError(f'Could not scroll widget into view: {widget_id}')
        x, y, width, height = found['r']
        if modal:
            self.request('/click', x=x + width / 2, y=y + height / 2, wait=0)
        else:
            self.click(x + width / 2, y + height / 2)

    def request(self, route, **params):
        # SDK 1f3b1de can fail wait=1 after already applying the input when a
        # hidden Metal frame cannot immediately submit. Never replay that input.
        # A separate read-only grab is the frame barrier instead.
        barrier = route in {'/click', '/m', '/k', '/t'} and params.get('wait') == 1
        if barrier:
            params['wait'] = 0
        try:
            result = super().request(route, **params)
            if barrier:
                time.sleep(.05)
                super().request('/g')
            return result
        except urllib.error.HTTPError as error:
            detail = error.read().decode('utf-8', errors='replace')
            self.trace.append({'bridge_error': route, 'status': error.code, 'detail': detail})
            raise NativeBridgeError(f'{route}: HTTP {error.code}: {detail}') from error


class NativeBridgeError(RuntimeError):
    pass



def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def launch(root, binary, endpoint, admin=False, narrow=False, session_file=None):
    profile = root / 'profile'
    (profile / 'app').mkdir(parents=True, exist_ok=True)
    app = PalpoApp(root, port=port(), auto_login=False)
    app.output.mkdir(parents=True)
    app.log = (app.output / 'native.log').open('w')
    env = dict(os.environ, RINX_DATA_DIR=str(profile.resolve()), MAKEPAD_HIDE_WINDOWS='1',
               MAKEPAD_NO_FOCUS='1', MAKEPAD_REMOTE=str(app.port), PALPO_FIXTURE_URL=endpoint)
    env.pop('MAKEPAD_FOCUS', None)
    env.pop('PALPO_LIVE_SESSION_FILE', None)
    if session_file:
        env['PALPO_LIVE_SESSION_FILE'] = str(session_file.resolve())
    args = [str(binary.resolve())] + (['--admin'] if admin else []) + (['--narrow'] if narrow else [])
    app.process = subprocess.Popen(args, env=env, stdout=app.log, stderr=subprocess.STDOUT)
    for _ in range(120):
        if app.process.poll() is not None:
            raise RuntimeError(f'Native fixture exited; see {app.output}')
        try:
            status = app.request('/s')
            if status['w']:
                app.wait_text('Pending actions stay here', timeout=10)
                return app
        except (OSError, NativeBridgeError):
            pass
        time.sleep(.1)
    app.stop()
    raise RuntimeError(f'Native bridge did not draw: {app.output}')


def fill(app, label, value):
    tree = app.request('/snap', all=1)['s']
    fields = [w for w in tree if w.get('ty') == 'TextInput' and w['r'][2] > 0]
    (app.root / 'last-fields.json').write_text(json.dumps(tree, indent=2))
    # Fields have their labels immediately above them; visual order is stable.
    labels = [w for w in tree if w.get('t') == label and w.get('ty') != 'TextInput']
    assert labels, (label, app.snap())
    y = labels[-1]['r'][1]
    target = min((w for w in fields if w['r'][1] >= y), key=lambda w: w['r'][1])
    x, y, width, height = target['r']
    app.click(x + width / 2, y + height / 2)
    app.request('/k', c='KeyA', cmd=1, wait=1)
    app.request('/t', t=value, wait=1)


def inspect(app):
    app.request('/event', data='palpo:inspect', wait=1)
    return json.loads((app.root / 'profile/inspection.json').read_text())


def main():
    # ADR 0011 replaces the contribution/admin-verdict fixture with Rust and
    # a delegated coordinator. Keep this established CLI entry point usable.
    from native_palpo_coordinator import main as run_coordinator
    run_coordinator()


if __name__ == '__main__':
    main()
