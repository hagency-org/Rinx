#!/usr/bin/env python3
"""Real native Hagency presentation/settings journeys using only isolated fixture accounts."""
import json, os, secrets, subprocess, time, uuid
from pathlib import Path
from native_probe import NativeApp
from seed import api, checked

ROOT = Path('lab/wechat-ux/evidence/live/hagency')

def fixture():
    path=ROOT/'fixture.json'; ROOT.mkdir(parents=True,exist_ok=True,mode=0o700)
    if not path.exists():
        original=json.loads(Path('lab/wechat-ux/evidence/live/moments/fixture.json').read_text())
        assert all(u['user_id'].startswith('@robrix_ux_') for u in original['users'].values())
        ssh_host=os.environ.get('ROBRIX_TEST_SSH')
        if not ssh_host:
            raise RuntimeError('Set ROBRIX_TEST_SSH to the isolated fixture server before provisioning')
        registration=json.loads(subprocess.check_output(['ssh','-o','BatchMode=yes',ssh_host,'cat ~/robrix-mobile-soak/credentials.json'],text=True))['registration_token']
        password=secrets.token_urlsafe(28)
        body={'username':'robrix_ux_'+secrets.token_hex(4)+'_reviewer','password':password,'initial_device_display_name':'Rinx Hagency fixture'}
        status,response=api(original['url'],'POST','register',body)
        for _ in range(5):
            if status!=401:break
            stages=next(f['stages'] for f in response['flows'] if set(f['stages']) <= {'m.login.registration_token','m.login.dummy'})
            stage=next(s for s in stages if s not in response.get('completed',[]));auth={'type':stage,'session':response['session']}
            if stage=='m.login.registration_token':auth['token']=registration
            status,response=api(original['url'],'POST','register',{**body,'auth':auth})
        assert status==200, 'Test agent registration failed'
        agent={**response,'password':password}; original['users']['agent']=agent
        checked(original['url'],'PUT',f"profile/{agent['user_id']}/displayname",{'displayname':'Fixture Reviewer'},agent['access_token'])
        alex=original['users']['alex']
        room=checked(original['url'],'POST','createRoom',{'name':'Hagency Test Lab','preset':'private_chat','invite':[agent['user_id']]},alex['access_token'])['room_id']
        checked(original['url'],'POST',f'join/{room}',{},agent['access_token']);original['rooms']['hagency']=room
        fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'w') as f:json.dump(original,f)
    return json.loads(path.read_text())

def scroll_to(app, widget):
    for _ in range(16):
        height = app.request('/s')['w'][0]['sz'][1]
        if any(w['i']==widget and 150 < w['r'][1] and w['r'][1]+w['r'][3]<height-30 for w in app.snap()):
            return
        app.request('/m',k='scroll',x=220,y=440,dy=350,wait=1)
        time.sleep(.3)
    raise AssertionError('Setting not reachable: '+widget)


