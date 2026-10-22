"""Persistent, bounded serial control of the real x86-64 guest (stdlib only)."""
from copy import deepcopy
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import threading
import time
import uuid

PROMPT = 'mnemosyne-os> '
LIMIT = 131072
SIMPLE = set('help about ps uptime ls fs-info snapshots ipc-demo cap-demo fault-demo cow-demo wasm-demo demo self-test recover plan-show bench bench-suite'.split())
EVENT_COMMANDS = {'demo', 'self-test', 'bench-suite', 'ipc-demo', 'cap-demo', 'fault-demo', 'cow-demo', 'wasm-demo', 'recover'}
NAME = r'[A-Za-z0-9][A-Za-z0-9_.-]{0,47}'


def validate(command):
    if not isinstance(command, str) or not 1 <= len(command) <= 480 or any(ord(c) < 32 or ord(c) > 126 for c in command):
        raise ValueError('Command must be 1-480 printable ASCII characters.')
    command = command.strip()
    if PROMPT.strip() in command:
        raise ValueError("Guest prompt text cannot be embedded in a command.")
    if command in SIMPLE:
        return command
    patterns = [rf'(?:cat|rm|verify-plan|recover-plan|snapshot) {NAME}',
                rf'write {NAME} [ -~]+', r'restore [0-9]{1,18}', r'kv-add (?:[0-9]|1[0-5]) -?[0-9]{1,18}']
    if not any(re.fullmatch(p, command) for p in patterns):
        raise ValueError('Unsupported guest command or arguments. Type help for available commands.')
    return command


