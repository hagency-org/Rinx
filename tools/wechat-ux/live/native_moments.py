#!/usr/bin/env python3
"""Real native Moments journeys. Only isolated @robrix_ux_ fixture profiles."""
import json, os, time, uuid
from pathlib import Path
from native_probe import NativeApp
from seed import checked


def main():
    os.environ.update(MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1')
    os.environ.pop('MAKEPAD_FOCUS', None)
    root = Path('lab/wechat-ux/evidence/live/moments')
    fixture = json.loads((root/'fixture.json').read_text())
    assert all(u['user_id'].startswith('@robrix_ux_') for u in fixture['users'].values())
    seed = json.loads((root/'integration/live-result.json').read_text())
    friend = fixture['users']['emma']; author_user = fixture['users']['alex']
    members=checked(fixture['url'],'GET','rooms/'+seed['friend_timeline']+'/joined_members',token=friend['access_token'])['joined']
    if author_user['user_id'] in members:
        checked(fixture['url'],'POST','rooms/'+seed['friend_timeline']+'/kick',{'user_id':author_user['user_id']},friend['access_token'])
    checked(fixture['url'],'POST','rooms/'+seed['friend_timeline']+'/invite',{'user_id':author_user['user_id']},friend['access_token'])
    viewer_root = root/'viewer'
    viewer_root.mkdir(exist_ok=True, mode=0o700)
    viewer_fixture = {**fixture, 'users': {**fixture['users'], 'alex': fixture['users']['emma']}}
    (viewer_root/'fixture.json').write_text(json.dumps(viewer_fixture)); os.chmod(viewer_root/'fixture.json', 0o600)
    apps = []
    report = {'passed': False, 'checks': [], 'runs': []}
    unique = uuid.uuid4().hex[:6]
    def passed(name):
        report['checks'].append(name); print('PASS', name, flush=True)
    def start(path, port=8299, size=(375,812)):
        app=NativeApp(path,port,size=size); apps.append(app); app.start()
        report['runs'].append(str(app.output));app.wait_text('All Chats',timeout=90);return app
    def texts(app): return [w.get('t') or '' for w in app.snap()]
    def idle(app, timeout=60):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            busy=any(t.startswith(('Updating Moments','Encrypting and publishing','Opening private File Transfer')) for t in texts(app))
            if not busy: time.sleep(.3);return
            time.sleep(.3)
        raise AssertionError('Moments operation did not finish')
    def field(app, name, text):
        app.click_id(name);app.request('/k',c='A',cmd=1,wait=1);app.request('/k',c='Backspace',wait=1)
        if text: app.request('/t',t=text,wait=1)
    def open_feed(app):
        app.click_id('discover_tab');app.click_id('discover_moments');idle(app)
    def back(app): app.click(24,54);time.sleep(.4)
    def feed_action(app, name):
        if not any(w['i']==name for w in app.snap()):
            app.click_id('moments_menu')
        app.click_id(name)
    def wait_widget(app, name, text, timeout=45):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            if any(w['i']==name and text in (w.get('t') or '') for w in app.snap()):return
            time.sleep(.3)
        raise AssertionError('Missing native '+name+': '+text)
    def find_post(app, text):
        for _ in range(24):
            candidates=[w for w in app.snap() if w['i']=='post_body' and text in (w.get('t') or '') and 180 < w['r'][1] < 700]
            if candidates:
                x,y,w,h=candidates[0]['r'];app.click(x+min(w/2,100),y+min(h/2,12));return
            app.request('/m',k='scroll',x=300,y=580,dy=210,wait=1);time.sleep(.3)
        raise AssertionError('Post not reachable by scrolling: '+text)
    def post(app, text):
        idle(app)
        app.click_id('moments_compose');idle(app)
        app.wait_text('Timeline audience',pixels=True);field(app,'moments_body',text)
        app.click_id('moments_publish');idle(app)
        wait_widget(app,'post_body',text)
    try:
        author=start(root)
        assert 'My Moments' not in texts(author)
        author.capture('chats-with-file-transfer');passed('moments_rooms_excluded_from_flat_chats')
        open_feed(author);author.capture('moments-native-feed')
        assert texts(author).count("No posts yet. Post your first moment, or accept a friend's timeline invitation.")==0
        # Accept Emma's independently owned timeline; no automatic audience reciprocity.
        feed_action(author,'moments_invites');idle(author)
        author.wait_text('invited you',timeout=45)
        author.capture('moments-invitation');author.click_id('accept_moments');idle(author)
        members=checked(fixture['url'],'GET','rooms/'+seed['friend_timeline']+'/joined_members',token=friend['access_token'])['joined']
        assert author_user['user_id'] in members
        back(author);passed('timeline_invitation_has_explicit_join_and_back')
        viewer=start(viewer_root,8298)
        open_feed(viewer)
        text='Native moment '+unique+' · 朋友圈'
        post(author,text);author.capture('moments-native-post')
        feed_action(viewer,'moments_refresh');idle(viewer);wait_widget(viewer,'post_body',text)
        viewer.capture('moments-recipient-feed');passed('native_author_publishes_recipient_decrypts_and_renders')
        viewer.click_text(text.split(" · ")[0]);idle(viewer);viewer.wait_text('Moment',pixels=True)
        viewer.click_id('moments_like');idle(viewer);viewer.wait_text('Unlike',pixels=True)
        comment='Native comment '+unique+' 真好看'
        field(viewer,'moments_comment',comment);viewer.click_id('moments_comment_send');idle(viewer)
        wait_widget(viewer,'comment_body',comment);viewer.capture('moments-comment-like')
        author.click_text(text.split(" · ")[0]);idle(author)
        wait_widget(author,'comment_body',comment);wait_widget(author,'detail_likes','1 likes')
        passed('native_cross_account_comments_and_likes')
        viewer.click_id('comment_edit');field(viewer,'moments_comment',comment+' edited');viewer.click_id('moments_comment_send');idle(viewer)
        wait_widget(viewer,'comment_body',comment+' edited')
        viewer.click_id('comment_delete');idle(viewer)
        assert not any(w['i']=='comment_body' and comment in (w.get('t') or '') for w in viewer.snap())
        viewer.click_id('moments_like');idle(viewer);wait_widget(viewer,'detail_likes','0 likes')
        passed('native_comment_edit_delete_and_unlike')
        author.click_id('moments_edit');field(author,'moments_comment',text+' edited');author.click_id('moments_comment_send');idle(author)
        wait_widget(author,'detail_body',text+' edited')
        author.capture('moments-edited-post');passed('native_author_edits_post')
        viewer.click_id('moments_hide');idle(viewer)
        assert not any(w['i']=='post_body' and text in (w.get('t') or '') for w in viewer.snap())
        feed_action(viewer,'moments_audience');idle(viewer);viewer.click_id('unhide_author');idle(viewer);back(viewer)
        wait_widget(viewer,'post_body',text+' edited');passed('hide_author_and_unhide_in_audience')
        # Delete the native post and verify removal in the recipient feed.
        author.click_id('moments_delete');idle(author)
        assert not any(w['i']=='post_body' and text in (w.get('t') or '') for w in author.snap())
        feed_action(viewer,'moments_refresh');idle(viewer)
        assert not any(w['i']=='post_body' and text in (w.get('t') or '') for w in viewer.snap())
        passed('post_redaction_converges_on_recipient')
        # The author's existing encrypted album exercises lazy native media display.
        find_post(author,'Weekend album');idle(author)
        wait_widget(author,'detail_meta','Media 1 / 2');author.click_id('media_next');wait_widget(author,'detail_meta','Media 2 / 2')
        author.capture('moments-album-detail');back(author);passed('ordered_album_native_preview_and_navigation')
        feed_action(author,'moments_audience');idle(author);author.capture('moments-audience')
        assert fixture['users']['emma']['user_id'] in ' '.join(texts(author));back(author)
        back(author);author.click_id('chats_tab');author.click_id('file_transfer_entry');idle(author)
        author.wait_text('File Transfer',pixels=True,timeout=60);author.capture('file-transfer-native')
        # Check membership and separation through authenticated fixture APIs.
        members=checked(fixture['url'],'GET','rooms/'+seed['file_transfer']+'/joined_members',token=fixture['users']['alex']['access_token'])['joined']
        assert list(members)==[fixture['users']['alex']['user_id']]
        assert seed['file_transfer']!=seed['timeline'];passed('file_transfer_opens_normal_private_chat_without_moments_audience')
        back(author);author.click_id('me_tab');author.click_id('my_posts');idle(author)
        author.wait_text('My Posts',pixels=True);author.capture('my-posts-native');back(author)
        passed('me_my_posts_and_navigation_back')
        open_feed(author);author.click_id('moments_compose');idle(author)
        draft='Unsent restart draft '+unique
        field(author,'moments_body',draft);back(author);back(author)
        viewer.stop();apps.remove(viewer);author.stop();apps.remove(author)
        author=start(root)
        open_feed(author);author.click_id('moments_compose');idle(author)
        author.wait_text('Unsent restart draft',pixels=True)
        drafts=list((root/'profile').rglob('moments-composer.json'))
        assert any(json.loads(path.read_text()).get('body')==draft for path in drafts)
        author.capture('moments-restored-draft');field(author,'moments_body','');back(author);back(author)
        author.stop();apps.remove(author);passed('unsent_composer_survives_restart_without_publishing')
        author=start(root,size=(1050,800))
        author.click_id('moments_button');idle(author);author.wait_text('Moments',pixels=True);author.capture('moments-desktop');back(author)
        passed('desktop_moments_navigation_and_feed')
        author.click_id('file_transfer_entry');idle(author);author.wait_text('File Transfer',pixels=True,timeout=60)
        author.capture('file-transfer-desktop');passed('desktop_file_transfer_and_session_restore')
        report['passed']=True
    finally:
        for app in apps: app.stop()
        errors=[]
        for run in report['runs']:
            for line in (Path(run)/'native.log').read_text(errors='replace').splitlines():
                if '[E]' in line or 'panicked at' in line or 'Assertion failed:' in line: errors.append(line)
        report['native_error_count']=len(errors)
        if errors: report['passed']=False
        (root/'native-moments.json').write_text(json.dumps(report,indent=2))
        print(json.dumps({'passed':report['passed'],'checks':len(report['checks']),'native_errors':len(errors)}),flush=True)

if __name__=='__main__': main()
