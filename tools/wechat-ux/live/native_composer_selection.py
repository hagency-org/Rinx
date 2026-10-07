#!/usr/bin/env python3
"""Check native composer selection pixels and replacement/undo, offline.

Build: cargo build --offline --profile fast --locked --example chat_theme
The fixtures use production desktop/mobile composer widgets on macOS, not
physical mobile devices. No Matrix account or message send is involved.
"""
import argparse
import hashlib
import json
from pathlib import Path
import time
import uuid
from PIL import Image

from native_chat_theme import launch
from native_theme_presets import PALETTES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/fast/examples/chat_theme'))
    parser.add_argument('--presets', action='store_true', help='Also exercise every bundled palette')
    args = parser.parse_args()
    root = Path('target/composer-selection-validation') / uuid.uuid4().hex
    root.mkdir(parents=True)
    report = {'passed': False, 'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'runs': []}
    draft = 'Selection stays readable 中文 123'
    near = lambda p, q: max(abs(i - j) for i, j in zip(p, q)) <= 8
    try:
        for narrow in (False, True):
            layout = 'narrow' if narrow else 'desktop'
            app = launch(root / layout, args.binary, narrow)
            try:
                palettes = [(0, 'rinx')]
                if args.presets:
                    palettes += [(i, name) for i, (_, name) in enumerate(PALETTES, 1)]
                for preset, name in palettes:
                    app.request('/event', data=f'theme:preset:{preset}', wait=1)
                    for mode in ('light', 'dark'):
                        app.request('/event', data='theme:' + mode, wait=1)
                        app.click_id('text_input')
                        app.request('/k', c='A', cmd=1, wait=1)
                        app.request('/t', t=draft, wait=1)
                        app.request('/event', data='theme:inspect', wait=1)
                        palette = json.loads((app.root / 'profile/chat-theme.json').read_text())
                        ink = tuple(bytes.fromhex(palette['color.content.primary'][1:]))
                        widget = next(w for w in app.snap() if w['i'] == 'text_input')
                        x, y, width, height = widget['r']
                        label = f'{name}-{mode}'
                        # Do not mistake an initial clear-only GPU frame for
                        # the rendered baseline just because the tree is ready.
                        for _ in range(8):
                            before = Image.open(app.capture(label + '-unselected')).convert('RGB')
                            scale = before.width / (430 if narrow else 1000)
                            crop = tuple(round(v * scale) for v in (x, y, x + width, y + height))
                            original = list(before.crop(crop).getdata())
                            glyphs = [i for i, pixel in enumerate(original) if near(pixel, ink)]
                            if len(glyphs) > 100:
                                break
                            time.sleep(.2)
                        assert len(glyphs) > 100, 'Unselected composer text did not render'
                        app.request('/k', c='A', cmd=1, wait=1)
                        after = Image.open(app.capture(label + '-selected')).convert('RGB')
                        selected = list(after.crop(crop).getdata())
                        retained = sum(near(selected[i], ink) for i in glyphs) / len(glyphs)
                        assert retained >= .95, ('Selection hides composer text', label, retained)
                        changed = sum(not near(p, q) for p, q in zip(original, selected))
                        assert changed > 100, ('Missing visible selection highlight', label)
                        app.request('/t', t='Replacement', wait=1)
                        assert any(w['i'] == 'text_input' and w.get('t') == 'Replacement' for w in app.snap())
                        app.request('/k', c='Z', cmd=1, wait=1)
                        assert any(w['i'] == 'text_input' and w.get('t') == draft for w in app.snap())
                        result = {'layout': layout, 'palette': name, 'appearance': mode,
                                  'retained_glyphs': retained, 'changed_pixels': changed,
                                  'select_all_replace_and_undo': 'passed'}
                        report['runs'].append(result)
                        print(json.dumps(result), flush=True)
            finally:
                app.stop()
        report['passed'] = True
    finally:
        (root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'report': str(root / 'report.json'), 'passed': report['passed'],
                          'runs': len(report['runs'])}), flush=True)


if __name__ == '__main__':
    main()
