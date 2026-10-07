#!/usr/bin/env python3
"""Whole native captures of production reading components with offline data.

This does not prove Matrix delivery, subscription support or phone-device parity.
The external mechanical scorer is recorded unchanged, never used as similarity.
"""
import argparse, hashlib, importlib.util, json, os, socket, subprocess, time
import urllib.error
from collections import Counter
from pathlib import Path
from PIL import Image
from native_probe import NativeApp

def text_contrast(path, snapshot, logical_width, scorer):
    """Check actual label pixels, including low-contrast text the row scorer misses.

    No screenshot is changed. The modal color of each text rectangle is its
    background. Ignore one-pixel noise; the strongest repeated ink must meet AA.
    This is a necessary readability check, not OCR or a similarity score.
    """
    im = Image.open(path).convert('RGB')
    scale = im.width / logical_width
    labels = {'post_author', 'post_body', 'post_meta', 'post_comments',
              'description', 'summary', 'preview_title', 'preview_author', 'preview_stats'}
    results = []
    for w in snapshot:
        paragraph = w['ty'] == 'Html' and w.get('t','').lstrip().startswith('<p>')
        if (w['i'] not in labels and not paragraph) or not w.get('t'): continue
        x, y, width, height = w['r']
        if height < 16 or (y+height)*scale > im.height-4: continue
        pixels = Counter(im.crop(tuple(round(v*scale) for v in (x,y,x+width,y+height))).getdata())
        bg = pixels.most_common(1)[0][0]
        ink = max((c for c,n in pixels.items() if n >= 4), key=lambda c: scorer.contrast_ratio(c,bg))
        ratio = scorer.contrast_ratio(ink, bg)
        results.append({'widget': w['i'], 'text': w['t'], 'background': bg, 'ink': ink,
                        'ratio': round(ratio,2), 'passed': ratio >= 4.5})
    return results

