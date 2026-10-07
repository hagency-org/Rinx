#!/usr/bin/env python3
"""Exercise the production mini-app window host with real hidden Makepad windows."""
import argparse, json, os, pathlib, subprocess, time, uuid
from native_palpo import PalpoApp, port

class WindowApp(PalpoApp):
    window = 0
    def request(self, route, **params):
        if route in {'/snap', '/g', '/click', '/m', '/k', '/t', '/close'}:
            params.setdefault('w', self.window)
        return super().request(route, **params)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=pathlib.Path, required=True)
    args = parser.parse_args()
    root = pathlib.Path('target/miniapp-window-validation') / uuid.uuid4().hex
    profile = root.resolve() / 'profile'; profile.mkdir(parents=True)
    app = WindowApp(root, port=port(), auto_login=False); app.output.mkdir(parents=True)
    app.log = (app.output / 'native.log').open('w')
    env = dict(os.environ, RINX_DATA_DIR=str(profile), MAKEPAD_REMOTE=str(app.port), MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1')
    env.pop('MAKEPAD_FOCUS', None)
    app.process = subprocess.Popen([str(args.binary.resolve())], env=env, stdout=app.log, stderr=subprocess.STDOUT)
    report = {'passed': False, 'checks': [], 'evidence': str(root.resolve())}
    try:
        for _ in range(100):
            try:
                if app.request('/s')['w']: break
            except OSError: pass
            time.sleep(.1)
        app.wait_text('Main conversation stays open')
        assert len(app.request('/s')['w']) == 1
        app.click_id('open'); time.sleep(.3)
        windows = app.request('/s')['w']; assert len(windows) == 2, windows
        app.request('/event', data='window:inspect', wait=1)
        before = json.loads((profile / 'inspection.json').read_text())
        app.window = before['miniapp_window']; assert app.window is not None
        app.wait_text('Built-in apps'); app.capture('catalog-light')
        app.click_id('import_app'); app.wait_text('OctoSense bundle folder')
        target = next(w for w in app.snap() if w.get('i') == 'path' and w['ty'] == 'TextInput')
        x,y,w,h = target['r']; app.click(x+w/2,y+h/2)
        app.request('/t', t='/tmp/unsaved-miniapp-bundle', wait=1)
        app.wait_text('/tmp/unsaved-miniapp-bundle')
        app.request('/event', data='window:dark', wait=1); app.capture('import-dark')
        app.wait_text('/tmp/unsaved-miniapp-bundle')
        app.request('/event', data='window:inspect', wait=1)
        after = json.loads((profile / 'inspection.json').read_text())
        assert before == after, (before, after)
        report['checks'].append('separate native window, keyboard input, preserved main draft/display context and theme reapply')
        app.request('/close'); time.sleep(.3)
        assert len(app.request('/s')['w']) == 1
        app.window = 0; app.wait_text('Unsent chat draft'); app.click_id('open'); time.sleep(.3)
        app.request('/event', data='window:inspect', wait=1)
        app.window = json.loads((profile/'inspection.json').read_text())['miniapp_window']
        app.wait_text('Built-in apps'); assert len(app.request('/s')['w']) == 2
        app.click_id('close'); time.sleep(.3)
        assert len(app.request('/s')['w']) == 1
        report['checks'].append('OS close, reopen and panel Back close only the mini-app window')
        errors = [line for line in (app.output/'native.log').read_text().splitlines() if '[E]' in line or 'on_render closure failed' in line]
        assert not errors, errors
        report['passed'] = True
    finally:
        app.window = 0
        app.stop()
        (root/'report.json').write_text(json.dumps(report, indent=2)+'\n')
        print(json.dumps(report, indent=2))

if __name__ == '__main__': main()
