"""Small native-Rust + Linux/WSL QEMU development runner; no cloud services."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import struct
import queue
import threading
import time
import hashlib

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
LIMINE = ROOT / ".tools" / "limine"
LIMINE_TAG = "v10.8.5-binary"
LIMINE_COMMIT = "7a9013f305de0bea1f6310e76e7baba30499fef0"
TARGET = "x86_64-unknown-none"


def run(args, **kwargs):
    print("+ " + " ".join(map(str, args)), flush=True)
    return subprocess.run(list(map(str, args)), cwd=ROOT, check=True, **kwargs)


def linux_path(path):
    path = Path(path).resolve()
    if os.name == "nt":
        return "/mnt/" + path.drive[0].lower() + path.as_posix()[2:]
    return str(path)


def linux_command(args):
    if os.name == "nt":
        return ["wsl", "-d", os.environ.get("MNEMOSYNE_WSL_DISTRO", "Ubuntu"), "--", *args]
    return args


def cargo(*args):
    run(["cargo", *args])


def build(release=False):
    cargo("build", "--locked", "-p", "mnemosyne-user-init", "--target", TARGET,
          *(["--release"] if release else []))
    # Embed only ELF load segments, not host debug paths/section tables.
    profile = "release" if release else "debug"
    data = bytearray((ROOT / "target" / TARGET / profile / "mnemosyne-user-init").read_bytes())
    phoff = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    end = phoff + size * count
    for i in range(count):
        at = phoff + i * size
        if struct.unpack_from("<I", data, at)[0] == 1:
            offset = struct.unpack_from("<Q", data, at + 8)[0]
            length = struct.unpack_from("<Q", data, at + 32)[0]
            end = max(end, offset + length)
    struct.pack_into("<Q", data, 40, 0)
    struct.pack_into("<HHH", data, 58, 0, 0, 0)
    BUILD.mkdir(parents=True, exist_ok=True)
    packed = BUILD / "user-init.elf"
    payload = data[:end]
    if not packed.exists() or packed.read_bytes() != payload:
        packed.write_bytes(payload)
    cargo("build", "--locked", "-p", "mnemosyne-kernel", "--target", TARGET,
          *(["--release"] if release else []))


def bootloader():
    if not LIMINE.exists():
        LIMINE.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--depth", "1", "--branch", LIMINE_TAG,
             "https://github.com/limine-bootloader/limine.git", LIMINE])
    revision = subprocess.check_output(["git", "-C", str(LIMINE), "rev-parse", "HEAD"], text=True).strip()
    if revision != LIMINE_COMMIT:
        raise RuntimeError(f"Limine revision mismatch: {revision}")
    run(linux_command(["make", "-C", linux_path(LIMINE), "-j2"]))


def image(release=False, shell=False):
    build(release)
    bootloader()
    suffix = ("-os" if shell else "") + ("-release" if release else "")
    profile = "release" if release else "debug"
    stage = BUILD / ("iso-root" + suffix)
    boot = stage / "boot"
    loader = boot / "limine"
    efi = stage / "EFI" / "BOOT"
    for folder in (loader, efi):
        folder.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "target" / TARGET / profile / "mnemosyne-kernel", boot / "mnemosyne-kernel")
    config = (ROOT / "boot" / "limine.conf").read_text()
    if shell:
        config = config.replace("MnemosyneOS smoke test", "mnemosyne-os") + "    cmdline: shell\n"
    (boot / "limine.conf").write_text(config)
    for name in ("limine-bios.sys", "limine-bios-cd.bin", "limine-uefi-cd.bin", "LICENSE"):
        shutil.copyfile(LIMINE / name, loader / name)
    shutil.copyfile(LIMINE / "BOOTX64.EFI", efi / "BOOTX64.EFI")
    iso = BUILD / ("mnemosyne" + suffix + ".iso")
    run(linux_command(["xorriso", "-as", "mkisofs", "-b", "boot/limine/limine-bios-cd.bin",
         "-no-emul-boot", "-boot-load-size", "4", "-boot-info-table", "--efi-boot",
         "boot/limine/limine-uefi-cd.bin", "-efi-boot-part", "--efi-boot-image",
         "--protective-msdos-label", linux_path(stage), "-o", linux_path(iso)]))
    run(linux_command([linux_path(LIMINE / "limine"), "bios-install", linux_path(iso)]))
    provenance = {
        "source-commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source-dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()),
        "sha256": hashlib.sha256(iso.read_bytes()).hexdigest(),
        "profile": profile, "guest-shell": shell,
    }
    iso.with_suffix(".json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    return iso


def smoke(release=False):
    iso = image(release)
    command = linux_command(["timeout", "--signal=KILL", "25s", "qemu-system-x86_64",
        "-machine", "q35", "-accel", "tcg,thread=single", "-cpu", "qemu64",
        "-m", "256M", "-smp", "1", "-cdrom", linux_path(iso), "-display", "none",
        "-serial", "stdio", "-monitor", "none", "-nic", "none", "-no-reboot", "-no-shutdown",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
    print("+ " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=35)
    output = result.stdout + result.stderr
    log_path = BUILD / ("boot-release.log" if release else "boot.log")
    log_path.write_text(output, encoding="utf-8")
    print(output, end="")
    events = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{"event":')]
    required = {"boot", "memory", "frames_ok", "capabilities_ok", "paging_ok",
                "exception_ok", "timer_ok", "boot_ok"}
    if result.returncode != 33 or not required <= {e["event"] for e in events}:
        raise RuntimeError(f"kernel smoke failed (QEMU status {result.returncode}); see {log_path}")
    print("Kernel smoke passed: frames, page permissions, capabilities, exceptions, and timer.")


def test():
    cargo("test", "--locked")


def check():
    cargo("fmt", "--all", "--", "--check")
    cargo("clippy", "--locked", "--all-targets", "--", "-D", "warnings")
    cargo("clippy", "--locked", "-p", "mnemosyne-kernel", "--target", TARGET,
          "--", "-D", "warnings")
    cargo("clippy", "--locked", "-p", "mnemosyne-user-init", "--target", TARGET,
          "--", "-D", "warnings")


def disk_image(path):
    path = Path(path)
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as disk:
            disk.truncate(16 * 1024 * 1024)
    if path.stat().st_size != 16 * 1024 * 1024:
        raise RuntimeError(f"Expected a dedicated 16-MiB disk image: {path}")
    return path


def guest_command(iso, disk, timeout=None):
    args = ["qemu-system-x86_64", "-machine", "pc", "-accel", "tcg,thread=single",
            "-cpu", "qemu64", "-m", "256M", "-smp", "1", "-cdrom", linux_path(iso),
            "-drive", f"file={linux_path(disk)},format=raw,if=ide,index=0,cache=writethrough",
            "-boot", "d", "-display", "none", "-serial", "stdio", "-monitor", "none",
            "-nic", "none", "-no-reboot", "-no-shutdown",
            "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
    if timeout:
        args = ["timeout", "--signal=KILL", f"{timeout}s", *args]
    return linux_command(args)


def run_os(release=False):
    iso = image(release, shell=True)
    disk = disk_image(BUILD / "chronicle-disk.img")
    print("Booting mnemosyne-os. Type help or shutdown. Disk:", disk, flush=True)
    status = subprocess.call(guest_command(iso, disk), cwd=ROOT)
    if status != 33:
        raise RuntimeError(f"Guest exited with status {status}")


def guest_session(iso, disk, commands, timeout=90):
    command = guest_command(iso, disk, timeout)
    process = subprocess.Popen(command, cwd=ROOT, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = bytearray()
    stream = queue.Queue()
    def reader():
        while True:
            byte = process.stdout.read(1)
            stream.put(byte)
            if not byte:
                break
    thread = threading.Thread(target=reader, daemon=True)
    thread.start()
    deadline = time.monotonic() + timeout + 5
    cursor = 0
    def expect(prompt):
        nonlocal cursor
        while prompt not in output[cursor:]:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError("guest console timed out")
            try:
                byte = stream.get(timeout=min(remaining, 1))
            except queue.Empty:
                continue
            if not byte:
                raise RuntimeError("guest exited before expected prompt")
            output.extend(byte)
        cursor = len(output)
    try:
        for command in commands:
            expect(b"mnemosyne-os> ")
            process.stdin.write(command.encode("ascii") + b"\r")
            process.stdin.flush()
        process.stdin.close()
        while True:
            try:
                byte = stream.get(timeout=max(.1, deadline - time.monotonic()))
            except queue.Empty:
                raise RuntimeError("guest shutdown timed out") from None
            if not byte:
                break
            output.extend(byte)
        status = process.wait(timeout=5)
        if status != 33:
            raise RuntimeError(f"guest failed with status {status}")
    finally:
        # The Linux timeout owns this exact QEMU process and enforces its lifetime.
        (BUILD / "guest-last.log").write_bytes(output)
        print(output.decode("utf-8", errors="replace"), end="", flush=True)
    return output.decode("utf-8", errors="replace")


def guest_test(release=False):
    iso = image(release, shell=True)
    disk = disk_image(BUILD / f"guest-test-{time.time_ns()}.img")
    guest_acceptance(iso, disk)


def guest_acceptance(iso, disk):
    first = guest_session(iso, disk, ["demo", "ps", "plan-show", "write persistence-proof.txt survived-reboot", "shutdown"], timeout=120)
    events = [json.loads(line) for line in first.splitlines() if line.startswith('{"event":')]
    required = {"ipc-demo", "capability-demo", "fault-demo", "cow-replay", "filesystem-demo", "wasm-recovery", "guest-demo"}
    passed = {e["event"] for e in events if e.get("passed") is True}
    if not required <= passed or "error:" in first or "PANIC" in first or "user-panic" in first:
        raise RuntimeError("Guest demonstration did not satisfy every acceptance gate")
    (BUILD / "guest-demo.log").write_text(first, encoding="utf-8")
    plan = next(json.loads(line) for line in first.splitlines() if line.startswith('{"version":1,"action":'))
    (BUILD / "recovery-context.json").write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    bad_plan = {**plan, "requested-rights": 255}
    bad_command = "write rejected-plan.json " + json.dumps(bad_plan, separators=(",", ":"))
    second = guest_session(iso, disk, ["cat persistence-proof.txt", "cat demo-note.txt", bad_command,
                           "verify-plan rejected-plan.json", "recover", "kv-add 1 2", "shutdown"], timeout=120)
    expected_rejection = "error: plan rejected: PrivilegeAmplification"
    if ("survived-reboot" not in second or "before-change" not in second or '"state-equal":true' not in second
            or "kv-service key=1 value=" not in second or expected_rejection not in second
            or "error:" in second.replace(expected_rejection, "")):
        raise RuntimeError("Guest reboot persistence or recovery failed")
    (BUILD / "guest-reboot.log").write_text(second, encoding="utf-8")
    print("Guest acceptance passed: ring-3 isolation, preemption, IPC, capabilities, COW, disk snapshots, Wasm recovery, and reboot persistence.")


def demo():
    result = run(["cargo", "run", "--locked", "--quiet", "-p", "mnemosyne-replay-lab"],
                 capture_output=True, text=True)
    report = json.loads(result.stdout)
    if report.get("passed") is not True:
        raise RuntimeError("replay demo failed its invariants")
    BUILD.mkdir(parents=True, exist_ok=True)
    report_path = BUILD / "replay-demo.json"
    report_path.write_text(result.stdout, encoding="utf-8")
    print(result.stdout, end="")
    print(f"Replay report: {report_path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "image", "os-image", "run", "guest-test", "boot", "test", "check", "demo"])
    parser.add_argument("--release", action="store_true", help="optimized kernel build/image/boot")
    args = parser.parse_args()
    kernel_commands = {"build": build, "image": image, "boot": smoke, "run": run_os,
                       "os-image": lambda release: image(release, shell=True), "guest-test": guest_test}
    if args.command in kernel_commands:
        kernel_commands[args.command](release=args.release)
    else:
        if args.release:
            parser.error("--release applies only to build, image, or boot")
        {"test": test, "check": check, "demo": demo}[args.command]()


if __name__ == "__main__":
    try:
        main()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, RuntimeError) as error:
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
