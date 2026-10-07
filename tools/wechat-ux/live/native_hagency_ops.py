#!/usr/bin/env python3
"""Development-only native session bootstrap checks against isolated Palpo accounts."""
import json, os, time, base64, hashlib
from pathlib import Path
from native_probe import NativeApp
from seed import checked
from native_hagency import fixture, ROOT

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
    f=fixture();alex=f['users']['alex'];bridge=f['users']['emma'];agent=f['users']['agent']
    assert all(u['user_id'].startswith('@robrix_ux_') for u in (alex,bridge,agent))
    if 'ops_owner' not in f['rooms']:
        room=checked(f['url'],'POST','createRoom',{'name':'Hagency Approval Test','preset':'private_chat','invite':[bridge['user_id'],agent['user_id']],
            'initial_state':[{'type':'m.room.encryption','state_key':'','content':{'algorithm':'m.megolm.v1.aes-sha2'}}]},alex['access_token'])['room_id']
        for member in (bridge,agent):checked(f['url'],'POST',f'join/{room}',{},member['access_token'])
        f['rooms']['ops_owner']=room;(ROOT/'fixture.json').write_text(json.dumps(f));os.chmod(ROOT/'fixture.json',0o600)
    owner_room=f['rooms']['ops_owner'];project=f['rooms']['hagency']
    (ROOT/'profile/ui-language.json').write_text('"en"')
    pin='sha256:'+base64.urlsafe_b64encode(hashlib.sha256(b'isolated-bootstrap-test-pin').digest()).decode().rstrip('=')
    app=NativeApp(ROOT,8299,size=(375,812));report={'passed':False,'checks':[]}
    def field(name,value):
        app.click_id(name);app.request('/k',c='A',cmd=1,wait=1);app.request('/k',c='Backspace',wait=1);app.request('/t',t=value,wait=1)
    def passed(name):report['checks'].append(name);print('PASS '+name,flush=True)
    def messages():return checked(f['url'],'GET',f'rooms/{owner_room}/messages?dir=b&limit=20',token=alex['access_token'])['chunk']
    try:
        app.start();report['run']=str(app.output);app.wait_text('All Chats',timeout=90)
        app.click_id('me_tab');app.click_id('settings');app.click_id('category_preferences_button');scroll_to(app,'agent_ops');app.click_id('agent_ops')
        app.wait_text('Development Agent Operations');app.capture('ops-dev-setup')
        for name,value in [('agent','worker'),('project',project),('owner_room',owner_room),('bridge',bridge['user_id']),('endpoint','http://example.org:8090'),('fingerprint',pin)]:field(name,value)
        app.click_id('connect');app.wait_text('Use an HTTP loopback address with an explicit port');passed('non_loopback_endpoint_refused_before_bootstrap')
        field('endpoint','http://127.0.0.1:8090');field('owner_room',project)
        app.click_id('connect');app.wait_text('Choose the separate owner approval room');passed('project_room_cannot_be_used_as_owner_room')
        field('owner_room',owner_room)
        before={e['event_id'] for e in messages()}
        app.click_id('connect');app.wait_text('Waiting for an encrypted session grant',timeout=20)
        deadline=time.monotonic()+45
        while time.monotonic()<deadline:
            sent=[e for e in messages() if e['event_id'] not in before and e['sender']==alex['user_id']]
            if sent:break
            time.sleep(.5)
        assert sent and all(e['type']=='m.room.encrypted' for e in sent), 'Bootstrap must be encrypted on the wire'
        assert all('client_public_jwk' not in json.dumps(e['content']) for e in sent)
        app.capture('ops-encrypted-bootstrap');passed('native_session_request_is_matrix_encrypted')
        app.click_id('close');app.wait_text('Enable agent workflow commands');app.click_id('agent_ops');app.wait_text('Development Agent Operations')
        assert not any('Waiting for an encrypted' in (w.get('t') or '') for w in app.snap());app.capture('ops-cancelled');passed('closing_panel_cancels_bootstrap_and_drops_session')
        app.click_id('close');report['passed']=True
    finally:
        if not report['passed'] and app.process and app.process.poll() is None:app.capture('ops-failure')
        app.stop()
        errors=[line for line in (app.output/'native.log').read_text(errors='replace').splitlines() if any(marker in line for marker in ['[E]','panicked at','Assertion failed:'])]
        report['native_error_count']=len(errors)
        if errors:report['passed']=False
        (ROOT/'native-hagency-ops.json').write_text(json.dumps(report,indent=2));print(json.dumps({'passed':report['passed'],'checks':len(report['checks']),'native_errors':len(errors)}),flush=True)
if __name__=='__main__':main()
