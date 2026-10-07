#!/usr/bin/env python3
"""English/Chinese native journeys; only isolated Moments fixture accounts."""
import json, os, shutil, time, uuid
from pathlib import Path
from native_probe import NativeApp


def main():
    os.environ.update(MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1')
    os.environ.pop('MAKEPAD_FOCUS', None)
    root = Path('lab/wechat-ux/evidence/live/moments')
    evidence = Path('lab/wechat-ux/evidence/live/i18n')
    evidence.mkdir(parents=True, exist_ok=True, mode=0o700)
    fixture = json.loads((root/'fixture.json').read_text())
    assert all(u['user_id'].startswith('@robrix_ux_') for u in fixture['users'].values())
    login_root = evidence/'login'
    login_root.mkdir(exist_ok=True, mode=0o700)
    shutil.copyfile(root/'fixture.json', login_root/'fixture.json')
    os.chmod(login_root/'fixture.json', 0o600)
    apps=[]
    report={'passed':False,'checks':[],'runs':[]}
    def passed(name): report['checks'].append(name); print('PASS '+name,flush=True)
    def start(path=root, size=(375,812), auto_login=True, expected='All Chats', port=8299):
        app=NativeApp(path,port,size=size,auto_login=auto_login);apps.append(app);app.start()
        report['runs'].append(str(app.output));app.wait_text(expected,timeout=90);return app
    def stop(app): app.stop();apps.remove(app)
    def field(app, name, value):
        app.click_id(name);app.request('/k',c='A',cmd=1,wait=1);app.request('/k',c='Backspace',wait=1)
        if value:app.request('/t',t=value,wait=1)
    def texts(app):return [w.get('t') or '' for w in app.snap()]
    def idle(app):
        deadline=time.monotonic()+60
        while time.monotonic()<deadline:
            if not any(t.startswith(('Updating Moments','Encrypting and publishing','正在更新朋友圈','正在加密并发表')) for t in texts(app)):
                time.sleep(.4);return
            time.sleep(.3)
        raise AssertionError('Operation did not finish')
    def back(app):app.click(24,54);time.sleep(.4)
    def language_page(app):
        app.click_id('me_tab');app.click_id('settings');app.click_id('category_preferences_button')
    def feed(app):app.click_id('discover_tab');app.click_id('discover_moments');idle(app)
    def locale(path,value):
        profile=path/'profile';profile.mkdir(exist_ok=True)
        # Test-only initial preference; subsequent switches use native controls.
        (profile/'ui-language.json').write_text(json.dumps(value))
    try:
        locale(login_root,'en')
        login=start(login_root,auto_login=False,expected='Sign in to Rinx',port=8297)
        field(login,'user_id_input','alice:matrix.org')
        login.click_id('login_language_zh');login.wait_text('登录 Rinx',pixels=True)
        login.wait_text('alice:matrix.org',pixels=True);login.capture('login-zh')
        assert json.loads((login_root/'profile/ui-language.json').read_text())=='zh-CN'
        login.click_id('login_language_en');login.wait_text('Sign in to Rinx',pixels=True)
        login.wait_text('alice:matrix.org',pixels=True);login.capture('login-en');stop(login)
        passed('login_language_switch_preserves_input')

        locale(root,'en')
        app=start();feed(app);app.click_id('moments_compose');idle(app)
        draft='Chats Settings Moments '+uuid.uuid4().hex[:6]+' 中文 {count}'
        field(app,'moments_body',draft);back(app);back(app)
        language_page(app);app.click_id('language_dropdown');app.request('/k',c='ArrowDown',wait=1);app.wait_text('语言',pixels=True)
        app.capture('language-zh');app.wait_text('应用设置',pixels=True);app.capture('preferences-zh')
        app.click_id('category_account_button');app.wait_text('账号设置',pixels=True);app.capture('settings-zh');app.click_id('close_button')
        app.wait_text('我的朋友圈',pixels=True);app.capture('me-zh')
        app.click_id('contacts_tab');app.wait_text('通讯录',pixels=True);app.capture('contacts-zh')
        app.click_id('chats_tab');app.wait_text('全部聊天',pixels=True);app.wait_text('文件传输助手',pixels=True);app.capture('chats-zh')
        passed('four_tabs_and_mobile_settings_in_chinese')
        feed(app);app.wait_text('朋友圈',pixels=True);app.capture('moments-zh')
        app.click_id('moments_compose');idle(app);app.wait_text('朋友圈可见范围',pixels=True)
        app.wait_text('Chats Settings Moments',pixels=True)
        drafts=list((root/'profile').rglob('moments-composer.json'))
        assert any(json.loads(path.read_text()).get('body')==draft for path in drafts)
        app.capture('composer-zh');passed('language_rebake_preserves_moments_draft_exactly')
        app.click_id('moments_publish');idle(app)
        deadline=time.monotonic()+60
        while time.monotonic()<deadline:
            if any(w['i']=='post_body' and w.get('t')==draft for w in app.snap()):break
            time.sleep(.3)
        else:raise AssertionError('Published body changed')
        app.capture('published-zh');passed('chinese_ui_publishes_original_mixed_language_text')
        viewer_root=root/'viewer';locale(viewer_root,'zh-CN')
        viewer=start(viewer_root,expected='全部聊天',port=8298);feed(viewer)
        deadline=time.monotonic()+60
        while time.monotonic()<deadline:
            if any(w['i']=='post_body' and w.get('t')==draft for w in viewer.snap()):break
            time.sleep(.3)
        else:raise AssertionError('Recipient body changed or missing')
        viewer.capture('recipient-zh');stop(viewer);locale(viewer_root,'en')
        passed('recipient_decrypts_unchanged_message_with_chinese_ui')
        stop(app)
        app=start(expected='全部聊天');app.wait_text('全部聊天',pixels=True);app.capture('restart-zh')
        assert json.loads((root/'profile/ui-language.json').read_text())=='zh-CN'
        passed('chinese_preference_survives_restart')
        stop(app)
        app=start(size=(1050,800),expected='全部聊天')
        app.click_id('profile_icon');app.click_id('category_preferences_button');app.wait_text('应用设置',pixels=True)
        app.capture('desktop-preferences-zh');app.click_id('language_dropdown');app.click_text('English')
        app.wait_text('App Settings',pixels=True);app.capture('desktop-preferences-en')
        assert json.loads((root/'profile/ui-language.json').read_text())=='en'
        passed('desktop_language_selection_restores_english')
        stop(app)
        app=start();app.wait_text('All Chats',pixels=True);app.capture('restart-en')
        passed('english_preference_survives_restart')
        report['passed']=True
    finally:
        for app in apps:app.stop()
        errors=[]
        for run in report['runs']:
            for line in (Path(run)/'native.log').read_text(errors='replace').splitlines():
                if any(marker in line for marker in ['[E]','panicked at','Assertion failed:']):errors.append(line)
        report['native_error_count']=len(errors)
        if errors:report['passed']=False
        (evidence/'native-i18n.json').write_text(json.dumps(report,indent=2))
        print(json.dumps({'passed':report['passed'],'checks':len(report['checks']),'native_errors':len(errors)}),flush=True)


if __name__=='__main__':main()
