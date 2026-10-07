#!/usr/bin/env python3
"""Measure an isolated optimized Rinx with Makepad's native timing monitor.

The window is visible but unfocused: hidden macOS windows are throttled and
cannot provide representative frame pacing. Uses only a loopback Matrix fixture.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from native_probe import NativeApp
from native_multi_account import App, MatrixFixture

SENTENCE = 'The quick brown fox types smoothly. 测试输入是否流畅。'


class PerformanceApp(App):
    def request(self, route, **params):
        return NativeApp.request(self, route, **params)

    def launch(self, binary):
        with socket.socket() as sock:
            if sock.connect_ex(('127.0.0.1', self.port)) == 0:
                raise RuntimeError('Refusing to use an occupied remote port')
        self.root.mkdir(parents=True, exist_ok=True)
        self.output.mkdir(parents=True, exist_ok=True)
        self.log = (self.output / 'native.log').open('w')
        profile = self.root / 'profile'
        profile.mkdir(mode=0o700, exist_ok=True)
        (profile / 'window_geom_state.json').write_text(json.dumps({'inner_size': list(self.size), 'position': [50, 50], 'is_fullscreen': False}))
        self.report = self.root / 'current.json'
        env = dict(os.environ, RINX_DATA_DIR=str(profile), MAKEPAD_REMOTE=str(self.port), MAKEPAD_NO_FOCUS='1', RINX_PERF_OUTPUT=str(self.report), RUST_BACKTRACE='1')
        for name in ('MAKEPAD_HIDE_WINDOWS', 'MAKEPAD_FOCUS'):
            env.pop(name, None)
        self.process = subprocess.Popen([str(binary.resolve())], env=env, stdout=self.log, stderr=subprocess.STDOUT)
        for _ in range(200):
            if self.process.poll() is not None:
                raise RuntimeError(f'Rinx exited; see {self.output}')
            try:
                status = self.request('/s')
                assert status['pid'] == self.process.pid
                if status['w']:
                    self.snap()
                    return
            except OSError:
                pass
            time.sleep(.1)
        raise RuntimeError(f'Native bridge did not start; see {self.output}')

    def measure(self, name, exercise):
        self.report.unlink(missing_ok=True)
        self.request('/event', data='rinx.perf.start')
        time.sleep(.1)
        def cpu_seconds():
            parts = subprocess.check_output(['ps', '-p', str(self.process.pid), '-o', 'time='], text=True).strip().split(':')
            return sum(float(value) * 60 ** index for index, value in enumerate(reversed(parts)))
        cpu_start = cpu_seconds()
        exercise()
        cpu_used = cpu_seconds() - cpu_start
        self.request('/event', data='rinx.perf.stop')
        for _ in range(100):
            if self.report.exists():
                break
            time.sleep(.05)
        report = json.loads(self.report.read_text())
        report['process_cpu_seconds'] = cpu_used
        (self.root / (name + '.json')).write_text(json.dumps(report, indent=2))
        def stats(values):
            values = sorted(values)
            if not values:
                return {'count': 0}
            return {'count': len(values), 'p50_ms': values[len(values)//2], 'p95_ms': values[min(len(values)-1, int(len(values)*.95))], 'max_ms': max(values), 'total_ms': sum(values)}
        summary = {'name': name, 'elapsed_ms': report['elapsed_ms'], 'process_cpu_seconds': cpu_used, 'frames_painted': report['frames_painted'], 'events': {key: stats(value) for key, value in report['events_and_phases_ms'].items()}}
        print(json.dumps(summary), flush=True)
        return summary

    def type_sentence(self):
        for char in SENTENCE:
            self.request('/t', t=char, wait=0)
            time.sleep(.06)
        time.sleep(.3)
        assert any(SENTENCE in (widget.get('t') or '') for widget in self.snap()), 'Typed text was not preserved'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--mode', choices=['app', 'moments', 'chat'], default='app')
    parser.add_argument('--assert-idle', action='store_true')
    args = parser.parse_args()
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    app = PerformanceApp(args.output, port=port, size=(1000, 800), auto_login=False)
    fixture = MatrixFixture()
    results = []
    try:
        app.launch(args.binary)
        time.sleep(3)
        results.append(app.measure('startup-idle', lambda: time.sleep(5)))
        if args.mode == 'app':
            app.click_id('homeserver_input')
            results.append(app.measure('login-typing', app.type_sentence))
            app.login(fixture, 'alice')
            app.active('alice')
            app.click(850, 150)
            time.sleep(3)
            results.append(app.measure('home-idle', lambda: time.sleep(5)))
            (app.root / 'home-widgets.json').write_text(json.dumps(app.snap(), indent=2))
            app.account_menu()
            time.sleep(2)
            results.append(app.measure('account-menu-idle', lambda: time.sleep(5)))
        elif args.mode == 'chat':
            (app.root / 'chat-widgets.json').write_text(json.dumps(app.snap(), indent=2))
            inputs = [w for w in app.snap() if w['ty'] in ('TextInput', 'MessageTextInput')]
            assert inputs, 'No visible chat composer'
            x, y, width, height = inputs[-1]['r']
            app.click(x + width / 2, y + height / 2)
            results.append(app.measure('chat-typing', app.type_sentence))
            time.sleep(2)
            results.append(app.measure('chat-rest', lambda: time.sleep(5)))
        else:
            def scroll():
                for _ in range(20):
                    app.request('/m', k='scroll', x=300, y=500, dy=30, wait=0)
                    time.sleep(.06)
                time.sleep(.3)
            results.append(app.measure('moments-scroll', scroll))
            time.sleep(3)
            results.append(app.measure('moments-rest', lambda: time.sleep(5)))
        app.capture('final')
    finally:
        app.stop()
        fixture.stop()
        args.output.mkdir(parents=True, exist_ok=True)
        (args.output / 'summary.json').write_text(json.dumps({'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'results': results}, indent=2))
    if args.assert_idle:
        for result in results:
            if result['name'].endswith(('idle', 'rest')):
                # A focused composer legitimately animates its caret. Count
                # widget redraws separately from compositor animation presents.
                draws = result['events'].get('event.draw', {}).get('count', 0)
                assert draws < 30, f"Redraw loop: {result['name']} performed {draws} widget draws"


if __name__ == '__main__':
    main()
