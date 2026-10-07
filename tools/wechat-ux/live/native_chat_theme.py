#!/usr/bin/env python3
"""Score whole native chat captures and check theme contrast/reload retention.

Uses production Message, Timeline, link preview and composer widgets with fixed
offline conversation data. This is component visual acceptance, not a Matrix
or physical-phone test. The external pixel scorer is never modified.
"""
import argparse
import hashlib
import html
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import uuid
from PIL import Image

from native_probe import NativeApp
from native_theme_presets import PALETTES


def launch(root, binary, narrow):
    profile = root / 'profile'
    profile.mkdir(parents=True)
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    app = NativeApp(root, port, auto_login=False)
    app.output.mkdir(parents=True)
    app.log = (app.output / 'native.log').open('w')
    app.process = subprocess.Popen([str(binary.resolve()), *(['--narrow'] if narrow else [])],
        env=dict(os.environ, RINX_DATA_DIR=str(profile.resolve()), MAKEPAD_REMOTE=str(port),
                 MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1'),
        stdout=app.log, stderr=subprocess.STDOUT)
    for _ in range(120):
        if app.process.poll() is not None:
            raise RuntimeError('Native chat fixture exited')
        try:
            assert app.request('/s')['pid'] == app.process.pid
            if any(w.get('t') == 'Design review' for w in app.snap()):
                return app
        except OSError:
            pass
        time.sleep(.25)
    raise RuntimeError('Chat fixture did not start')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/fast/examples/chat_theme'))
    parser.add_argument('--scorer', type=Path, required=True)
    parser.add_argument('--root', type=Path, default=Path('target/chat-theme-review'))
    parser.add_argument('--presets', action='store_true', help='Also check all named light/dark palettes')
    parser.add_argument('--first-preset', type=int, default=1, help='Start preset captures at this one-based index')
    args = parser.parse_args()
    assert 1 <= args.first_preset <= len(PALETTES)
    assert args.presets or args.first_preset == 1, '--first-preset requires --presets'
    spec = importlib.util.spec_from_file_location('pixel_uxscore', args.scorer)
    scorer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(scorer)
    root = (args.root / uuid.uuid4().hex).resolve()
    root.mkdir(parents=True)
    binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    report = {'passed': False, 'functional_passed': False, 'target': 9.5, 'screens': [],
              'binary_sha256': binary_hash,
              'scorer_sha256': hashlib.sha256(args.scorer.read_bytes()).hexdigest(),
              'scope': 'Offline production chat components; whole unedited native captures'}
    try:
        for narrow in (False, True):
            size = 'narrow' if narrow else 'desktop'
            app = launch(root / size, args.binary, narrow)
            try:
                app.click_id('text_input')
                app.request('/t', t='Keep this unsent draft', wait=1)
                combinations = [(0, 'rinx', mode, accent) for mode, accent in
                    (('light', 'teal'), ('dark', 'teal'), ('dark', 'violet'), ('light', 'violet'))]
                if args.first_preset > 1:
                    combinations = []
                if args.presets:
                    combinations.extend((i, name, mode, 'teal')
                        for i, (_, name) in enumerate(PALETTES, 1) if i >= args.first_preset
                        for mode in ('light', 'dark'))
                for preset, palette, appearance, accent in combinations:
                    app.request('/event', data=f'theme:preset:{preset}', wait=1)
                    for value in (appearance, accent):
                        app.request('/event', data='theme:' + value, wait=1)
                    time.sleep(.6)
                    app.request('/event', data='theme:inspect', wait=1)
                    colors = json.loads((app.root / 'profile/chat-theme.json').read_text())
                    rgb = lambda role: tuple(bytes.fromhex(colors[role][1:]))
                    text_ratios = [scorer.contrast_ratio(rgb(fg), rgb(bg))
                        for fg in ('color.content.primary', 'color.content.secondary')
                        for bg in ('color.surface.page', 'color.chat.incoming', 'color.chat.outgoing')]
                    assert min(text_ratios) >= 4.5, colors
                    fill_ratios = [scorer.contrast_ratio(rgb('color.surface.page'), rgb(bg))
                                   for bg in ('color.chat.incoming', 'color.chat.outgoing')]
                    # OctosCode's original dark variants retain their source fills.
                    # New variants and Rinx defaults enforce stronger separation.
                    threshold = 1.05 if 1 <= preset <= 4 and appearance == 'dark' else 1.15
                    assert min(fill_ratios) >= threshold, colors
                    snapshot = app.snap()
                    assert any(w['i'] == 'text_input' and w.get('t') == 'Keep this unsent draft' for w in snapshot)
                    name = f'{size}-{palette}-{appearance}-{accent}'
                    path = app.capture(name)
                    # State alone misses stale GPU draws after a stylesheet reload.
                    # Each fallback avatar must actually render its white initial.
                    pixels = Image.open(path).convert('RGB')
                    scale = pixels.width / (430 if narrow else 1000)
                    timeline = next(w['r'] for w in snapshot if w['i'] == 'timeline')
                    canvas_pixel = pixels.getpixel((round((timeline[0] + timeline[2] / 2) * scale),
                                                    round((timeline[1] + 4) * scale)))
                    assert canvas_pixel == rgb('color.surface.page'), (name, canvas_pixel, colors)
                    initials = [w for w in snapshot if w['ty'] == 'Label' and w.get('t') in ('E', 'L', 'Y')]
                    composer_top = min(w['r'][1] for w in snapshot if w['i'] in ('mobile_input', 'desktop_input'))
                    visible_initials = [w for w in initials if w['r'][1] + w['r'][3] <= composer_top]
                    assert visible_initials, 'No visible avatar initials to check'
                    for initial in visible_initials:
                        x, y, width, height = initial['r']
                        region = pixels.crop(tuple(round(v * scale) for v in (x, y, x + width, y + height)))
                        assert sum(min(pixel) > 240 for pixel in region.getdata()) > 10, (name, initial)
                    (app.root / (name + '.json')).write_text(json.dumps(snapshot, indent=2))
                    score = scorer.score(str(path))
                    report['screens'].append({'name': name, 'path': str(path.relative_to(root)),
                        'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'colors': colors,
                        'minimum_text_contrast': min(text_ratios), 'canvas_bubble_contrast': fill_ratios,
                        'visible_avatar_initials': len(visible_initials),
                        'score': score})
                    print(json.dumps({'screen': name, 'score': score['total']}), flush=True)
                log = (app.output / 'native.log').read_text(errors='replace')
                assert '[E]' not in log and 'panicked at' not in log, 'Native rendering errors'
            finally:
                app.stop()
        assert hashlib.sha256(args.binary.read_bytes()).hexdigest() == binary_hash
        report['functional_passed'] = True
        scores = [s['score']['total'] for s in report['screens']]
        report['mean'] = sum(scores) / len(scores)
        report['minimum'] = min(scores)
        report['passed'] = min(scores) >= report['target']
    finally:
        (root / 'report.json').write_text(json.dumps(report, indent=2))
        cards = ''.join(f'<article><h2>{html.escape(s["name"])} · {s["score"]["total"]}/10</h2>'
                        f'<img src="{html.escape(s["path"])}"><p>{html.escape(str(s["score"]["parts"]))}</p></article>'
                        for s in report['screens'])
        (root / 'review.html').write_text('<!doctype html><meta charset="utf-8"><title>Rinx chat theme review</title>'
            '<style>body{font:16px system-ui;margin:32px;background:#f1f3f5;color:#17212b}article{margin:32px 0}img{max-width:100%;max-height:850px;border:1px solid #bbb}</style>'
            '<h1>Rinx chat theme review</h1><p>Production chat components, fixed offline conversation. '
            'Whole native captures scored by the unchanged external pixel scorer. No physical-phone validation.</p>' + cards)
        print(json.dumps({'passed': report['passed'], 'functional_passed': report['functional_passed'],
                          'report': str(root / 'report.json')}), flush=True)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
