#!/usr/bin/env python3
"""Exercise real shared palette/mode controls, retained state, restart and ownership.

Build: cargo build --offline --profile fast --locked --example theme_mvp
Desktop and narrow desktop instrumentation; not a physical-phone test.
"""
import argparse
import hashlib
import json
from pathlib import Path
import uuid

from native_theme import launch, inspect, type_at
from native_theme_packages import settle

PALETTES = ([('octoscode', name) for name in ('codex', 'claude', 'slate', 'solarized')]
            + [('rinx', 'sage'), ('rinx', 'rose')]
            + [('community', name) for name in ('catppuccin', 'nord', 'dracula', 'gruvbox', 'tokyo-night')])


def step(app, direction, control='theme_preset'):
    app.click_id(control)
    app.request('/k', c=direction, wait=1)
    settle(app)
    return inspect(app)


def argb(token):
    return 0xff000000 | sum(round(c * 255) << shift for c, shift in
                            zip(token['$value']['components'], (16, 8, 0)))


def check_palette(after, package, mode):
    assert after['preferences']['package']['id'] == package['id']
    assert after['selection']['appearance'] == mode
    colors = package['variants'][mode]['color']
    assert after['ink'] == after['l0_ink'] == argb(colors['content']['primary'])
    assert after['accent'] == argb(colors['action']['primary'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--first-preset', type=int, default=1,
                        help='Start detailed captures at this one-based preset index')
    args = parser.parse_args()
    assert 1 <= args.first_preset <= len(PALETTES)
    root = Path('target/theme-preset-validation') / uuid.uuid4().hex
    root.mkdir(parents=True)
    report = {'passed': False, 'screens': [], 'first_preset': args.first_preset, 'binary_sha256': hashlib.sha256(
        Path('target/fast/examples/theme_mvp').read_bytes()).hexdigest()}
    packages = [json.loads(Path(f'resources/themes/{family}/{name}.octotheme').read_text())
                for family, name in PALETTES]
    try:
        for narrow in (False, True):
            run = root / ('narrow' if narrow else 'desktop')
            app = launch(run, narrow=narrow)
            try:
                settle(app)
                drafts = [w for w in app.snap() if w['i'] == 'draft']
                # The fixture has additional reference panels below the settings.
                # At phone width the first draft needs scrolling into view.
                for _ in range(4):
                    if drafts:
                        break
                    scroll = next(w['r'] for w in app.snap() if w['i'] == 'scroll')
                    app.request('/m', k='scroll', x=scroll[0] + 100,
                                y=scroll[1] + scroll[3] / 2, dy=100, precise=1, wait=1)
                    settle(app)
                    drafts = [w for w in app.snap() if w['i'] == 'draft']
                assert drafts, 'Reference draft must be reachable by scrolling'
                type_at(app, drafts[0], 'Keep my unsaved 中英 draft')
                before = inspect(app)
                # Verify the larger menu remains visible and pointer-selectable.
                app.click_id('theme_preset')
                settle(app)
                menu = [w for w in app.snap() if w['ty'] == 'PopupMenuItem']
                assert len(menu) == len(packages) + 1, menu
                width, height = app.request('/s')['w'][0]['sz']
                for item in menu:
                    x, y, w, h = item['r']
                    assert 0 <= x < x + w <= width and 0 <= y < y + h <= height, item
                report['screens'].append(str(app.capture('all-palettes-menu')))
                x, y, w, h = max(menu, key=lambda item: item['r'][1])['r']
                app.click(x + w / 2, y + h / 2)
                settle(app)
                check_palette(inspect(app), packages[-1], 'light')
                app.click_id('theme_preset')
                settle(app)
                menu = [w for w in app.snap() if w['ty'] == 'PopupMenuItem']
                x, y, w, h = min(menu, key=lambda item: item['r'][1])['r']
                app.click(x + w / 2, y + h / 2)
                settle(app)
                assert inspect(app)['preferences']['package'] is None
                for _ in range(args.first_preset - 1):
                    step(app, 'ArrowDown')
                for package in packages[args.first_preset - 1:]:
                    after = step(app, 'ArrowDown')
                    for mode in ('light', 'dark'):
                        if mode == 'dark':
                            after = step(app, 'ArrowDown', 'mode')
                        check_palette(after, package, mode)
                        assert not after['preferences']['follow_system']
                        assert [(f['uid'], f['text']) for f in after['fields']] == [(f['uid'], f['text']) for f in before['fields']]
                        for key in ('script_heap', 'l0_heap', 'requests', 'timers'):
                            assert after[key] == before[key], (package['id'], mode, key)
                        visible = app.snap()
                        assert any(w['i'] == 'mode' for w in visible)
                        assert not any(w['i'] == 'accent' for w in visible)
                        assert {'Appearance mode', 'Color palette', 'Chat preview', 'Incoming message', 'Your message'} <= {w.get('t') for w in visible}
                        width = app.request('/s')['w'][0]['sz'][0]
                        for control in ('mode', 'theme_preset', 'chat_preview'):
                            widget = next(w for w in visible if w['i'] == control)
                            assert 0 <= widget['r'][0] < widget['r'][0] + widget['r'][2] <= width, widget
                        report['screens'].append(str(app.capture(package['id'] + '-' + mode)))
                        saved = json.loads((app.root / 'profile/theme-state.json').read_text())
                        assert saved['current']['package']['id'] == package['id']
                        assert saved['current']['selection']['appearance'] == mode
                        assert not (app.root / 'profile/theme-rollback.json').exists()
                    print(json.dumps({'palette': package['name'], 'layout': 'narrow' if narrow else 'desktop',
                                      'light_and_dark': 'passed'}), flush=True)
                    # Preserve Dark while changing the palette, then restore Light.
                    if package != packages[-1]:
                        following = packages[packages.index(package) + 1]
                        check_palette(step(app, 'ArrowDown'), following, 'dark')
                        check_palette(step(app, 'ArrowUp'), package, 'dark')
                    step(app, 'ArrowUp', 'mode')
                step(app, 'ArrowDown', 'mode')
                system = step(app, 'ArrowDown', 'mode')
                assert system['preferences']['follow_system']
                assert step(app, 'ArrowUp')['preferences']['follow_system']
                assert step(app, 'ArrowDown')['preferences']['follow_system']
                assert '[E]' not in (app.output / 'native.log').read_text(), 'Native script error'
            finally:
                app.stop()
            restarted = launch(run, narrow=narrow)
            try:
                settle(restarted)
                state = inspect(restarted)
                assert state['preferences']['package']['id'] == packages[-1]['id']
                assert state['preferences']['follow_system']
                assert any(w['i'] == 'mode' and w.get('t') == 'System' for w in restarted.snap())
                for _ in packages:
                    restored = step(restarted, 'ArrowUp')
                assert restored['preferences']['package'] is None
                assert restored['preferences']['follow_system']
                assert {'mode', 'accent'} <= {w['i'] for w in restarted.snap()}
                restarted.capture('restored-rinx')
            finally:
                restarted.stop()
        # Seed the precise old persisted format, as installed before light variants.
        for package in (packages[:4] if args.first_preset == 1 else []):
            run = root / ('upgrade-' + package['id'])
            profile = run / 'profile'
            profile.mkdir(parents=True)
            legacy = dict(package, tokens=package['variants']['dark'], variants={'light': {}, 'dark': {}})
            preferences = {'selection': {'appearance': 'dark', 'accent': 'teal'}, 'package': legacy, 'follow_system': False}
            (profile / 'theme-state.json').write_text(json.dumps({'current': preferences, 'previous': preferences}))
            app = launch(run)
            try:
                settle(app)
                check_palette(inspect(app), package, 'dark')
                assert any(w['i'] == 'theme_preset' and w.get('t') == package['name'] for w in app.snap())
                check_palette(step(app, 'ArrowUp', 'mode'), package, 'light')
            finally:
                app.stop()
        app = launch(root / 'hosted', hosted=True)
        try:
            assert inspect(app)['selection'] is None
            assert not any(w['i'] in ('mode', 'theme_preset', 'chat_preview') for w in app.snap())
        finally:
            app.stop()
        report['passed'] = True
        report['checks'] = ['selected_palettes_in_light_and_dark_on_desktop_and_narrow',
            'all_palette_menu_items_fit_viewport_and_pointer_selection_works',
            'native_and_embedded_colors_match', 'drafts_and_runtime_state_retained',
            'palette_preserves_light_dark_and_system_mode', 'system_and_palette_persist_across_restart',
            'labeled_controls_and_chat_preview_fit_viewport',
            'default_restores_accent_control', 'hosted_theme_authority_preserved']
        if args.first_preset == 1:
            report['checks'].append('legacy_dark_only_presets_upgrade')
    finally:
        (root / 'report.json').write_text(json.dumps(report, indent=2))
        print(json.dumps({'report': str(root / 'report.json'), **report}))


if __name__ == '__main__':
    main()
