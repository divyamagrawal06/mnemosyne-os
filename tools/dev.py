"""Small native-Rust + Linux/WSL QEMU development runner; no cloud services."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

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


def image(release=False):
    build(release)
    bootloader()
    suffix = "-release" if release else ""
    profile = "release" if release else "debug"
    stage = BUILD / ("iso-root" + suffix)
    boot = stage / "boot"
    loader = boot / "limine"
    efi = stage / "EFI" / "BOOT"
    for folder in (loader, efi):
        folder.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "target" / TARGET / profile / "mnemosyne-kernel", boot / "mnemosyne-kernel")
    shutil.copyfile(ROOT / "boot" / "limine.conf", boot / "limine.conf")
    for name in ("limine-bios.sys", "limine-bios-cd.bin", "limine-uefi-cd.bin", "LICENSE"):
        shutil.copyfile(LIMINE / name, loader / name)
    shutil.copyfile(LIMINE / "BOOTX64.EFI", efi / "BOOTX64.EFI")
    iso = BUILD / ("mnemosyne" + suffix + ".iso")
    run(linux_command(["xorriso", "-as", "mkisofs", "-b", "boot/limine/limine-bios-cd.bin",
         "-no-emul-boot", "-boot-load-size", "4", "-boot-info-table", "--efi-boot",
         "boot/limine/limine-uefi-cd.bin", "-efi-boot-part", "--efi-boot-image",
         "--protective-msdos-label", linux_path(stage), "-o", linux_path(iso)]))
    run(linux_command([linux_path(LIMINE / "limine"), "bios-install", linux_path(iso)]))
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
    parser.add_argument("command", choices=["build", "image", "boot", "test", "check", "demo"])
    parser.add_argument("--release", action="store_true", help="optimized kernel build/image/boot")
    args = parser.parse_args()
    kernel_commands = {"build": build, "image": image, "boot": smoke}
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
