#!/usr/bin/env python3
"""Drive the actual MiniAppsPanel against the disposable ADR Matrix server.

All mutations use native OctoScript controls. This runner never manufactures
provider observations, Matrix membership, approval receipts or readiness.
Requires the palpo-instrument palpo_local example and private fixture accounts.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import time
import urllib.parse
import urllib.request
import uuid

from native_palpo import PalpoApp, fill, port


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def private_accounts(path, role):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > 65536 or metadata.st_mode & 0o077:
        raise ValueError('Accounts must be a private regular file of at most 64 KiB')
    accounts = json.loads(path.read_text())
    user = accounts[role]['user_id']
    if not user.endswith(':rinx-adr0011.test'):
        raise ValueError('Only isolated ADR server identities are supported')
    return user


def scroll(app, delta):
    app.request('/m', k='scroll', x=500, y=680, dy=delta, wait=1)


class Readback:
    """Verify native mutations through independent authenticated read services."""
    def __init__(self, origin, accounts, role):
        self.origin = origin.rstrip('/')
        self.token = json.loads(accounts.read_text())[role]['access_token']
        result = self.request('/session', {'appId': 'im.palpo.operations',
            'bundleDigest': 'a' * 64, 'services': ['palpo.requests.list', 'palpo.inbox.get', 'palpo.inbox.list', 'palpo.fleets.list']})
        self.token = result['sessionToken']

    def request(self, path, body):
        request = urllib.request.Request(self.origin + '/_palpo/miniapp/v1' + path,
            data=json.dumps(body).encode(), headers={'Content-Type': 'application/json',
            'Authorization': 'Bearer ' + self.token})
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)

    def call(self, service, args):
        return self.request('/call', {'service': service, 'args': args})

    def agent(self, name):
        found = []
        offset = 0
        while True:
            page = self.call('palpo.requests.list', {'offset': offset, 'limit': 100})
            found.extend(a for a in page['requests'] if a.get('agentDefinition', {}).get('name') == name)
            offset += len(page['requests'])
            if offset >= page['total'] or not page['requests']:
                break
        if len(found) > 1:
            raise AssertionError('Agent name is ambiguous; use a unique acceptance name')
        return found[0] if found else None

    def fleet(self, name):
        found = [f for f in self.call('palpo.fleets.list', {})['fleets'] if f['name'] == name]
        if len(found) != 1:
            raise AssertionError('Engagement name must identify exactly one visible engagement')
        return found[0]


def create_agent(app, project, name, tokens):
    app.click_id('projects')
    app.wait_text('Project rooms verified', timeout=30)
    scroll(app, -10000)
    for _ in range(50):
        tree = app.request('/snap', all=1)['s']
        labels = [i for i, w in enumerate(tree) if w.get('t') == project]
        if labels:
            start = labels[0]
            end = next((i + start + 1 for i, w in enumerate(tree[start + 1:])
                        if w.get('i') == 'agent'), len(tree))
            if end < len(tree):
                x, y, width, height = tree[end]['r']
                if width > 0 and 160 <= y and y + height <= 770:
                    app.click(x + width / 2, y + height / 2)
                    break
        scroll(app, 110)
    else:
        raise AssertionError('Selected project request button unavailable')
    app.wait_text('Use this resource')
    app.click_id('choose')
    app.wait_text('Agent name')
    fill(app, 'Agent name', name)
    scroll(app, 200)
    fill(app, 'Initial tokens', str(tokens))
    app.capture('agent-request-form')
    app.click_id('review_form'); app.wait_text('3 · Review and send')
    app.click_id('submit')


def create_project(app, fleet, name, submit=True):
    app.click_id('resources')
    app.wait_text('Request project here', timeout=30)
    scroll(app, -10000)
    for _ in range(60):
        tree = app.request('/snap', all=1)['s']
        start = next((i for i, w in enumerate(tree) if w.get('t') == fleet), None)
        if start is not None:
            target = next((w for w in tree[start + 1:] if w.get('i') == 'choose'), None)
            if target:
                x, y, width, height = target['r']
                if width > 0 and 160 <= y and y + height <= 770:
                    app.click(x + width / 2, y + height / 2)
                    break
        scroll(app, 130)
    else:
        raise AssertionError('Selected engagement resource unavailable')
    app.wait_text('Project name')
    fill(app, 'Project name', name)
    fill(app, 'What will your project do?', 'Isolated workflow validation')
    app.capture('project-request-form')
    if submit:
        app.click_id('review_form'); app.wait_text('3 · Review and send')
        app.click_id('submit')


def agent_card(app, name, button=None):
    scroll(app, -10000)
    for _ in range(50):
        tree = app.request('/snap', all=1)['s']
        names = [(index, w) for index, w in enumerate(tree) if w.get('t') == name]
        if names:
            index, label = names[0]
            top = label['r'][1]
            # Every agent card ends with its own result button. Use that bound
            # even when another card's same-named action is visible below it.
            end = next((i + index + 1 for i, w in enumerate(tree[index + 1:])
                        if w.get('i') == 'agent_result'), len(tree))
            if button:
                targets = [w for w in tree[index:end + 1] if w.get('i') == button
                           and w['r'][2] > 0
                           and 160 <= w['r'][1] and w['r'][1] + w['r'][3] <= 770]
                if targets:
                    x, y, width, height = targets[0]['r']
                    app.click(x + width / 2, y + height / 2)
                    return
            elif 150 <= top <= 550:
                return
        scroll(app, 110)
    raise AssertionError(f'Agent card/control unavailable: {name}, {button}')


def rotate_fleet(app, name, fleet_names):
    app.click_id('fleets')
    app.wait_text('Renew credentials', timeout=30)
    scroll(app, -10000)
    for _ in range(50):
        tree = app.request('/snap', all=1)['s']
        start = next((i for i, w in enumerate(tree) if w.get('t') == name), None)
        if start is not None:
            end = next((i for i in range(start + 1, len(tree)) if tree[i].get('t') in fleet_names), len(tree))
            target = next((w for w in tree[start:end] if w.get('i') == 'rotate_fleet'), None)
            if target:
                x, y, width, height = target['r']
                if width > 0 and 160 <= y and y + height <= 770:
                    app.click(x + width / 2, y + height / 2)
                    break
        scroll(app, 110)
    else:
        raise AssertionError('Selected engagement renewal control unavailable')
    app.wait_text('Renew connection credentials')
    app.capture('renewal-form')
    app.click_id('submit')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--accounts', type=Path, required=True)
    parser.add_argument('--origin', default='http://127.0.0.1:24682')
    parser.add_argument('--role', choices=['manager', 'owner', 'coordinator', 'admin'], default='manager')
    parser.add_argument('--scenario', choices=['inspect', 'project', 'room-picker', 'rotate', 'create', 'top-up', 'approve', 'reject', 'pause', 'resume', 'retire'], required=True)
    parser.add_argument('--agent')
    parser.add_argument('--project')
    parser.add_argument('--fleet')
    parser.add_argument('--room-name')
    parser.add_argument('--action-id')
    parser.add_argument('--tokens', type=int, default=10000)
    parser.add_argument('--output', type=Path, default=Path('target/palpo-live-validation'))
    args = parser.parse_args()
    origin = urllib.parse.urlsplit(args.origin)
    if origin.scheme != 'http' or origin.hostname != '127.0.0.1' or origin.path not in ('', '/') or origin.username or origin.password or origin.query or origin.fragment:
        parser.error('Origin must be the isolated loopback HTTP server')
    if args.scenario in ('approve', 'reject'):
        if not args.action_id:
            parser.error('A concrete action ID is required for a decision')
    elif args.scenario == 'rotate':
        if not args.fleet:
            parser.error('A concrete engagement name is required')
    elif args.scenario in ('project', 'room-picker'):
        if not args.project or not args.fleet:
            parser.error('Concrete engagement and project names are required')
    elif not args.agent:
        parser.error('A concrete agent name is required')
    if args.scenario == 'room-picker' and not args.room_name:
        parser.error('An existing eligible room name is required')
    if args.tokens <= 0:
        parser.error('Tokens must be positive')
    if args.scenario == 'create' and not args.project:
        parser.error('A concrete project name is required')
    user = private_accounts(args.accounts, args.role)
    readback = Readback(args.origin, args.accounts, args.role)
    before = readback.agent(args.agent) if args.agent else None
    before_fleet = readback.fleet(args.fleet) if args.scenario == 'rotate' else None
    if before_fleet and not before_fleet['canRotate']:
        parser.error('Current account cannot rotate the selected engagement')
    before_actions = {a['id'] for a in readback.call('palpo.inbox.list', {'view': 'all', 'limit': 100}).get('actions', [])}
    if args.scenario == 'create' and before:
        parser.error('Choose a new unique agent name')
    root = (args.output / uuid.uuid4().hex).resolve()
    root.mkdir(parents=True, mode=0o700)
    os.chmod(root, 0o700)
    # Never let another Cargo build replace an executable during this run.
    binary = root / 'palpo_local'
    shutil.copy2(args.binary.resolve(), binary)
    binary_hash = digest(binary)
    (root / 'profile').mkdir(mode=0o700)
    app = PalpoApp(root, port=port(), auto_login=False)
    app.output.mkdir(parents=True, mode=0o700)
    app.log = (app.output / 'native.log').open('w')
    env = dict(os.environ, RINX_DATA_DIR=str(root / 'profile'), MAKEPAD_HIDE_WINDOWS='1',
               MAKEPAD_NO_FOCUS='1', MAKEPAD_REMOTE=str(app.port), PALPO_FIXTURE_URL=args.origin,
               PALPO_ACCEPTANCE_ACCOUNTS=str(args.accounts.resolve()), PALPO_ACCEPTANCE_ROLE=args.role,
               PALPO_ACCEPTANCE_ACTION=args.action_id or '')
    env.pop('MAKEPAD_FOCUS', None)
    report = {'passed': False, 'scenario': args.scenario, 'account': user,
              'binarySha256': binary_hash, 'checks': [], 'evidence': str(app.output)}
    print(root, flush=True)
    app.process = subprocess.Popen([str(binary)], env=env, stdout=app.log, stderr=subprocess.STDOUT)
    try:
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            if app.process.poll() is not None:
                raise RuntimeError('Native process exited during startup')
            try:
                if app.request('/s')['w']:
                    break
            except OSError:
                pass
            time.sleep(.1)
        else:
            raise TimeoutError('Native bridge did not start')
        if args.scenario in ('approve', 'reject'):
            app.wait_text('Approve' if args.scenario == 'approve' else 'Reject', timeout=40)
            app.capture('exact-action-review')
            app.click_id(args.scenario)
            app.wait_text('Decision reason')
            fill(app, 'Decision reason', 'Isolated ADR acceptance decision')
            app.click_id('submit')
            app.wait_text('Pending actions stay here', timeout=30)
            report['checks'].append('Native decision returned to durable Inbox')
        elif args.scenario == 'rotate':
            app.wait_text('Pending actions stay here', timeout=40)
            names = {f['name'] for f in readback.call('palpo.fleets.list', {})['fleets']}
            rotate_fleet(app, args.fleet, names)
        elif args.scenario == 'room-picker':
            app.wait_text('Pending actions stay here', timeout=40)
            create_project(app, args.fleet, args.project, submit=False)
            app.click_id('choose_room')
            app.wait_text('Choose a project room')
            app.click_id('rooms')
            app.wait_text(args.room_name, pixels=True)
            app.capture('eligible-room-menu')
            app.click_text(args.room_name)
            app.click_id('change_theme')
            app.wait_text(args.room_name)
            app.capture('room-selection-dark')
            app.click_id('use_room')
            scroll(app, -10000)
            app.wait_text('Project room: ' + args.room_name)
            app.capture('selected-room-form')
            app.click_id('choose_room')
            app.click_id('cancel_room')
            scroll(app, -10000)
            app.wait_text('Project room: ' + args.room_name)
            app.click_id('choose_room')
            app.click_id('switch_account')
            app.wait_text('Session ended. Review and run again.', timeout=30)
            assert not any(w.get('i') == 'use_room' and w['r'][3] > 0 for w in app.request('/snap', all=1)['s'])
            report['checks'].append('Native eligible room selection, live theme, cancel and account revocation')
        elif args.scenario == 'project':
            app.wait_text('Pending actions stay here', timeout=40)
            create_project(app, args.fleet, args.project)
        elif args.scenario == 'create':
            app.wait_text('Pending actions stay here', timeout=40)
            create_agent(app, args.project, args.agent, args.tokens)
        else:
            app.wait_text('Pending actions stay here', timeout=40)
            app.click_id('requests')
            app.wait_text('Agent provisioning and connection readiness are verified by the server.', timeout=30)
            agent_card(app, args.agent)
            app.capture('agent-before')
            if args.scenario != 'inspect':
                action = {'top-up': 'more_tokens', 'pause': 'pause_agent',
                          'resume': 'resume_agent', 'retire': 'remove_agent'}[args.scenario]
                agent_card(app, args.agent, action)
                if args.scenario == 'top-up':
                    app.wait_text('Additional tokens')
                    fill(app, 'Additional tokens', str(args.tokens))
                app.capture('intent-form')
                app.click_id('submit')
                # Do not wait for a transient Saved toast or another card's
                # off-screen status. The form must close after the real reply.
                deadline = time.monotonic() + 40
                while time.monotonic() < deadline:
                    visible = [w for w in app.request('/snap', all=1)['s']
                               if w.get('i') == 'submit' and w['r'][3] > 0]
                    if not visible:
                        break
                    time.sleep(.2)
                else:
                    raise AssertionError('Native mutation form did not complete')
                report['checks'].append('Native mutation form completed')
            report['checks'].append('Actual agent card rendered in production MiniAppsPanel')
        deadline = time.monotonic() + 45
        while True:
            if args.scenario in ('approve', 'reject'):
                result = readback.call('palpo.inbox.get', {'id': args.action_id})['action']
                verified = result['state'] == ('approved' if args.scenario == 'approve' else 'rejected')
            elif args.scenario == 'rotate':
                result = readback.fleet(args.fleet)
                verified = result['id'] == before_fleet['id'] and result['transportGeneration'] == before_fleet['transportGeneration'] + 1 and result['registrationGeneration'] == before_fleet['registrationGeneration'] and not result['connectionVerified']
            elif args.scenario == 'room-picker':
                actions = readback.call('palpo.inbox.list', {'view': 'all', 'limit': 100}).get('actions', [])
                result = {'unchangedActionIds': {a['id'] for a in actions} == before_actions}
                verified = result['unchangedActionIds']
            elif args.scenario == 'project':
                actions = readback.call('palpo.inbox.list', {'view': 'all', 'limit': 100}).get('actions', [])
                result = next((a for a in actions if a['id'] not in before_actions and a.get('kind') == 'project' and a.get('payload', {}).get('name') == args.project), None)
                verified = result is not None and result['state'] == 'requested'
            else:
                result = readback.agent(args.agent)
                verified = result is not None
                if verified and args.scenario == 'create':
                    verified = result['state'] == 'requested' and result['requestedTokens'] == args.tokens
                elif verified and args.scenario in ('pause', 'resume', 'retire'):
                    control = result.get('agentControl') or {}
                    verified = control.get('operation') == {'pause': 'stop', 'resume': 'start', 'retire': 'retire'}[args.scenario]
                    verified = verified and control != (before or {}).get('agentControl')
                elif verified and args.scenario == 'top-up':
                    actions = readback.call('palpo.inbox.list', {'view': 'all', 'limit': 100}).get('actions', [])
                    allocation = result.get('lifecycle', {}).get('settlement', {}).get('agentAllocationId')
                    verified = any(a['id'] not in before_actions and a.get('kind') == 'token_top_up' and a.get('payload', {}).get('agentAllocationId') == allocation
                        and str(a.get('payload', {}).get('requestedAdditionalTokens')) == str(args.tokens) for a in actions)
            if verified:
                break
            if time.monotonic() >= deadline:
                raise AssertionError('Durable backend state did not confirm the native action')
            time.sleep(.3)
        report['backend'] = result
        report['checks'].append('Independent authenticated read confirmed durable result')
        app.capture('result')
        assert digest(binary) == binary_hash, 'Native executable changed during run'
        report['passed'] = True
    except Exception as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
        try:
            app.capture('failure')
            (root / 'failure-widgets.json').write_text(json.dumps(app.request('/snap', all=1), indent=2))
        except Exception:
            pass
        raise
    finally:
        app.stop()
        (root / 'report.json').write_text(json.dumps(report, indent=2))
        (root / 'trace.json').write_text(json.dumps(app.trace, indent=2))
        binary.unlink()
    print('PASS native ' + args.scenario, flush=True)


if __name__ == '__main__':
    main()
