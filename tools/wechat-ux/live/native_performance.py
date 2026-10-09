#!/usr/bin/env python3
"""Measure an isolated optimized Rinx with Makepad's native timing monitor.

The window is visible but unfocused: hidden macOS windows are throttled and
cannot provide representative frame pacing. App mode uses a loopback Matrix
fixture; chat mode takes the offline production-widget `chat_theme` example.

On Linux the Wayland app_id is the executable's file name, so a copy of the
binary under a distinct name can be routed by window rule to a headless
compositor output (Hyprland: `hyprctl output create headless`), keeping the
measured window off the desktop and its focus. Linux samples also report
per-thread CPU time and event-loop wakeups from /proc.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
from native_probe import NativeApp
from native_multi_account import App, MatrixFixture

SENTENCE = 'The quick brown fox types smoothly. 测试输入是否流畅。'


def process_sample(pid):
    """CPU seconds per thread, plus the UI (main) thread's voluntary sleeps.

    Linux `ps -o time` has one-second resolution, too coarse for idle samples;
    per-thread schedstat run time is in nanoseconds. Each voluntary context
    switch of the main thread is one event-loop sleep, so the delta counts
    event-loop wakeups.
    """
    if not sys.platform.startswith('linux'):
        parts = subprocess.check_output(['ps', '-p', str(pid), '-o', 'time='], text=True).strip().split(':')
        return {'cpu': sum(float(value) * 60 ** index for index, value in enumerate(reversed(parts)))}
    threads = {}
    for task in Path(f'/proc/{pid}/task').iterdir():
        try:
            name = (task / 'comm').read_text().strip()
            threads[f'{name}:{task.name}'] = int((task / 'schedstat').read_text().split()[0]) / 1e9
        except (OSError, ValueError, IndexError):
            pass  # The thread exited while sampling.
    status = Path(f'/proc/{pid}/status').read_text()
    wakeups = next(int(line.split()[1]) for line in status.splitlines() if line.startswith('voluntary_ctxt_switches'))
    return {'cpu': sum(threads.values()), 'threads': threads, 'ui_wakeups': wakeups}


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
        before = process_sample(self.process.pid)
        exercise()
        after = process_sample(self.process.pid)
        cpu_used = after['cpu'] - before['cpu']
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
        if 'ui_wakeups' in after:
            summary['ui_thread_wakeups'] = after['ui_wakeups'] - before['ui_wakeups']
            used = {thread: (cpu - before['threads'].get(thread, 0)) * 1000 for thread, cpu in after['threads'].items()}
            summary['busiest_threads_ms'] = dict(sorted(used.items(), key=lambda item: -item[1])[:5])
        # The monitor ring holds the most recent presented frames: paint-to-paint
        # gaps and per-channel main-thread (and GPU) microseconds.
        frames = report['recent_frames']
        summary['frame_gap_ms'] = stats([frame['gap_ms'] for frame in frames])
        summary['frame_channels_ms'] = {channel: sum(frame['channel_us'][index] for frame in frames) / 1000
                                        for index, channel in enumerate(report['channels']) if index < len(frames[0]['channel_us'])} if frames else {}
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
    # Leave a quiet long-poll interval after each sync response. A stream of
    # responses every 150 ms continually resets the SDK's sync-indicator
    # debounce, leaving a legitimate spinner active during an "idle" sample.
    fixture = MatrixFixture(sync_delay=1.0)
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
            app.click_id('account_switcher_button')
            assert not any(w.get('t') == 'Account Settings' for w in app.snap()), 'Account menu did not close'
            time.sleep(3)
            results.append(app.measure('home-idle', lambda: time.sleep(5)))
            (app.root / 'home-widgets.json').write_text(json.dumps(app.snap(), indent=2))
            app.account_menu()
            time.sleep(2)
            results.append(app.measure('account-menu-idle', lambda: time.sleep(5)))
        elif args.mode == 'chat':
            widgets = app.snap()
            (app.root / 'chat-widgets.json').write_text(json.dumps(widgets, indent=2))
            assert any(w['ty'] in ('RoomInputBar', 'MobileRoomInputBar') for w in widgets), \
                'Chat mode requires an open composer; use --binary target/release/examples/chat_theme'
            inputs = [w for w in widgets if w['i'] == 'text_input' and w['ty'] in ('TextInput', 'MessageTextInput')]
            assert inputs, 'No visible chat composer'
            x, y, width, height = inputs[-1]['r']
            app.click(x + width / 2, y + height / 2)
            results.append(app.measure('chat-typing', app.type_sentence))
            time.sleep(2)
            results.append(app.measure('chat-caret', lambda: time.sleep(5)))
            app.click_id('header')
            time.sleep(1)
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
                # The focused composer's animated caret is measured separately;
                # idle samples must settle both widget draws and GPU repaints.
                draws = result['events'].get('event.draw', {}).get('count', 0)
                assert draws < 30, f"Redraw loop: {result['name']} performed {draws} widget draws"
                assert result['frames_painted'] < 30, f"Repaint loop: {result['name']} submitted {result['frames_painted']} frames"


if __name__ == '__main__':
    main()
