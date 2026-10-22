"""Exercise the real dashboard HTTP API and QEMU on an isolated persistent disk.

Requires the prebuilt release ISO and QEMU (or Ubuntu WSL on Windows).
No Rust build, cloud resources, browser mocks or saved success logs are used.
The separate browser walkthrough validates DOM interactions against this API.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import threading
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import dashboard


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    if not 10 <= args.timeout <= 600:
        parser.error('timeout must be 10..600 seconds per guest action')
    root = Path(__file__).resolve().parent.parent
    iso = root / 'build/mnemosyne-os-release.iso'
    if not iso.is_file():
        parser.error('Install the prebuilt bundle or build the release OS image first')
    isolated = Path(tempfile.mkdtemp(prefix='guest-e2e-', dir=root / 'build'))
    (isolated / 'build').mkdir()
    shutil.copy2(iso, isolated / 'build' / iso.name)
    shutil.copy2(iso.with_suffix('.json'), isolated / 'build' / iso.with_suffix('.json').name)
    shutil.copytree(root / 'ui', isolated / 'ui')
    server = dashboard.make_server(0, manager=dashboard.RunManager(isolated), guest_timeout=args.timeout)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    origin = f'http://127.0.0.1:{server.server_port}'
    token = server.manager.token
    evidence = {'started-at': datetime.now(timezone.utc).isoformat(),
                'iso-sha256': hashlib.sha256(iso.read_bytes()).hexdigest(),
                'environment': 'actual HTTP server and QEMU TCG', 'steps': [], 'passed': False}

    def request(path, body=None):
        headers = {'Origin': origin, 'X-Mnemosyne-Token': token}
        if body is not None:
            headers['Content-Type'] = 'application/json'
        req = Request(origin + path, data=None if body is None else json.dumps(body).encode(), headers=headers)
        with urlopen(req, timeout=30) as response:
            raw = response.read()
            return json.loads(raw) if 'application/json' in response.headers['Content-Type'] else raw

    def ready():
        deadline = time.monotonic() + args.timeout * 4 + 10
        while time.monotonic() < deadline:
            state = request('/api/guest')
            if state['status'] == 'error':
                raise AssertionError(state.get('error'))
            if state['status'] == 'ready' and not state['busy']:
                return state
            time.sleep(.1)
        raise AssertionError('HTTP guest action did not complete')

    def command(value, expected=True):
        request('/api/guest/command', {'command': value})
        state = ready()
        result = state['last_result']
        assert result and result['command'] == value, state
        assert result['ok'] == expected, result
        evidence['steps'].append(result)
        print(value.split(' ', 1)[0] + ': checked', flush=True)
        return state

    try:
        assert b'guest-app.js' in request('/')
        assert len(request('/guest-app.js')) > 100
        request('/api/guest/start', {})
        assert len(ready()['tasks']) == 4
        state = command('demo')
        required = {'ipc-demo', 'capability-demo', 'fault-demo', 'cow-replay',
                    'filesystem-demo', 'wasm-recovery', 'guest-demo'}
        assert required <= {e['event'] for e in state['events'] if e.get('passed') is True}
        state = command('bench-suite')
        samples = [e for e in state['events'] if e.get('event') == 'benchmark-sample']
        assert len(samples) == 35 and len({e['benchmark'] for e in samples}) == 7
        evidence['benchmark-samples'] = samples
        command('write e2e-note.txt survives-reboot')
        state = command('snapshot e2e-before-edit')
        generation = next(s['generation'] for s in state['snapshots'] if s['label'] == 'e2e-before-edit')
        command('write e2e-note.txt changed')
        command(f'restore {generation}')
        assert 'survives-reboot' in command('cat e2e-note.txt')['last_result']['output']
        state = command('plan-show')
        plan = next(json.loads(line) for line in state['last_result']['output'].splitlines() if line.startswith('{'))
        plan['requested-rights'] = 255
        command('write e2e-plan.json ' + json.dumps(plan, separators=(',', ':')))
        state = command('verify-plan e2e-plan.json', expected=False)
        assert 'PrivilegeAmplification' in state['last_result']['output']
        try:
            request('/api/guest/command', {'command': 'write injected.txt hello\nshutdown'})
            raise AssertionError('newline injection accepted')
        except HTTPError as error:
            assert error.code == 400
        request('/api/guest/stop', {})
        assert request('/api/guest')['status'] == 'stopped'
        request('/api/guest/start', {})
        ready()
        assert 'survives-reboot' in command('cat e2e-note.txt')['last_result']['output']
        command('recover')
        assert 'value=180' in command('kv-add 1 2')['last_result']['output']
        exported = request('/api/guest/export')
        assert 'token' not in exported
        assert exported['tasks'] and exported['files']
        evidence['final-state'] = exported
        evidence['passed'] = True
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        evidence['finished-at'] = datetime.now(timezone.utc).isoformat()
        destination = isolated / 'acceptance.json'
        destination.write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
        print('Evidence: ' + str(destination), flush=True)
    print('HTTP-to-guest acceptance passed: demo, benchmarks, filesystem restore, policy rejection, reboot and live recovery.')


if __name__ == '__main__':
    main()
