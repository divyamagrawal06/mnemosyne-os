"""Display the prebuilt MnemosyneOS framebuffer using native QEMU or loopback VNC."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def verify_image(image):
    record = json.loads(image.with_suffix('.json').read_text(encoding='utf-8'))
    if record.get('sha256') != hashlib.sha256(image.read_bytes()).hexdigest():
        raise ValueError('Desktop ISO checksum differs from provenance.')
    if record.get('profile') != 'release' or record.get('guest-desktop') is not True:
        raise ValueError('Expected a release desktop ISO with guest-desktop provenance.')
    return record


def disk_image(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        with path.open('xb') as stream:
            stream.truncate(16 * 1024 * 1024)
    if not path.is_file() or path.stat().st_size != 16 * 1024 * 1024:
        raise ValueError('Desktop disk must be a dedicated 16-MiB regular file; existing data was not reset.')


def qemu_command(image, disk, log, vnc=None, qmp=None, headless=False):
    if vnc is not None and not re.fullmatch(r'127\.0\.0\.1:(?:[0-9]|[1-9][0-9])', vnc):
        raise ValueError('--vnc must be a loopback display, for example 127.0.0.1:1')
    if qmp is not None and not re.fullmatch(r'127\.0\.0\.1:[0-9]{1,5}', qmp):
        raise ValueError('--qmp must be a loopback TCP endpoint')
    if qmp and not 1 <= int(qmp.rsplit(':', 1)[1]) <= 65535:
        raise ValueError('QMP port must be 1..65535')
    args = ['qemu-system-x86_64', '-machine', 'pc', '-accel', 'tcg,thread=single',
            '-cpu', 'qemu64', '-m', '256M', '-smp', '1', '-vga', 'std',
            '-cdrom', str(image.resolve()), '-drive',
            f"file={str(disk.resolve()).replace(',', ',,')},format=raw,if=ide,index=0,cache=writethrough",
            '-boot', 'd', '-serial', 'file:' + str(log.resolve()), '-monitor', 'none',
            '-nic', 'none', '-no-reboot', '-no-shutdown',
            '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04']
    if vnc:
        args += ['-display', 'none', '-vnc', vnc]
    elif headless:
        args += ['-display', 'none']
    if qmp:
        args += ['-qmp', 'tcp:' + qmp + ',server=on,wait=off']
    return args


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', type=Path, default=ROOT / 'build/mnemosyne-desktop-release.iso')
    parser.add_argument('--disk', type=Path, default=ROOT / 'build/desktop/chronicle-disk.img')
    parser.add_argument('--log', type=Path, default=ROOT / 'build/desktop/serial.log')
    parser.add_argument('--vnc', help='loopback VNC display, e.g. 127.0.0.1:1 (TCP port 5901)')
    parser.add_argument('--qmp', help='optional local test/control endpoint, e.g. 127.0.0.1:4444')
    parser.add_argument('--check', action='store_true', help='verify image and configuration without starting QEMU')
    args = parser.parse_args()
    if os.name == 'nt' and not shutil.which('qemu-system-x86_64'):
        def linux(path):
            value = path.resolve()
            return '/mnt/' + value.drive[0].lower() + value.as_posix()[2:]
        delegated = ['wsl', '-d', os.environ.get('MNEMOSYNE_WSL_DISTRO', 'Ubuntu'), '--exec',
                     'python3', linux(Path(__file__)), '--image', linux(args.image),
                     '--disk', linux(args.disk), '--log', linux(args.log)]
        for name in ('vnc', 'qmp'):
            if getattr(args, name):
                delegated += ['--' + name, getattr(args, name)]
        if args.check:
            delegated.append('--check')
        raise SystemExit(subprocess.call(delegated))
    try:
        record = verify_image(args.image)
        command = qemu_command(args.image, args.disk, args.log, args.vnc, args.qmp)
        if not shutil.which(command[0]):
            raise ValueError('Install qemu-system-x86 (Debian/Ubuntu) or add QEMU to PATH.')
        print(f"Desktop release {record.get('source-commit', 'unknown')}; x86-64, one TCG CPU, 256 MiB")
        if args.check:
            print('ISO and launch configuration verified.')
            return
        disk_image(args.disk)
        args.log.parent.mkdir(parents=True, exist_ok=True)
        print('Disk:', args.disk.resolve(), '\nSerial log:', args.log.resolve(), flush=True)
        if args.vnc:
            display = int(args.vnc.rsplit(':', 1)[1])
            print(f'VNC: 127.0.0.1:{5900 + display}; connect through SSH on remote hosts.', flush=True)
        process = subprocess.Popen(command, start_new_session=os.name != 'nt')
        def interrupted(signum, frame):
            raise KeyboardInterrupt
        signal.signal(signal.SIGTERM, interrupted)
        try:
            code = process.wait()
        except KeyboardInterrupt:
            print('Stopping QEMU; use the guest shutdown command for a clean exit.', flush=True)
            code = 0
        finally:
            if process.poll() is None:
                if os.name == 'nt':
                    process.terminate()
                else:
                    os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
        if code not in (0, 33):
            raise SystemExit(f'QEMU exited with status {code}; inspect {args.log}')
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
