#!/usr/bin/env python3
"""Capture the production Palpo UI with isolated, populated Rust service fixtures.

Scores use the supplied unmodified uxscore.py; screenshots are whole native
windows. This is desktop/narrow-layout evidence, not a physical-phone claim.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import html
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import threading
import time
import uuid
from urllib.error import HTTPError
from http.server import ThreadingHTTPServer

from native_palpo_coordinator import Matrix, HTTP, call, digest, launch, machine_update, port, post
from native_palpo import fill, inspect


def score_summary(screens, target):
    """Keep workflow success distinct from the requested visual threshold."""
    after = [screen for screen in screens if screen['variant'] == 'after']
    values = [screen['score']['total'] for screen in after]
    below = [screen['name'] for screen in after if screen['score']['total'] < target]
    return {'target': target, 'met': bool(after) and not below,
            'mean': round(sum(values) / len(values), 2) if values else None,
            'minimum': min(values) if values else None, 'screens_below_target': below}


def write_review(root, report):
    """A local index of whole, unaltered native captures and their raw scores."""
    visual = report['visual_score']
    cards = []
    for screen in report['screens']:
        name = html.escape(screen['name'].replace('-', ' ').title())
        path = html.escape(Path(screen['path']).relative_to(root.resolve()).as_posix(), quote=True)
        size = 'narrow' if screen['name'].startswith('narrow-') else 'desktop'
        variant = screen['variant']
        score = screen['score']['total']
        parts = ' · '.join(f'{html.escape(key)} {value}/2' for key, value in screen['score']['parts'].items())
        cards.append(f'<article data-size="{size}" data-variant="{variant}"><h2>{name} <b>{score:.2f}/10</b></h2>'
                     f'<p>{parts}</p><a href="{path}"><img src="{path}" alt="{name}, whole native window" loading="lazy"></a></article>')
    functional = 'passed' if report['functional_passed'] else 'failed'
    target = 'met' if visual['met'] else 'not met'
    page = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Hagency native UX review</title><style>
*{box-sizing:border-box}body{margin:0;background:#f5f6f8;color:#19232d;font:16px/1.6 system-ui,sans-serif}
header,main{max-width:1200px;margin:auto;padding:24px}h1{margin:0;font-size:30px}header p{max-width:850px}
nav{display:flex;gap:16px;flex-wrap:wrap}select{font:inherit;padding:8px;border:1px solid #8996a3;border-radius:6px;background:white}
article{background:white;border:1px solid #d3dbe2;border-radius:12px;padding:20px;margin-bottom:24px}
h2{font-size:20px;margin:0}h2 b{float:right}article p{color:#526170;font-size:14px}img{display:block;max-width:100%;height:auto;margin:auto}
article[data-size=narrow] img{max-width:min(100%,430px)}a{color:#075985}[hidden]{display:none}
</style><header><h1>Hagency native UX review</h1>'''
    page += f'<p>Functional checks: <strong>{functional}</strong>. Visual target {visual["target"]:.1f}/10: <strong>{target}</strong>. '
    page += f'After-screen mean: <strong>{visual["mean"]}</strong>; minimum: <strong>{visual["minimum"]}</strong>.</p>'
    page += '<p>Whole native windows, scored with the unmodified Octoscript-OH pixel scorer. Desktop and narrow layouts use isolated fixture accounts. This is not physical-phone or live Hagency validation. Sparse forms can lose content-density points. Every result is retained, including scores below target.</p>'
    page += '<nav><label>Layout <select id="size"><option value="all">All</option><option value="narrow">Narrow</option><option value="desktop">Desktop</option></select></label><label>Version <select id="variant"><option value="all">Before and after</option><option value="after">After</option><option value="before">Before</option></select></label><a href="report.json">Raw report</a></nav></header><main>'
    page += ''.join(cards)
    page += '''</main><script>
function filter(){for(const card of document.querySelectorAll('article')){card.hidden=
(size.value!=='all'&&card.dataset.size!==size.value)||(variant.value!=='all'&&card.dataset.variant!==variant.value)}}
const size=document.querySelector('#size'),variant=document.querySelector('#variant');size.onchange=filter;variant.onchange=filter;
</script></html>'''
    (root / 'review.html').write_text(page)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/fast/examples/palpo_miniapp'))
    parser.add_argument('--scorer', type=Path, required=True)
    parser.add_argument('--before', type=Path, help='Optional original bundle source for identical-data comparison')
    parser.add_argument('--score-target', type=float, default=9.5)
    parser.add_argument('--report-only', action='store_true', help='Record scores without failing the command below target')
    parser.add_argument('--i18n', action='store_true', help='Exercise English/Chinese rebakes and capture Chinese screens')
    parser.add_argument('--layout', choices=('all', 'narrow', 'desktop'), default='all',
                        help='Run both layouts or reproduce one native layout')
    args = parser.parse_args()
    if not 0 <= args.score_target <= 10:
        parser.error('--score-target must be between 0 and 10')
    spec = importlib.util.spec_from_file_location('pixel_uxscore', args.scorer)
    scorer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(scorer)
    root = Path('target/palpo-ux-review') / uuid.uuid4().hex
    root.mkdir(parents=True)
    source = Path('apps/palpo/bundle/main.splash')
    report = {'passed': False, 'functional_passed': False, 'scope': 'Real native UI and Rust Palpo; isolated Matrix/provider fixtures',
              'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
              'scorer_sha256': hashlib.sha256(args.scorer.read_bytes()).hexdigest(),
              'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'screens': [], 'checks': []}
    matrix = ThreadingHTTPServer(('127.0.0.1', 0), Matrix)
    threading.Thread(target=matrix.serve_forever, daemon=True).start()
    endpoint = f'http://127.0.0.1:{port()}'
    env = {k: v for k, v in os.environ.items() if not k.startswith('PALPO_')}
    env.update(PALPO_SERVER_NAME='example.test', PALPO_URL=f'http://127.0.0.1:{matrix.server_port}',
               PALPO_ADMIN_DATABASE=str((root / 'admin.sqlite').resolve()), PUBLIC_ORIGIN=endpoint,
               PALPO_OPERATIONS_LISTEN=endpoint.removeprefix('http://'), PALPO_TRANSPORT_ORIGIN=endpoint,
               PALPO_RELAY_ORIGIN=endpoint)
    fleet = 'hf_' + 'a' * 32
    now = int(time.time() * 1000)
    authority = {'engagements': {fleet: {'id': fleet, 'server': 'example.test', 'owner': '@provider:example.test',
        'coordinator': '@coordinator:example.test', 'registrationGeneration': 1, 'delegationRevision': 1,
        'delegationExpiresAtMs': now + 3600000, 'state': 'verified', 'allowSelfApproval': False,
        'coordinatorApprovalV1': True}},
        'resources': {'grant_a': {'id': 'grant_a', 'serverEngagementId': fleet, 'revision': 1,
            'allocatedTokens': 1000000, 'eligibleManagers': ['@owner:example.test']}},
        'projects': {'launch_workspace': {'projectId': 'launch_workspace', 'serverEngagementId': fleet,
            'revision': 1, 'owner': '@owner:example.test', 'resourceAllocations': ['grant_a'], 'state': 'ready'}}}
    authority['resources']['grant_b'] = dict(authority['resources']['grant_a'], id='grant_b', allocatedTokens=500000)
    project_names = {'launch_workspace': 'Launch workspace', 'documentation': 'Documentation', 'release_review': 'Release review'}
    for project_id in project_names:
        authority['projects'][project_id] = dict(authority['projects']['launch_workspace'], projectId=project_id)
    path = root / 'authority.json'
    path.write_text(json.dumps(authority))
    subprocess.run([str(args.backend.resolve()), 'import-authority', str(path.resolve())], env=env,
                   check=True, stdout=subprocess.DEVNULL)
    with sqlite3.connect(root / 'admin.sqlite') as db:
        state = json.loads(db.execute('SELECT body FROM state WHERE id=1').fetchone()[0])
        state['fleets'][fleet] = {'id': fleet, 'name': 'Studio resources', 'state': 'ready', 'installation': 'installed',
            'ownerMxid': '@provider:example.test', 'representativeMxid': f'@{fleet}_representative:example.test',
            'registrationGeneration': 1, 'transport': {'mode': 'outbound', 'generation': 1, 'sequence': 0, 'token': 'fixture-machine'},
            'capabilities': {'coordinatorApprovalV1': True, 'coordinatorAgentControlV1': True,
                             'coordinatorAgentProfileV1': True, 'coordinatorProjectSetupV1': True}}
        for project_id, project_name in project_names.items():
            definition = {'name': project_name, 'roomId': '!' + project_id + ':example.test', 'ownerDmRoomId': '!private:example.test'}
            definition_digest = digest(definition)
            state['rustWorkflows']['definitions'][definition_digest] = definition
            action_id = 'action_' + project_id
            state['rustWorkflows']['actions'][action_id] = {'id': action_id, 'revision': 1, 'state': 'approved', 'execution': 'done',
                'createdAt': now, 'updatedAt': now, 'decision': None,
                'request': {'kind': 'project', 'request': {'id': 'request_' + project_id, 'revision': 1,
                    'serverEngagementId': fleet, 'projectId': project_id, 'owner': '@owner:example.test',
                    'requester': '@owner:example.test', 'definitionDigest': definition_digest, 'resourceAllocations': ['grant_a']}}}
        state['fleets'][fleet]['capabilities']['offers'] = [
            {'role': 'coding', 'description': 'Implementation and project work', 'resources': [
                {'id': 'resource_' + '1' * 24, 'name': 'Local Codex', 'framework': 'codex', 'model': 'gpt-5.6-sol'}]},
            {'role': 'review', 'description': 'Code and quality review', 'resources': [
                {'id': 'resource_' + '2' * 24, 'name': 'Code review', 'framework': 'codex', 'model': 'gpt-5.6-sol'}]}]
        state['fleets'][fleet]['capabilities']['offers'].append({
            'role': 'documentation', 'description': 'Write and maintain documentation',
            'resources': state['fleets'][fleet]['capabilities']['offers'][0]['resources']})
        catalog_capabilities = dict(state['fleets'][fleet]['capabilities'], v=1, fleetId=fleet,
            serverName='example.test', representativeMxid=f'@{fleet}_representative:example.test',
            approvalBotMxid=f'@{fleet}_approval:example.test')
        catalog_capabilities['resourceBudgets'] = [
            dict(id='grant_a', resourceId='resource_'+'1'*24, revision=1, allocatedTokens=1000000,
                 retainedTokens=300000, remainingTokens=700000, overdrawn=False, period='month', periodKey='2026-10'),
            dict(id='grant_b', resourceId='resource_'+'2'*24, revision=1, allocatedTokens=500000,
                 retainedTokens=500000, remainingTokens=0, overdrawn=False, period='month', periodKey='2026-10')]
        state['rustWorkflows']['resourceDetails'] = {
            'grant_a': {'resourceId': 'resource_' + '1' * 24, 'period': 'month', 'periodKey': '2026-10'},
            'grant_b': {'resourceId': 'resource_' + '2' * 24, 'period': 'month', 'periodKey': '2026-10'}}
        db.execute('UPDATE state SET body=? WHERE id=1', [json.dumps(state)])
    log = (root / 'backend.log').open('w')
    server = subprocess.Popen([str(args.backend.resolve())], env=env, stdout=log, stderr=subprocess.STDOUT)
    apps = []

    def capture(app, name, before=False):
        app.request('/m', k='scroll', x=250, y=650, dy=-5000, wait=1)
        time.sleep(.8)
        path = app.capture(name)
        snapshot = app.request('/snap', all=1)
        (app.root / (name + '.json')).write_text(json.dumps(snapshot, indent=2))
        width, height = app.request('/s')['w'][0]['sz']
        content = next(w['r'] for w in snapshot['s'] if w.get('i') == 'content' and w['r'][2] > 0)
        bottom = min(height, content[1] + content[3])
        # Snapshot rectangles are clipped to the scroll viewport. A partial
        # control at its bottom is not the control's actual target height.
        controls = [w for w in snapshot['s'] if w.get('ty') in ('Button', 'TextInput') and w['r'][2] > 0
                    and w['r'][1] >= 0 and w['r'][1] + w['r'][3] < bottom - 1]
        horizontal_ok = all(w['r'][0] >= 0 and w['r'][0] + w['r'][2] <= width + 1 for w in controls)
        targets_ok = all(w['r'][3] >= 43 for w in controls)
        if not before:
            assert horizontal_ok, 'Horizontal control overflow'
            assert targets_ok, 'Control below shared 44-point target (1px snapshot rounding)'
        score = scorer.score(str(path))
        report['screens'].append({'name': name, 'path': str(path.resolve()),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'score': score,
            'variant': 'before' if before else 'after', 'visible_controls': len(controls), 'no_horizontal_control_overflow': horizontal_ok, 'shared_touch_targets': targets_ok})
        print(json.dumps({'screen': name, 'score': score['total'], 'path': str(path)}), flush=True)

    try:
        for _ in range(100):
            if server.poll() is not None:
                raise RuntimeError('Fixture service exited')
            try:
                with HTTP.open(endpoint + '/healthz', timeout=1):
                    break
            except OSError:
                time.sleep(.1)
        owner_token = post(endpoint, 'session', 'owner-secret', {'appId': 'im.palpo.operations', 'bundleDigest': 'c' * 64,
            'services': ['palpo.inbox.submit', 'palpo.inbox.get']})['sessionToken']
        coordinator_token = post(endpoint, 'session', 'coordinator-secret', {'appId': 'im.palpo.operations', 'bundleDigest': 'd' * 64,
            'services': ['palpo.inbox.decide']})['sessionToken']
        observations = []
        receipts = []
        for i, (name, instructions) in enumerate([
            ('frank-lee', 'Build and review the project implementation.'),
            ('littlebird', 'Keep the project notes and next steps up to date.'),
            ('reviewer', 'Check changes and report reproducible issues.'),
            ('researcher', 'Research open questions for the project.')
        ]):
            request_id = format(i + 1, '040x')
            definition = {'v': 1, 'fleetId': fleet, 'requestId': request_id, 'targetProjectId': 'launch_workspace',
                'targetRoomId': '!project:example.test', 'sourceRoomId': '!reception:example.test',
                'sourceEventId': '$fixture-' + name, 'ownerMxid': '@owner:example.test',
                'requesterMxid': '@owner:example.test', 'ownerDmRoomId': '!private:example.test',
                'role': 'coding', 'requestedTokens': 100000, 'agentDefinition': {'name': name, 'instructions': instructions}}
            request = {'kind': 'agent', 'request': {'id': request_id, 'revision': 1, 'serverEngagementId': fleet,
                'projectId': 'launch_workspace', 'projectRevision': 1, 'resourceAllocationId': 'grant_a',
                'projectOwner': '@owner:example.test', 'requester': '@owner:example.test',
                'definitionDigest': digest(definition), 'requestedTokens': 100000}, 'definition': definition}
            action = call(endpoint, owner_token, 'palpo.inbox.submit', request)['action']
            if i < 2:
                continue
            command_id = format(i + 16, '040x')
            call(endpoint, coordinator_token, 'palpo.inbox.decide', {'id': action['id'], 'expectedRevision': action['revision'],
                'decision': 'approve', 'commandId': command_id, 'reason': 'Approved for the project.'})
            with sqlite3.connect(root / 'admin.sqlite') as db:
                state = json.loads(db.execute('SELECT body FROM state WHERE id=1').fetchone()[0])
                command = state['rustWorkflows']['outbox'][command_id]['command']
            receipt = {'kind': 'receipt', 'registrationGeneration': 1, 'delegationRevision': 1, 'commandId': command_id,
                'state': 'applied', 'agentId': 'en_' + name,
                'commandDigest': digest({'operation': 'coordinator_agent_approval', 'command': command})}
            observation = dict(definition, engagementId='en_' + name, state='active', ready=True, bound=True, quotaPaused=False,
                allocatedTokens=100000, consumedTokens=4200 + i * 1000, usageObservedAtMs=now, usageComplete=False, usageEvidence="host_attributed_lower_bound",
                agentMxid=f'@{fleet}_en_{name}:example.test',
                fulfillment={'phase': 'complete', 'incomplete': False}, observedAt=datetime.now(timezone.utc).isoformat(),
                lifecycle={'runtimeState': 'active', 'paused': False, 'cleanup': 'not_required', 'cleanupEffect': None})
            observations.append(observation)
            receipts.append({'id': 'command_' + command_id, 'payload': receipt, 'digest': digest(receipt)})
        try:
            machine_update(endpoint, fleet, {'v': 2, 'generation': 1, 'sequence': 1, 'heartbeat': True,
                'statuses': observations, 'coordinatorUpdates': receipts})
        except HTTPError as error:
            raise RuntimeError(error.read().decode()) from error
        sequence = 1
        def refresh_runtime_fixture(stale_capacity=False):
            # The longer bilingual matrix can outlive the server's liveness
            # interval. Model a reporting provider; never weaken freshness rules.
            nonlocal sequence
            sequence += 1
            for observation in observations:
                observation['observedAt'] = datetime.now(timezone.utc).isoformat()
                observation['usageObservedAtMs'] = int(time.time() * 1000)
            catalog_capabilities['resourceBudgetObservedAtMs'] = int(time.time()*1000) - (100000 if stale_capacity else 0)
            machine_update(endpoint, fleet, {'v': 2, 'generation': 1, 'sequence': sequence,
                'heartbeat': True, 'statuses': observations, 'coordinatorUpdates': [], 'capabilities': catalog_capabilities})
        layouts = (True, False) if args.layout == 'all' else (args.layout == 'narrow',)
        for narrow in layouts:
            refresh_runtime_fixture()
            size = 'narrow' if narrow else 'desktop'
            if args.before:
                old_owner = launch(root / (size + '-before-owner'), args.binary, endpoint, 'owner', narrow=narrow, ux_review=True, bundle_source=args.before)
                apps.append(old_owner)
                old_owner.click_id('requests'); old_owner.wait_text('Execution · ready')
                capture(old_owner, size + '-before-agents', before=True)
                old_owner.stop(); apps.remove(old_owner)
                old_coordinator = launch(root / (size + '-before-coordinator'), args.binary, endpoint, 'coordinator', narrow=narrow, ux_review=True, bundle_source=args.before)
                apps.append(old_coordinator)
                old_coordinator.wait_text('littlebird')
                capture(old_coordinator, size + '-before-inbox', before=True)
                old_coordinator.stop(); apps.remove(old_coordinator)
                report['before_source_sha256'] = hashlib.sha256(args.before.read_bytes()).hexdigest()
            owner = launch(root / (size + '-owner'), args.binary, endpoint, 'owner', narrow=narrow, ux_review=True)
            apps.append(owner)
            owner.click_id('requests'); owner.wait_text('Ready to chat')
            capture(owner, size + '-agents-light')
            owner.click_id('manage_agent'); owner.wait_text('Remove agent')
            capture(owner, size + '-agent-controls')
            owner.click_id('rename_agent'); owner.wait_text('Display name')
            before = inspect(owner)
            fill(owner, 'Display name', 'Draft name')
            owner.request('/event', data='palpo:dark', wait=1); owner.wait_text('Draft name')
            assert inspect(owner)['heap'] == before['heap']
            capture(owner, size + '-form-dark')
            if args.i18n:
                draft = owner.root / 'profile/app/draft.json'
                saved = draft.read_bytes()
                owner.request('/event', data='palpo:zh-CN', wait=1)
                owner.wait_text('显示名称'); owner.wait_text('Draft name'); owner.wait_text('已分配：')
                current = inspect(owner)
                assert (current['heap'], current['connects'], current['calls']) == (before['heap'], before['connects'], before['calls'])
                assert draft.read_bytes() == saved, 'Language switch rewrote draft or request identity'
                capture(owner, size + '-zh-form-dark')
                owner.request('/event', data='palpo:en', wait=1)
                owner.wait_text('Display name'); owner.wait_text('Allocated:')
                assert draft.read_bytes() == saved
                report['checks'].append(size + ': English/Chinese switches preserve heap, session, request count and exact saved draft')
            owner.click_id('cancel'); owner.wait_text('Ready to chat')
            capture(owner, size + '-agents-dark')
            owner.request('/event', data='palpo:light', wait=1)
            owner.click_id('more'); owner.click_id('notifications'); owner.wait_text('Action notifications: On')
            capture(owner, size + '-notifications')
            owner.click_id('projects'); owner.wait_text('Launch workspace')
            capture(owner, size + '-projects')
            owner.click_id('agent'); owner.wait_text('Use for Coding')
            owner.click_text('Use for Coding'); owner.wait_text('Agent name')
            fill(owner, 'Agent name', 'GuidedAgent')
            original = inspect(owner)
            draft = owner.root / 'profile/app/draft.json'
            assert json.loads(draft.read_text())['payload']['role'] == 'coding', 'Role button submitted a different capability'
            intent = json.loads(draft.read_text())['payload']['requestId']
            owner.click_id('review_form'); owner.wait_text('3 · Review and send')
            assert inspect(owner)['calls'] == original['calls'], 'Review sent a host command'
            assert json.loads(draft.read_text())['payload']['requestId'] == intent
            capture(owner, size + '-agent-review')
            owner.click_id('edit_request'); owner.wait_text('Agent name'); owner.wait_text('GuidedAgent')
            assert json.loads(draft.read_text())['payload']['requestId'] == intent
            fill(owner, 'Initial tokens', '2000001')
            fill(owner, 'Daily rate', '1234')
            owner.click_id('review_form'); owner.wait_text('3 · Review and send')
            owner.click_id('submit'); owner.wait_text('Request not submitted')
            owner.wait_text('resource_not_granted:')
            rejected = json.loads(draft.read_text())['payload']
            assert rejected['requestId'] == intent
            assert rejected['agentDefinition']['name'] == 'GuidedAgent'
            assert rejected['requestedTokens'] == '2000001' and rejected['ratePerDay'] == '1234'
            assert 'agentName' not in rejected
            capture(owner, size + '-agent-form-error')
            if args.i18n:
                saved = draft.read_bytes()
                owner.request('/event', data='palpo:zh-CN', wait=1)
                owner.wait_text('申请未提交'); owner.wait_text('resource_not_granted:')
                assert draft.read_bytes() == saved
                capture(owner, size + '-zh-agent-form-error')
                owner.request('/event', data='palpo:en', wait=1)
                owner.wait_text('Request not submitted')
            owner.click_id('edit_request'); owner.wait_text('Agent name')
            fill(owner, 'Initial tokens', '40000')
            owner.click_id('review_form'); owner.wait_text('3 · Review and send')
            assert not any(w.get('t') == 'Request not submitted' for w in owner.snap())
            assert json.loads(draft.read_text())['payload']['requestId'] == intent
            report['checks'].append(size + ': rejected agent form retains edited values and request identity, shows inline error, and clears it after correction')
            owner.click_id('cancel'); owner.wait_text('Use for Coding')
            report['checks'].append(size + ': cancelling an agent request preserves the selected project and resource catalog')
            # The preceding screenshots can outlive the 90-second capacity
            # interval; publish again as a connected provider would.
            refresh_runtime_fixture()
            owner.click_id('resources'); owner.wait_text('Resources published')
            capture(owner, size + '-resources')
            owner.wait_text('700000 / 1000000 tokens available')
            texts=[w.get('t') for w in owner.request('/snap', all=1)['s']]
            assert texts.count('Local Codex') == 1, 'Shared-capability allocation duplicated'
            owner.click_text('Documentation'); owner.wait_text('700000 / 1000000 tokens available')
            assert not any(w.get('t')=='Code review' for w in owner.request('/snap', all=1)['s'])
            capture(owner, size + '-resources-documentation')
            owner.request('/event', data='palpo:dark', wait=1)
            owner.wait_text('Documentation')
            assert not any(w.get('t')=='Code review' for w in owner.request('/snap', all=1)['s'])
            owner.request('/event', data='palpo:light', wait=1)
            refresh_runtime_fixture(stale_capacity=True)
            owner.click_id('refresh'); owner.wait_text('Unknown / 1000000 tokens available')
            owner.wait_text('Capacity update is stale')
            capture(owner, size + '-resources-stale')
            refresh_runtime_fixture()
            owner.click_id('refresh'); owner.wait_text('700000 / 1000000 tokens available')
            owner.click_id('all_capabilities'); owner.wait_text('Code review')
            owner.click_text('Review'); owner.wait_text('0 / 500000 tokens available')
            capture(owner, size + '-resources-exhausted')
            owner.click_id('all_capabilities'); owner.wait_text('Local Codex')
            report['checks'].append(size + ': capability filters group shared allocations, preserve theme state, and distinguish stale/zero capacity')
            owner.click_id('choose'); owner.wait_text('Project name')
            owner.click_id('review_form'); owner.wait_text('Complete the request details')
            owner.wait_text('Project name')
            fill(owner, 'Project name', 'Guided project')
            fill(owner, 'What will your project do?', 'Validate the project workflow')
            original = inspect(owner)
            owner.click_id('review_form'); owner.wait_text('3 · Review and send')
            assert inspect(owner)['calls'] == original['calls'], 'Review sent a host command'
            capture(owner, size + '-project-review')
            owner.click_id('edit_request'); owner.wait_text('Guided project')
            owner.click_id('cancel'); owner.wait_text('Resources published')
            owner.click_id('more'); owner.click_id('guide'); owner.wait_text('One account, clear next steps')
            capture(owner, size + '-workflow-guide')
            owner.click_id('resources'); owner.wait_text('Resources published')
            report['checks'].append(size + ': project-request cancellation returns to Resources')
            coordinator = launch(root / (size + '-coordinator'), args.binary, endpoint, 'coordinator', narrow=narrow, ux_review=True)
            apps.append(coordinator)
            coordinator.wait_text('frank-lee')
            capture(coordinator, size + '-inbox')
            coordinator.click_id('review'); coordinator.wait_text('Requested allocation')
            capture(coordinator, size + '-request')
            coordinator.click_id('approve'); coordinator.wait_text('Decision reason')
            capture(coordinator, size + '-approval')
            if args.i18n:
                coordinator.request('/event', data='palpo:zh-CN', wait=1)
                coordinator.wait_text('审批理由'); coordinator.wait_text('智能体申请')
                capture(coordinator, size + '-zh-approval')
                coordinator.click_id('cancel'); coordinator.wait_text('申请的配额')
                capture(coordinator, size + '-zh-request')
                coordinator.click_id('inbox'); coordinator.wait_text('需要我处理')
                capture(coordinator, size + '-zh-inbox')
                owner.request('/event', data='palpo:zh-CN', wait=1)
                owner.wait_text('资源')
                capture(owner, size + '-zh-resources')
                owner.click_id('choose'); owner.wait_text('项目名称')
                fill(owner, '项目名称', '中文项目 Projects')
                draft = owner.root / 'profile/app/draft.json'
                saved = draft.read_bytes()
                owner.request('/event', data='palpo:en', wait=1)
                owner.wait_text('Project name'); owner.wait_text('中文项目 Projects')
                assert draft.read_bytes() == saved, 'User content changed across locale switch'
                owner.request('/event', data='palpo:zh-CN', wait=1)
                owner.wait_text('项目名称'); owner.wait_text('中文项目 Projects')
                capture(owner, size + '-zh-project-form')
                fill(owner, '你的项目将做什么？', '验证流程')
                owner.click_id('review_form'); owner.wait_text('3 · 核对并发送')
                capture(owner, size + '-zh-project-review')
                owner.request('/event', data='palpo:en', wait=1)
                owner.wait_text('3 · Review and send'); owner.wait_text('中文项目 Projects')
                owner.request('/event', data='palpo:zh-CN', wait=1)
                owner.wait_text('3 · 核对并发送')
                owner.click_id('cancel'); owner.click_id('projects'); owner.wait_text('Launch workspace')
                capture(owner, size + '-zh-projects')
                refresh_runtime_fixture()
                owner.click_id('requests'); owner.wait_text('可以开始聊天')
                capture(owner, size + '-zh-agents')
                coordinator.request('/event', data='palpo:en', wait=1)
                coordinator.wait_text('Needs my action')
                coordinator.click_id('review'); coordinator.wait_text('Requested allocation')
                coordinator.click_id('approve'); coordinator.wait_text('Decision reason')
            coordinator.click_id('cancel'); coordinator.wait_text('Requested allocation')
            report['checks'].append(size + ': cancel returns to request; form theme reload retains draft; primary and overflow navigation work')
        for app in apps:
            native_log = (app.output / 'native.log').read_text(errors='replace')
            assert not any(marker in native_log for marker in ('[E]', 'panicked at', 'on_render closure failed', 'callback error', 'script time budget exceeded')), native_log[-2000:]
        assert hashlib.sha256(source.read_bytes()).hexdigest() == report['source_sha256'], 'Source changed during capture'
        assert hashlib.sha256(args.binary.read_bytes()).hexdigest() == report['binary_sha256'], 'Native binary changed during capture'
        report['functional_passed'] = True
    except Exception:
        for app in apps:
            try:
                app.capture('failure')
                (app.root / 'failure.json').write_text(json.dumps(app.request('/snap', all=1)))
            except Exception:
                pass
        raise
    finally:
        for app in apps:
            app.stop()
        server.terminate()
        server.wait(timeout=15)
        log.close()
        matrix.shutdown(); matrix.server_close()
        report['visual_score'] = score_summary(report['screens'], args.score_target)
        report['passed'] = report['functional_passed'] and report['visual_score']['met']
        (root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        write_review(root, report)
        print(json.dumps({'passed': report['passed'], 'functional_passed': report['functional_passed'],
                          'visual_score': report['visual_score'], 'report': str((root / 'report.json').resolve())}), flush=True)
    return 0 if report['passed'] or args.report_only else 1


if __name__ == '__main__':
    raise SystemExit(main())
