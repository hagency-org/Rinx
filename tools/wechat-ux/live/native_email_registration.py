#!/usr/bin/env python3
"""Native desktop/phone-width email verification against isolated Palpo.
Run Palpo's tests/registration_email_otp.py --serve first; pass its state.json.
Only the local recording email transport is read. Never reads a human mailbox.
"""
import argparse, json, secrets, socket, time, urllib.request
from pathlib import Path
from native_server_history import App
from native_server_catalog import fill_field, widget


def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--state',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    args=p.parse_args();state=json.loads(args.state.read_text());report={'passed':False,'checks':[]}
    assert state['server'].startswith('http://127.0.0.1:')
    assert state['mail_capture'].startswith('http://127.0.0.1:')
    for name,size in [('desktop',(1000,820)),('phone',(390,844))]:
        with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
        app=App(args.output/name,port=port,size=size,auto_login=False)
        try:
            app.launch(args.binary);app.wait_text('Mozilla community');time.sleep(1)
            app.fill(state['server']);app.continue_server();app.click_id('register_option_button');app.wait_text('Verify your email')
            app.capture('email-entry-en')
            address=name+secrets.token_hex(4)+'@example.org'
            fill_field(app,'registration_email',address);app.click_id('send_email_code');app.wait_text('Code sent.')
            for button in ['send_email_code','verify_email_code','email_back']:
                w=widget(app,button);x,y,width,height=w['r'];assert height>=44 and x>=0 and x+width<=size[0] and y>=0 and y+height<=size[1],w
            app.capture('email-code-en')
            with urllib.request.urlopen(state['mail_capture']) as response:messages=json.load(response)
            import re
            message=next(m for m in reversed(messages) if m['to']==[address]);code=re.search(r'\b[0-9]{6}\b',message['text']).group()
            fill_field(app,'registration_email_code','000000' if code!='000000' else '000001');app.click_id('verify_email_code');app.wait_text('Code incorrect or expired.')
            fill_field(app,'registration_email_code',code);app.click_id('verify_email_code');app.wait_text('Email verified')
            app.capture('email-verified-en')
            # Changing the email must discard the proof and password.
            fill_field(app,'registration_password','temporary-password');app.click_id('change_registration_email');app.wait_text('Verify your email')
            assert not any(w['i']=='registration_password' for w in app.snap())
            app.click_id('login_language_zh');app.wait_text('验证邮箱');app.capture('email-entry-zh');app.click_id('login_language_en')
            # New recipient avoids the deliberate recipient resend cooldown.
            address=name+secrets.token_hex(4)+'@example.org';fill_field(app,'registration_email',address);app.click_id('send_email_code');app.wait_text('Code sent.')
            with urllib.request.urlopen(state['mail_capture']) as response:messages=json.load(response)
            code=re.search(r'\b[0-9]{6}\b',next(m for m in reversed(messages) if m['to']==[address])['text']).group()
            fill_field(app,'registration_email_code',code);app.click_id('verify_email_code');app.wait_text('Email verified')
            fill_field(app,'registration_username',name+secrets.token_hex(4));fill_field(app,'registration_password','native-otp-test-password');fill_field(app,'registration_token','invite')
            app.capture('ready-to-register');app.click_id('submit_registration_button')
            # Successful registration leaves the authentication UI for the app.
            deadline=time.monotonic()+45
            while time.monotonic()<deadline:
                if not any(w['i']=='submit_registration_button' for w in app.snap()):break
                time.sleep(.2)
            else:raise AssertionError('Registration did not leave login: '+str([(w["i"],w.get("t")) for w in app.snap()]))
            app.capture('registered')
            report['checks'].append(name+': native send, error recovery, verification, change-email invalidation, Chinese copy, invitation and account creation')
        finally:app.stop()
    report['passed']=True;(args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))

if __name__=='__main__':main()