def journeys(app, screen, root, media_count=0):
    checks = []
    def check(name, condition):
        assert condition, name
        checks.append(name)
    def visible(name): return [w for w in app.snap() if w['i'] == name]
    def text(name): return [w.get('t','') for w in visible(name)]
    def key(code, **mods):
        app.request('/k', c=code, wait=1, **mods); time.sleep(.25)
    def theme(mode):
        app.request('/event', data='review:'+mode, wait=1); time.sleep(.4)
    def click_widget(w):
        x,y,width,height=w['r']; app.click(x+min(12,width/2),y+min(12,height/2))
    def tap(name):
        x,y,width,height=visible(name)[0]['r']
        app.request('/click',x=x+width/2,y=y+height/2,wait=0)
        # Read back the resulting frame explicitly. The SDK's wait=1 input
        # acknowledgement can reject a redundant present after page changes;
        # replaying the click would navigate twice.
        app.request('/g')
    if screen == 'moments':
        check('secondary actions initially collapsed', not visible('moments_refresh'))
        for name in ('moments_menu','moments_compose'):
            check(name+' hit target >= 44', all(w['r'][2]>=44 and w['r'][3]>=44 for w in visible(name)))
        app.click_id('moments_menu')
        check('more exposes refresh audience invitations', all(visible(n) for n in ('moments_refresh','moments_audience','moments_invites')))
        app.capture('moments-more')
        app.click_id('moments_menu')
        check('more collapses without leaving feed', not visible('moments_refresh') and visible('post_body'))
        if media_count:
            photos=sorted((w for w in app.snap() if w['ty']=='TextOrImage' and w['i'].startswith('a') and w['i'][1:].isdigit()), key=lambda w:int(w['i'][1:]))
            check('album renders all images',len(photos)==media_count)
            columns = 1 if media_count == 1 else 2 if media_count in (2,4) else 3
            check('album uses appropriate column count',len({w['r'][0] for w in photos})==columns)
            check('album cells remain square', all(abs(w['r'][2]-w['r'][3])<=2 for w in photos))
            click_widget(photos[-1])
            check('clicked photo opens its own position',any(f'{media_count} / {media_count}' in t for t in text('detail_meta')))
            app.capture('album-selected-photo')
            app.click_id('back')
        body=text('post_body')[0]
        click_widget(visible('post_body')[0])
        check('post opens its complete text', text('detail_body') == [body])
        if not media_count:
            check('existing comment is visible', any('这样的周末真好' in t for t in text('comment_body')))
            likes = text('detail_likes')
            tap('moments_like')
            check('liking without an account stays open with sign-in guidance',
                  text('detail_body') == [body] and any('需要先登录' in t for t in text('moments_status')))
            check('offline like is not presented as successful', text('detail_likes') == likes)
        else:
            tap('media_download')
            check('offline download stays open with sign-in guidance',
                  text('detail_body') == [body] and any('需要先登录' in t for t in text('moments_status')))
        tap('moments_comment')
        app.request('/t', t='离线预览不能发送这条评论', wait=1)
        tap('moments_comment_send')
        check('offline comment retains input without claiming delivery',
              text('moments_comment') == ['离线预览不能发送这条评论']
              and all('离线预览不能发送这条评论' not in t for t in text('comment_body'))
              and any('需要先登录' in t for t in text('moments_status')))
        app.capture('moment-detail')
        tap('back')
        check('back restores feed', body in text('post_body'))
        tap('moments_menu'); tap('moments_audience')
        check('audience page opens without an account', bool(visible('audience_name')))
        tap('share_dm_contacts')
        check('offline audience change stays open with sign-in guidance',
              bool(visible('audience_name')) and any('需要先登录' in t for t in text('moments_status')))
        tap('back')
        tap('moments_compose'); tap('moments_add_media')
        check('offline media picker gives guidance without starting an account operation',
              bool(visible('moments_body')) and any('需要先登录' in t for t in text('moments_status')))
        tap('compose_audience'); tap('share_dm_contacts'); tap('back')
        check('audience returns to composer without an account', bool(visible('moments_body')))
        tap('back')
        check('composer returns to feed without an account', body in text('post_body'))
    elif screen == 'library':
        check('classifications use readable names', all(visible(n) for n in ('drafts_tab','published_tab','withdrawn_tab')))
        check('draft summaries visible', bool(text('description')))
        app.click_id('published_tab')
        check('empty published classification has guidance', bool(visible('library_empty')) and not visible('title'))
        theme('light')
        check('classification survives theme switch', bool(visible('library_empty')) and not visible('title'))
        app.capture('library-empty-published')
        app.click_id('drafts_tab'); app.click_id('article_search')
        app.request('/t', t='咖啡', wait=1); time.sleep(.3)
        check('search returns matching draft', text('title') == ['一杯咖啡的时间'])
        app.capture('library-search')
        key('KeyA', cmd=1); key('Backspace')
        check('clearing search restores drafts', len(text('title')) >= 3)
    else:
        def tabs(): return sorted((w for w in app.snap() if w['ty']=='Tab'),key=lambda w:w['r'][0])
        check('two document tabs present',len(tabs())==2)
        check('document has no redundant URL toolbar', not visible('web_address'))
        reader=visible('article_reader')[0]['r']; width=app.request('/s')['w'][0]['sz'][0]
        check('reader has equal gutters and bounded line width',reader[0]>=24 and reader[2]<=760 and abs(reader[0]-(width-reader[0]-reader[2]))<=2)
        check('scrollbar has space outside article text',all(w['r'][0]+w['r'][2]<=reader[0]+reader[2]-12 for w in app.snap() if w['ty']=='Html'))
        key('Tab', ctrl=1)
        check('Control Tab cycles documents', any('周末读书笔记' == t for t in text('preview_title')))
        key('Tab', ctrl=1, shift=1)
        check('Control Shift Tab cycles back', any('把普通的日子' in t for t in text('preview_title')))
        x,y,w,h=reader
        app.request('/m',k='scroll',x=x+w/2,y=y+h/2,dy=460,wait=1);time.sleep(.8)
        check('heading scrolls with article', not visible('preview_title'))
        saved=sorted(w.get('t','') for w in app.snap() if w['ty']=='Html')
        key('Tab',ctrl=1); key('Tab',ctrl=1)
        check('tab switch preserves article position',saved == sorted(w.get('t','') for w in app.snap() if w['ty']=='Html'))
        theme('light')
        check('theme preserves article position',saved == sorted(w.get('t','') for w in app.snap() if w['ty']=='Html'))
        app.capture('reader-scrolled')
        key('KeyW',cmd=1)
        check('Command W closes only active tab',len(tabs())==1 and visible('article_reader'))
    (root/'journey-trace.json').write_text(json.dumps(app.trace,ensure_ascii=False,indent=2))
    return checks

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--screens', nargs='+', choices=('moments', 'library', 'reader'), default=('moments', 'library', 'reader'))
    parser.add_argument('--scorer', type=Path, default=Path('/Users/ychen/home/octosense-org/Octoscript-OH/tools/uxscore.py'))
    parser.add_argument('--media-count', type=int, choices=range(10), default=0)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location('pixel_score', args.scorer)
    scorer = importlib.util.module_from_spec(spec); spec.loader.exec_module(scorer)
    report = {'accepted': False, 'threshold': 9.5, 'screens': [], 'journeys': {}, 'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'scorer_sha256': hashlib.sha256(args.scorer.read_bytes()).hexdigest(), 'scope': 'Offline production components on native macOS Metal; phone width is not a physical phone test.'}
    try:
        for screen in args.screens:
            for narrow in (True, False):
                name = screen + ('-mobile' if narrow else '-desktop')
                root = args.output / name
                profile = root / 'profile'; profile.mkdir(parents=True)
                (profile/'ui-language.json').write_text(json.dumps('zh-CN'))
                with socket.socket() as sock: sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
                app = NativeApp(root, port, auto_login=False)
                app.output.mkdir(parents=True)
                app.log = (app.output/'native.log').open('w')
                app.process = subprocess.Popen([str(args.binary.resolve()), *(['--narrow'] if narrow else [])], stdout=app.log, stderr=subprocess.STDOUT, env=dict(os.environ, RINX_DATA_DIR=str(profile.resolve()), MAKEPAD_REMOTE=str(port), MAKEPAD_HIDE_WINDOWS='1', MAKEPAD_NO_FOCUS='1', RINX_REVIEW_SCREEN=screen, RINX_REVIEW_MEDIA=str(args.media_count)))
                try:
                    for _ in range(200):
                        assert app.process.poll() is None, 'Native process exited'
                        native_log = (app.output/'native.log').read_text(errors='replace')
                        assert '[E]' not in native_log and 'panicked at' not in native_log, native_log[-2000:]
                        try:
                            assert app.request('/s')['pid'] == app.process.pid
                            if any(w.get('i') == {'moments':'post_body','library':'article_library','reader':'article_reader'}[screen] for w in app.snap()): break
                        except OSError: pass
                        time.sleep(.1)
                    else: raise AssertionError('Native surface did not become ready')
                    for theme in ('light', 'dark'):
                        app.request('/event', data='review:'+theme, wait=1)
                        time.sleep(.6)
                        app.request('/event', data='review:fonts', wait=1)
                        fonts = json.loads((profile/'font-review.json').read_text())
                        (root/(theme+'-fonts.json')).write_text(json.dumps(fonts,indent=2))
                        assert all(f['members'][0] == 'apple_system' for f in fonts), fonts
                        path = app.capture(name+'-'+theme)
                        snap = app.snap()
                        (root/(theme+'-widgets.json')).write_text(json.dumps(snap,ensure_ascii=False,indent=2))
                        score = scorer.score(str(path))
                        contrast = text_contrast(path,snap,app.request('/s')['w'][0]['sz'][0],scorer)
                        report['screens'].append({'name':name+'-'+theme,'path':str(path.resolve()),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'score':score,'text_contrast':contrast})
                        assert contrast and all(c['passed'] for c in contrast), contrast
                        if screen == 'library':
                            im = Image.open(path).convert('RGB')
                            scale = im.width/app.request('/s')['w'][0]['sz'][0]
                            fills = []
                            for tab in ('drafts_tab','published_tab','withdrawn_tab'):
                                x,y,_,_=next(w['r'] for w in snap if w['i']==tab)
                                fills.append(im.getpixel((round((x+12)*scale),round((y+8)*scale))))
                            assert fills[1] == fills[2] and fills[0] != fills[1], fills
                            report['screens'][-1]['selected_tab_fills'] = fills
                        print(name, theme, score['total'], flush=True)
                    report['journeys'][name] = journeys(app,screen,root,args.media_count)
                    log = (app.output/'native.log').read_text(errors='replace')
                    assert '[E]' not in log and 'panicked at' not in log, log[-1500:]
                finally: app.stop()
        report['minimum'] = min(s['score']['total'] for s in report['screens'])
        report['mechanical_gate'] = report['minimum'] >= report['threshold']
        # Full UX acceptance additionally requires the reference and journey review.
    except Exception as error:
        report['error'] = str(error)
        if isinstance(error, urllib.error.HTTPError):
            report['native_error'] = error.read().decode(errors='replace')
        raise
    finally:
        (args.output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2))
if __name__ == '__main__': main()