class GuestController:
    def __init__(self, root, timeout=120, popen=subprocess.Popen):
        self.root = Path(root)
        self.folder = self.root / 'build' / 'dashboard'
        self.disk = self.folder / 'guest-disk.img'
        self.iso = self.root / 'build' / 'mnemosyne-os-release.iso'
        self.timeout, self.popen = timeout, popen
        self.lock = threading.RLock()
        self.condition = threading.Condition(self.lock)
        self.process = None
        self.worker = None
        self.lockfile = None
        self.session = None
        self.closed = False
        self.stopping = False
        self.state = dict(status='stopped', busy=False, log='', log_truncated=False,
                          events=[], tasks=[], files=[], snapshots=[], last_command=None,
                          last_result=None, error=None, platform='x86-64 guest / QEMU TCG', persistent_disk=True)
        self.total = 0
        self.prompt_at = 0

    def snapshot(self):
        with self.lock:
            return deepcopy(self.state)

    def _append(self, text):
        with self.condition:
            self.total += len(text)
            combined = self.state['log'] + text
            self.state['log_truncated'] |= len(combined) > LIMIT
            self.state['log'] = combined[-LIMIT:]
            if self.state['log'].endswith(PROMPT):
                self.prompt_at = self.total
            self.condition.notify_all()

    def _args(self):
        def path(p):
            p = p.resolve()
            return '/mnt/' + p.drive[0].lower() + p.as_posix()[2:] if os.name == 'nt' else str(p)
        # A dedicated Linux process group is recorded so Windows WSL cleanup kills
        # precisely this QEMU, including on boot/command timeout.
        args = ['qemu-system-x86_64', '-machine', 'pc', '-accel', 'tcg,thread=single',
                '-cpu', 'qemu64', '-m', '256M', '-smp', '1', '-cdrom', path(self.iso),
                '-drive', f'file={path(self.disk)},format=raw,if=ide,index=0,cache=writethrough',
                '-boot', 'd', '-display', 'none', '-serial', 'stdio', '-monitor', 'none',
                '-nic', 'none', '-no-reboot', '-no-shutdown',
                '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04']
        if os.name == 'nt':
            # No user input enters this shell script; paths are separate argv.
            return ['wsl', '-d', os.environ.get('MNEMOSYNE_WSL_DISTRO', 'Ubuntu'), '--exec',
                    'setsid', '--wait', 'sh', '-c', 'echo $$ > "$1"; cancel=$2; shift 2; test ! -e "$cancel" || exit 125; exec "$@"', 'guest', path(self.pidfile), path(self.cancelfile), *args]
        return args

    def start(self):
        if self.process is not None and self.process.poll() is not None:
            self.stop()
        with self.lock:
            if self.closed:
                raise RuntimeError('Guest controller is closed.')
            if self.stopping or self.state['busy'] or self.process is not None:
                raise RuntimeError('A guest is already active.')
            if not self.iso.is_file():
                raise RuntimeError('Prebuilt build/mnemosyne-os-release.iso is missing. Build or install the release guest image first.')
            self.folder.mkdir(parents=True, exist_ok=True)
            lockfile = self.folder / 'guest-disk.lock'
            try:
                fd = os.open(lockfile, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
            except FileExistsError:
                raise RuntimeError('Guest disk is owned by another controller or an interrupted session. Stop that guest and remove build/dashboard/guest-disk.lock before retrying.') from None
            os.close(fd)
            self.lockfile = lockfile
            try:
                if not self.disk.exists():
                    with self.disk.open('xb') as disk:
                        disk.truncate(16 * 1024 * 1024)
                if self.disk.stat().st_size != 16 * 1024 * 1024:
                    raise RuntimeError('Dashboard guest disk must be exactly 16 MiB.')
                self.pidfile = self.folder / ('guest-' + uuid.uuid4().hex + '.pid')
                self.cancelfile = self.pidfile.with_suffix('.cancel')
                self.state.update(status='starting', busy=True, log='', log_truncated=False,
                                  events=[], tasks=[], files=[], snapshots=[], error=None, last_result=None, last_command=None)
                self.total = self.prompt_at = 0
                options = {'creationflags': subprocess.CREATE_NO_WINDOW} if os.name == 'nt' else {'start_new_session': True}
                self.process = self.popen(self._args(), cwd=self.root, stdin=subprocess.PIPE,
                                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, **options)
                self.session = uuid.uuid4().hex
                threading.Thread(target=self._read, args=(self.process, self.session), daemon=True).start()
                self.worker = threading.Thread(target=self._boot, daemon=True)
                self.worker.start()
            except Exception:
                self.lockfile.unlink(missing_ok=True)
                self.state.update(status='error', busy=False)
                raise

    def _read(self, process, session):
        try:
            while True:
                data = process.stdout.read(1)
                if not data:
                    break
                with self.lock:
                    if self.session != session:
                        return
                    self._append(data.decode('utf-8', errors='replace'))
            code = process.wait()
            with self.condition:
                if self.session == session:
                    self.state.update(status='error', busy=False, error=f'Guest exited unexpectedly (status {code}). Start it again to reuse its disk.')
                    self.condition.notify_all()
        except (OSError, ValueError):
            pass

    def _expect(self, after):
        deadline = time.monotonic() + self.timeout
        with self.condition:
            while self.prompt_at <= after:
                if self.process is None or self.process.poll() is not None:
                    raise RuntimeError('Guest exited before returning a prompt.')
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError('Guest timed out; its process has been stopped. Restart to reuse the persistent disk.')
                self.condition.wait(min(remaining, .2))

    def _exchange(self, command):
        with self.lock:
            after = self.total
            self.process.stdin.write(command.encode('ascii') + b'\r')
            self.process.stdin.flush()
        self._expect(after)
        with self.lock:
            output = self.state['log'][-min(self.total - after, LIMIT):]
            return output.removesuffix(PROMPT)

    def _refresh(self):
        for command in ('ps', 'ls', 'snapshots'):
            output = self._exchange(command)
            with self.lock:
                if command == 'ps':
                    rows = re.findall(r'(?m)^(\d+)\s+(\S+)\s+(ready|receive|sleep|dead)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)', output)
                    self.state['tasks'] = [dict(pid=int(r[0]), name=r[1], state=r[2], switches=int(r[3]), preemptions=int(r[4]), faults=int(r[5]), epoch=int(r[6])) for r in rows]
                elif command == 'ls':
                    self.state['files'] = [dict(name=n, bytes=int(b)) for n,b in re.findall(r'(?m)^(\S+)  (\d+) bytes', output)]
                else:
                    self.state['snapshots'] = [dict(generation=int(g), label=l, bytes=int(b)) for g,l,b in re.findall(r'generation=(\d+) label=(\S*) bytes=(\d+)', output)]

    def _boot(self):
        try:
            self._expect(0)
            self._refresh()
            with self.lock:
                if self.process is not None and not self.stopping:
                    self.state.update(status='ready', busy=False)
        except Exception as error:
            self._fail(error)

    def command(self, command):
        command = validate(command)
        with self.lock:
            if self.state['status'] != 'ready' or self.state['busy']:
                raise RuntimeError('Guest must be ready before sending a command.')
            self.state.update(status='busy', busy=True, last_command=command, last_result=None, error=None)
            if command in EVENT_COMMANDS:
                self.state.update(events=[], event_command=command)
            self.worker = threading.Thread(target=self._command, args=(command,), daemon=True)
            self.worker.start()

    def _command(self, command):
        try:
            output = self._exchange(command)
            with self.lock:
                self.state['last_result'] = dict(command=command, output=output, ok=not bool(re.search(r'(?m)^error:', output)))
                events = []
                for line in (output.splitlines() if command in EVENT_COMMANDS else []):
                    try:
                        obj = json.loads(line)
                        if isinstance(obj, dict) and 'event' in obj:
                            events.append(obj)
                    except ValueError:
                        pass
                self.state['events'] = (self.state['events'] + events)[-128:]
            self._refresh()
            with self.lock:
                if self.process is not None and not self.stopping:
                    self.state.update(status='ready', busy=False)
        except Exception as error:
            self._fail(error)

    def _fail(self, error):
        with self.lock:
            if self.process is None or self.stopping:
                return
        self.stop(error=str(error))

    def stop(self, error=None):
        with self.lock:
            if self.stopping:
                return
            self.stopping = True
            graceful = self.state['status'] == 'ready' and not self.state['busy']
            self.state.update(status='stopping', busy=True)
            process = self.process
            self.session = None
            self.process = None
            self.condition.notify_all()
        try:
            if process is not None:
                if graceful and process.poll() is None:
                    try:
                        process.stdin.write(b'shutdown\r')
                        process.stdin.flush()
                        process.wait(timeout=10)
                    except (OSError, subprocess.SubprocessError):
                        pass
                if os.name == 'nt' and process.poll() is None:
                    self.cancelfile.touch()
                    deadline = time.monotonic() + 2
                    while not self.pidfile.exists() and process.poll() is None and time.monotonic() < deadline:
                        time.sleep(.02)
                    try:
                        pid = self.pidfile.read_text().strip()
                        if re.fullmatch(r'[1-9][0-9]*', pid):
                            subprocess.run(['wsl', '-d', os.environ.get('MNEMOSYNE_WSL_DISTRO', 'Ubuntu'), '--exec', 'kill', '-KILL', '--', '-' + pid], capture_output=True, timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
                    except (OSError, subprocess.SubprocessError):
                        pass
                    if process.poll() is None:
                        subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], capture_output=True, timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
                elif process.poll() is None:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=10)
                for stream in (process.stdin, process.stdout):
                    if stream:
                        stream.close()
        except (OSError, subprocess.SubprocessError) as failure:
            # Keep disk ownership if termination cannot be confirmed. A retry of
            # Stop is safe; another guest must never reuse a possibly live disk.
            with self.lock:
                self.process = process
                self.state.update(status='error', busy=False, error=f'Guest cleanup failed: {failure}')
                self.stopping = False
            raise RuntimeError(f'Guest cleanup failed: {failure}') from failure
        if self.worker and self.worker is not threading.current_thread():
            self.worker.join(timeout=12)
        with self.lock:
            if self.lockfile:
                self.lockfile.unlink(missing_ok=True)
                self.lockfile = None
            if hasattr(self, 'pidfile'):
                had_pid = self.pidfile.exists()
                self.pidfile.unlink(missing_ok=True)
                if had_pid:
                    self.cancelfile.unlink(missing_ok=True)
            self.state.update(status='error' if error else 'stopped', busy=False, error=error)
            self.stopping = False

    def close(self):
        self.closed = True
        self.stop()
        if self.worker and self.worker is not threading.current_thread():
            self.worker.join(timeout=12)
