#!/usr/bin/env python3
"""Shared settings and theme reload regression in an isolated native window.

Build: cargo build --profile fast --features palpo-instrument --example settings_parity
No Matrix login, account changes, messages, or live human window input.
"""
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import uuid
from native_probe import NativeApp


def launch(root, narrow):
    profile = root / 'profile'
    profile.mkdir(parents=True, mode=0o700)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    app = NativeApp(root, port, auto_login=False)
    app.output.mkdir(parents=True)
    app.log = (app.output / 'native.log').open('w')
    app.process = subprocess.Popen(
        ['target/fast/examples/settings_parity'] + (['--narrow'] if narrow else []),
        env=dict(os.environ, MAKEPAD_REMOTE=str(port), MAKEPAD_HIDE_WINDOWS='1',
                 MAKEPAD_NO_FOCUS='1', RINX_DATA_DIR=str(profile), RUST_BACKTRACE='1'),
        stdout=app.log, stderr=subprocess.STDOUT)
    for _ in range(120):
        if app.process.poll() is not None:
            raise RuntimeError('Fixture exited before startup')
        try:
            if app.request('/s')['w']:
                app.wait_text('Settings')
                time.sleep(1)
                return app
        except OSError:
            pass
        time.sleep(.25)
    raise RuntimeError('Fixture did not start')


def inspect(app):
    app.request('/event', data='inspect', wait=1)
    value = json.loads((app.root / 'profile/inspection.json').read_text())
    assert value['buttons'][0] == value['buttons'][1], value['buttons']
    assert value['buttons'][0]['text'] == ''
    assert value['buttons'][0]['size'] == 'Vec2d { x: 40.0, y: 40.0 }'
    assert value['buttons'][0]['icon'] == 'Vec2f { x: 22.0, y: 22.0 }'
    assert value['drafts'] == ['Unsent 中文 draft'] * 2
    return value


def select_with_key(app, widget, key):
    app.click_id(widget)
    app.request('/k', c=key, wait=1)
    time.sleep(.8)


def run(root, narrow):
    app = launch(root, narrow)
    try:
        # Instantiate the normal responsive home tree before settings reloads.
        app.request('/event', data='production:init', wait=1)
        time.sleep(.7)
        app.request('/event', data='production:hide', wait=1)
        time.sleep(.7)
        categories = {}
        for category, title, controls in [
            ('account', 'Account Settings', ['display_name_input','upload_avatar_button','delete_avatar_button','manage_account_button','logout_button']),
            ('preferences', 'App appearance', ['mode','accent','customize','language_dropdown','view_mode_dropdown','ui_zoom_input','agent_access']),
            ('privacy', 'Privacy Settings', ['moments_sharing_toggle','blocked_users_list','assistant_rooms_list']),
            ('about', 'About Rinx', ['privacy_policy_button','homepage_button','source_button','issues_button','clear_cache_button']),
        ]:
            app.click_id('category_'+category+'_button')
            app.wait_text(title)
            all_widgets = app.request('/snap', all=1)['s']
            ids = {w['i'] for w in all_widgets}
            assert set(controls) <= ids, (category, set(controls)-ids)
            categories[category] = controls
            app.capture(category)
        app.click_id('category_preferences_button')
        app.wait_text('App appearance')
        initial = inspect(app)
        app.request('/event', data='encrypt', wait=1)
        encrypted = inspect(app)
        assert initial['buttons'][0]['svg'] != encrypted['buttons'][0]['svg']
        for stage in ['embedded', 'window-open', 'window-closed']:
            if stage == 'window-open':
                app.request('/event', data='miniapps:open', wait=1)
            if stage == 'window-closed':
                app.request('/event', data='miniapps:close', wait=1)
            for mode, key in [('dark','ArrowDown'),('light','ArrowUp')]:
                app.request('/event', data='gc', wait=1)
                select_with_key(app, 'mode', key)
                after = inspect(app)
                assert after['selection']['appearance'] == mode, after
                assert after['isolate'] == initial['isolate']
                assert after['buttons'][0]['svg'] == encrypted['buttons'][0]['svg']
                app.wait_text('Mini app keeps its own language')
                app.wait_text('App appearance')
                app.capture(stage+'-'+mode)
        # System remains offered after both a theme reload and language refresh.
        select_with_key(app, 'mode', 'ArrowDown')
        select_with_key(app, 'mode', 'ArrowDown')
        assert inspect(app)['follow_system'] and inspect(app)['mode_index'] == 2
        select_with_key(app, 'mode', 'ArrowUp')
        select_with_key(app, 'mode', 'ArrowUp')
        select_with_key(app, 'accent', 'ArrowDown')
        assert inspect(app)['selection']['accent'] == 'violet'
        select_with_key(app, 'language_dropdown', 'ArrowDown')
        assert json.loads((root / 'profile/ui-language.json').read_text()) == 'zh-CN'
        app.wait_text('Mini app keeps its own language')
        app.capture('chinese-violet')
        select_with_key(app, 'mode', 'ArrowDown')
        assert inspect(app)['selection']['appearance'] == 'dark'
        select_with_key(app, 'language_dropdown', 'ArrowUp')
        app.click_id('customize')
        app.wait_text('Customize appearance')
        app.click_id('preview_light')
        app.wait_text('Preview active')
        app.click_id('theme_cancel')
        app.wait_text('Preview cancelled')
        app.capture('theme-editor')
        app.click_id('studio_close')
        app.wait_text('App appearance')
        final = inspect(app)
        assert final['selection']['appearance'] == 'dark'
        assert final['buttons'][0]['svg'] == encrypted['buttons'][0]['svg']
        log = (app.output / 'native.log').read_text()
        assert not any(x in log for x in ['panicked at','use-after-free','[E]','Splash script errors']), log[-3000:]
        return {'narrow':narrow, 'categories':categories,'theme_changes_with_isolate':True,
                'drafts_and_encryption_icon_preserved':True,'system_and_language_switch':True,
                'theme_preview_cancel':True,'evidence':str(root)}
    finally:
        (root / 'trace.json').write_text(json.dumps(app.trace,indent=2))
        app.stop()


def main():
    root = (Path('target/settings-parity-validation') / uuid.uuid4().hex).resolve()
    root.mkdir(parents=True)
    report = {'passed':False, 'runs':[], 'binary_sha256':hashlib.sha256(Path('target/fast/examples/settings_parity').read_bytes()).hexdigest()}
    try:
        for narrow in [False, True]:
            report['runs'].append(run(root / ('mobile' if narrow else 'desktop'), narrow))
        assert report['runs'][0]['categories'] == report['runs'][1]['categories']
        report['passed'] = True
    finally:
        (root / 'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({'evidence':str(root),**report}),flush=True)


if __name__ == '__main__':
    main()