def main():
    os.environ.update(MAKEPAD_HIDE_WINDOWS='1',MAKEPAD_NO_FOCUS='1');os.environ.pop('MAKEPAD_FOCUS',None)
    f=fixture();room=f['rooms']['hagency'];agent=f['users']['agent'];assert agent['user_id'].startswith('@robrix_ux_')
    profile=ROOT/'profile';profile.mkdir(exist_ok=True);(profile/'ui-language.json').write_text('"en"')
    for state in profile.rglob('latest_app_state.json'):
        saved=json.loads(state.read_text());saved['app_prefs']['agent_chat_enabled']=False;state.write_text(json.dumps(saved))
    report={'passed':False,'checks':[],'runs':[]};apps=[]
    def passed(name):report['checks'].append(name);print('PASS '+name,flush=True)
    def send(content):return checked(f['url'],'PUT',f'rooms/{room}/send/m.room.message/{uuid.uuid4().hex}',content,agent['access_token'])['event_id']
    def start(size=(375,812)):
        app=NativeApp(ROOT,8299,size=size);apps.append(app);app.start();report['runs'].append(str(app.output));app.wait_text('All Chats',timeout=90);return app
    def stop(app):app.stop();apps.remove(app)
    def back(app):app.click(24,54)
    def settings(app):app.click_id('me_tab');app.click_id('settings');app.click_id('category_preferences_button');scroll_to(app,'agent_ops')
    def texts(app):return [w['text'] for w in app.ocr()]
    def scroll(app,dy=500):app.request('/m',k='scroll',x=220,y=440,dy=dy,wait=1);time.sleep(.4)
    try:
        body='\n'.join(f'Agent analysis line {i}: 中文 👨‍👩‍👧‍👦' for i in range(1,15))
        send({'msgtype':'m.text','body':body})
        app=start();app.wait_text('Hagency Test Lab',timeout=60,pixels=True);app.click_text('Hagency Test Lab');app.wait_text('Show more',pixels=True);app.capture('reply-folded')
        assert not any('line 14' in t for t in texts(app)), 'Folded tail is visible'
        app.click_text('Show more');time.sleep(.5)
        for _ in range(8):
            if any('line 14' in t for t in texts(app)):break
            scroll(app,220)
        app.wait_text('line 14',pixels=True);app.capture('reply-expanded')
        # The fold button may be below the viewport after expansion; scroll to the tail.
        for _ in range(5):
            if any(t=='Show less' for t in texts(app)):break
            scroll(app,220)
        app.click_text('Show less');app.wait_text('Show more',pixels=True);passed('long_agent_reply_expands_and_collapses_without_changing_body')
        scroll(app,10000)
        live=send({'msgtype':'m.text','body':'Streaming 中文 initial','org.matrix.msc4357.live':{}})
        app.wait_text('Receiving response',timeout=25,pixels=True);app.capture('stream-initial')
        updated='Streaming 中文 update 👨‍👩‍👧‍👦 intact'
        send({'msgtype':'m.text','body':'* '+updated,'m.new_content':{'msgtype':'m.text','body':updated,'org.matrix.msc4357.live':{}},'m.relates_to':{'rel_type':'m.replace','event_id':live}})
        app.wait_text('intact',timeout=25,pixels=True);app.capture('stream-update')
        final='Finished 中文 reply'
        send({'msgtype':'m.text','body':'* '+final,'m.new_content':{'msgtype':'m.text','body':final,'format':'org.matrix.custom.html','formatted_body':'<strong>Finished</strong> 中文 reply'},'m.relates_to':{'rel_type':'m.replace','event_id':live}})
        app.wait_text('Finished',timeout=25,pixels=True);time.sleep(.5)
        assert not any('Receiving response' in t for t in texts(app));app.capture('stream-final');passed('live_edits_reveal_unicode_and_finalize_rich_text')
        history=checked(f['url'],'GET',f'rooms/{room}/messages?dir=b&limit=30',token=agent['access_token'])
        assert any(e.get('content',{}).get('body')==body for e in history['chunk']);passed('matrix_message_body_is_preserved')
        back(app);settings(app);app.wait_text('Enable agent workflow commands');app.capture('settings-en')
        app.click_id('agent_chat_toggle');time.sleep(.3)
        app.click_id('agent_ops');app.wait_text('Agent Operations awaits a released backend contract');app.capture('ops-release-gate')
        assert not any(t=='Connect' for t in texts(app)), 'Production connection form must be gated'
        app.click_id('close');app.request('/m',k='scroll',x=220,y=440,dy=-10000,wait=1);app.click_id('language_dropdown');app.request('/k',c='ArrowDown',wait=1);scroll_to(app,'agent_ops')
        app.wait_text('启用智能体工作流命令',pixels=True);app.capture('settings-zh')
        app.click_id('agent_ops');app.wait_text('智能体操作正在等待后端协议正式发布',pixels=True);app.capture('ops-release-gate-zh')
        app.click_id('close');app.request('/m',k='scroll',x=220,y=440,dy=-10000,wait=1);app.click_id('language_dropdown');app.request('/k',c='ArrowUp',wait=1);passed('mobile_hagency_settings_and_release_gate_translate_in_place')
        state_path=next(profile.rglob('latest_app_state.json'))
        stop(app)
        assert json.loads(state_path.read_text())['app_prefs']['agent_chat_enabled'], 'Mobile toggle did not persist'
        passed('mobile_workflow_preference_persists')
        app=start(size=(1050,800));app.click_id('profile_icon');app.click_id('category_preferences_button')
        # Desktop preference page may require scrolling.
        for _ in range(6):
            if any(w['i']=='agent_ops' for w in app.snap()):break
            app.request('/m',k='scroll',x=700,y=600,dy=400,wait=1);time.sleep(.3)
        app.click_id('agent_ops');app.wait_text('Agent Operations awaits a released backend contract');app.capture('desktop-ops-release-gate');app.click_id('close')
        passed('desktop_hagency_settings_share_native_panel')
        app.click_id('agent_chat_toggle');stop(app)
        assert not json.loads(state_path.read_text())['app_prefs']['agent_chat_enabled'], 'Desktop toggle did not persist'
        passed('desktop_uses_the_same_workflow_preference');report['passed']=True
    finally:
        for app in apps:
            if not report['passed']:
                app.capture('failure');(app.output/'failure-snap.json').write_text(json.dumps(app.snap(),ensure_ascii=False))
            app.stop()
        errors=[]
        for run in report['runs']:
            errors.extend(line for line in (Path(run)/'native.log').read_text(errors='replace').splitlines() if any(marker in line for marker in ['[E]','panicked at','Assertion failed:']))
        report['native_error_count']=len(errors)
        if errors:report['passed']=False
        (ROOT/'native-hagency.json').write_text(json.dumps(report,indent=2));print(json.dumps({'passed':report['passed'],'checks':len(report['checks']),'native_errors':len(errors)}),flush=True)
if __name__=='__main__':main()
