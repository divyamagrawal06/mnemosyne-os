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
        time.sleep(.12)

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

    def move(self, x, y):
        self.execute('input-send-event', {'events': [
            {'type': 'rel', 'data': {'axis': 'x', 'value': x}},
            {'type': 'rel', 'data': {'axis': 'y', 'value': y}}]})
        time.sleep(.05)

    def click(self):
        for down in (True, False):
            self.execute('input-send-event', {'events': [
                {'type': 'btn', 'data': {'button': 'left', 'down': down}}]})
            time.sleep(.1)

    def move_to(self, x, y):
        for _ in range(20):
            self.move(-100, -100)
        while x or y:
            dx, dy = min(x, 100), min(y, 100)
            self.move(dx, dy)
            x, y = x - dx, y - dy

    def close(self):
        self.stream.close()
        self.socket.close()


def wait_for(log, needle, timeout, after=0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        text = log.read_text(encoding='utf-8', errors='replace') if log.exists() else ''
        if 'user-panic:' in text or 'PANIC' in text:
            raise AssertionError(f'Guest panicked; inspect {log}')
        if needle == 'native-desktop-ready' and 'mnemosyne-os> ' in text:
            raise AssertionError(f'Guest fell back to serial console instead of desktop; inspect {log}')
        if needle in text[after:]:
            return text
        time.sleep(.1)
    raise AssertionError(f'Guest did not emit {needle!r}; inspect {log}')


def terminal_command(qmp, log, command, timeout, expected):
    """Require this command's exact echo and completed result, never earlier output."""
    cursor = len(log.read_text(errors='replace'))
    qmp.text(command)
    output = wait_for(log, '"event":"native-command-complete"', timeout, after=cursor)[cursor:]
    echo = 'mnemosyne-os> ' + command
    if echo not in output or '"ok":false' in output or expected not in output:
        raise AssertionError(f'Fresh command did not complete correctly: {command!r}; output={output!r}')
    return output


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
            vnc = None
            if boot == 1:
                for display in range(1, 100):
                    try:
                        with socket.socket() as reservation:
                            reservation.bind(('127.0.0.1', 5900 + display))
                        vnc = f'127.0.0.1:{display}'
                        break
                    except OSError:
                        continue
                if vnc is None:
                    raise RuntimeError('No unused loopback VNC display among 1..99')
            log = folder / f'boot-{boot}.log'
            command = runner.qemu_command(args.image, disk, log, vnc=vnc, qmp=f'127.0.0.1:{port}', headless=True)
            error_log = (folder / f'qemu-{boot}.log').open('wb')
            process = subprocess.Popen(command, stdout=error_log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                qmp = QMP(port, min(args.timeout, 30))
                wait_for(log, 'native-desktop-ready', args.timeout)
                if vnc:
                    with socket.create_connection(('127.0.0.1', 5900 + int(vnc.rsplit(':', 1)[1])), timeout=5) as viewer:
                        handshake = viewer.recv(12)
                    if not handshake.startswith(b'RFB '):
                        raise AssertionError('Loopback VNC did not return an RFB handshake')
                    report['steps'].append({'loopback-vnc-handshake': handshake.decode('ascii').strip()})
                time.sleep(.3)
                first = qmp.screenshot(folder / f'boot-{boot}.ppm')
                report['steps'].append({'boot': boot, 'framebuffer-sha256': first})
                if boot == 1:
                    qmp.move(40, 30)
                    time.sleep(.5)
                    second = qmp.screenshot(folder / 'pointer-moved.ppm')
                    if first == second:
                        raise AssertionError('PS/2 mouse movement did not change the framebuffer')
                    report['steps'].append({'mouse-movement-visible': True})
                    # PS/2 is relative. Clamp to the top-left then move to the
                    # native Tasks sidebar target; each delta fits one packet.
                    qmp.move_to(100, 250)
                    cursor = len(log.read_text(errors='replace'))
                    qmp.click()
                    wait_for(log, '"event":"desktop-view","view":"tasks"', args.timeout, after=cursor)
                    report['steps'].append({'ps2-click-opened-tasks': True})
                    qmp.move_to(928, 340)
                    for operation in ('stop', 'start'):
                        cursor = len(log.read_text(errors='replace'))
                        qmp.click()
                        wait_for(log, 'task-' + operation + ': pid=3', args.timeout, after=cursor)
                        time.sleep(.3)
                        qmp.screenshot(folder / ('task-' + operation + '.ppm'))
                        report['steps'].append({'native-task-control': operation, 'pid': 3, 'passed': True})
                    for function, name in [('f2', 'files'), ('f3', 'tasks'), ('f4', 'system'), ('f1', 'terminal')]:
                        qmp.key(function)
                        time.sleep(.3)
                        qmp.screenshot(folder / (name + '.ppm'))
                    qmp.key('f2')
                    cursor = len(log.read_text(errors='replace'))
                    qmp.text('desktop UI test')
                    wait_for(log, '"event":"desktop-file-save"', args.timeout, after=cursor)
                    wait_for(log, '"event":"native-command-complete","ok":true', args.timeout, after=cursor)
                    time.sleep(1)
                    qmp.screenshot(folder / 'native-file-editor.ppm')
                    report['steps'].append({'native-file-editor-save': True})
                    qmp.key('f1')
                    for operation, event in [('cap-demo', 'capability-demo'), ('fault-demo', 'fault-demo')]:
                        terminal_command(qmp, log, operation, args.timeout, '"event":"' + event + '","passed":true')
                        report['steps'].append({'guest-command': operation, 'passed': True})
                    terminal_command(qmp, log, 'write desktop-proof.txt survived-reboot', args.timeout, 'committed generation=')
                terminal_command(qmp, log, 'cat desktop-proof.txt', args.timeout, '\nsurvived-reboot\n')
                time.sleep(1)
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
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        report['finished-at'] = datetime.now(timezone.utc).isoformat()
        (folder / 'acceptance.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        print('Evidence:', folder, flush=True)
    print('Desktop acceptance passed: real framebuffer, PS/2 keyboard/mouse, clean shutdown, reboot persistence.')


if __name__ == '__main__':
    main()
