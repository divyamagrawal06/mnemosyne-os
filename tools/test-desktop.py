"""Real QEMU/QMP keyboard, mouse, framebuffer and disk-persistence acceptance."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('desktop_runner', Path(__file__).with_name('run-desktop.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class QMP:
    def __init__(self, port, timeout):
        deadline = time.monotonic() + timeout
        while True:
            try:
                self.socket = socket.create_connection(('127.0.0.1', port), timeout=2)
                break
            except OSError:
                if time.monotonic() > deadline:
                    raise RuntimeError('QMP endpoint did not become ready')
                time.sleep(.1)
        self.socket.settimeout(15)
        self.stream = self.socket.makefile('rwb')
        self.stream.readline()
        self.execute('qmp_capabilities')

    def execute(self, name, arguments=None):
        message = {'execute': name}
        if arguments is not None:
            message['arguments'] = arguments
        self.stream.write(json.dumps(message).encode() + b'\n')
        self.stream.flush()
        while True:
            result = json.loads(self.stream.readline())
            if 'error' in result:
                raise RuntimeError(f'QMP {name}: {result["error"]}')
            if 'return' in result:
                return result['return']

    def key(self, name):
        self.execute('human-monitor-command', {'command-line': 'sendkey ' + name + ' 20'})
        time.sleep(.04)

    def text(self, value):
        special = {' ': 'spc', '.': 'dot', '-': 'minus', '_': 'shift-minus', '/': 'slash'}
        for character in value:
            key = special.get(character, character.lower())
            if character.isupper():
                key = 'shift-' + key
            self.key(key)
        self.key('ret')

    def screenshot(self, path):
        self.execute('screendump', {'filename': str(path.resolve())})
        data = path.read_bytes()
        if not data.startswith(b'P6\n') or len(data) < 10000 or len(set(data[-100000:])) < 8:
            raise AssertionError('Framebuffer screenshot is missing or has insufficient pixel variation')
        return hashlib.sha256(data).hexdigest()

    def close(self):
        self.stream.close()
        self.socket.close()


def wait_for(log, needle, timeout, after=0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        text = log.read_text(encoding='utf-8', errors='replace') if log.exists() else ''
        if needle in text[after:]:
            return text
        time.sleep(.1)
    raise AssertionError(f'Guest did not emit {needle!r}; inspect {log}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', type=Path, default=ROOT / 'build/mnemosyne-desktop-release.iso')
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    if os.name == 'nt':
        parser.error('Run this acceptance in Linux/Ubuntu WSL (QMP screenshot paths must belong to QEMU host).')
    if not 10 <= args.timeout <= 600:
        parser.error('--timeout must be 10..600')
    provenance = runner.verify_image(args.image)
    (ROOT / 'build').mkdir(exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='desktop-test-', dir=ROOT / 'build'))
    disk = folder / 'disk.img'
    runner.disk_image(disk)
    report = {'started-at': datetime.now(timezone.utc).isoformat(), 'passed': False,
              'image': provenance, 'steps': [], 'environment': 'actual QEMU TCG / QMP / native framebuffer'}
    process = qmp = None
    try:
        for boot in (1, 2):
            with socket.socket() as reservation:
                reservation.bind(('127.0.0.1', 0))
                port = reservation.getsockname()[1]
            log = folder / f'boot-{boot}.log'
            command = runner.qemu_command(args.image, disk, log, qmp=f'127.0.0.1:{port}', headless=True)
            error_log = (folder / f'qemu-{boot}.log').open('wb')
            process = subprocess.Popen(command, stdout=error_log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                qmp = QMP(port, min(args.timeout, 30))
                wait_for(log, 'desktop-ready', args.timeout)
                first = qmp.screenshot(folder / f'boot-{boot}.ppm')
                report['steps'].append({'boot': boot, 'framebuffer-sha256': first})
                if boot == 1:
                    qmp.execute('input-send-event', {'events': [
                        {'type': 'rel', 'data': {'axis': 'x', 'value': 40}},
                        {'type': 'rel', 'data': {'axis': 'y', 'value': 30}}]})
                    time.sleep(.5)
                    second = qmp.screenshot(folder / 'pointer-moved.ppm')
                    if first == second:
                        raise AssertionError('PS/2 mouse movement did not change the framebuffer')
                    report['steps'].append({'mouse-movement-visible': True})
                    qmp.text('write desktop-proof.txt survived-reboot')
                    wait_for(log, 'generation=', args.timeout)
                before = len(log.read_text(errors='replace'))
                qmp.text('cat desktop-proof.txt')
                wait_for(log, 'survived-reboot', args.timeout, after=before)
                report['steps'].append({'boot': boot, 'keyboard-file-read': True})
                qmp.screenshot(folder / f'file-read-{boot}.ppm')
                qmp.text('shutdown')
                code = process.wait(timeout=args.timeout)
                if code != 33:
                    raise AssertionError(f'Guest shutdown returned {code}, expected 33')
                report['steps'].append({'boot': boot, 'clean-shutdown': True})
            finally:
                if qmp:
                    qmp.close()
                    qmp = None
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                error_log.close()
                process = None
        report['passed'] = True
    finally:
        report['finished-at'] = datetime.now(timezone.utc).isoformat()
        (folder / 'acceptance.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        print('Evidence:', folder, flush=True)
    print('Desktop acceptance passed: real framebuffer, PS/2 keyboard/mouse, clean shutdown, reboot persistence.')


if __name__ == '__main__':
    main()
