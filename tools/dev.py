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


def build():
    cargo("build", "--locked", "-p", "mnemosyne-kernel", "--target", TARGET)


def bootloader():
    if not LIMINE.exists():
        LIMINE.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--depth", "1", "--branch", LIMINE_TAG,
             "https://github.com/limine-bootloader/limine.git", LIMINE])
    revision = subprocess.check_output(["git", "-C", str(LIMINE), "rev-parse", "HEAD"], text=True).strip()
    if revision != LIMINE_COMMIT:
        raise RuntimeError(f"Limine revision mismatch: {revision}")
    run(linux_command(["make", "-C", linux_path(LIMINE), "-j2"]))


def image():
    build()
    bootloader()
    stage = BUILD / "iso-root"
    boot = stage / "boot"
    loader = boot / "limine"
    efi = stage / "EFI" / "BOOT"
    for folder in (loader, efi):
        folder.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "target" / TARGET / "debug" / "mnemosyne-kernel", boot / "mnemosyne-kernel")
    shutil.copyfile(ROOT / "boot" / "limine.conf", boot / "limine.conf")
    for name in ("limine-bios.sys", "limine-bios-cd.bin", "limine-uefi-cd.bin", "LICENSE"):
        shutil.copyfile(LIMINE / name, loader / name)
    shutil.copyfile(LIMINE / "BOOTX64.EFI", efi / "BOOTX64.EFI")
    iso = BUILD / "mnemosyne.iso"
    run(linux_command(["xorriso", "-as", "mkisofs", "-b", "boot/limine/limine-bios-cd.bin",
         "-no-emul-boot", "-boot-load-size", "4", "-boot-info-table", "--efi-boot",
         "boot/limine/limine-uefi-cd.bin", "-efi-boot-part", "--efi-boot-image",
         "--protective-msdos-label", linux_path(stage), "-o", linux_path(iso)]))
    run(linux_command([linux_path(LIMINE / "limine"), "bios-install", linux_path(iso)]))
    return iso


def smoke():
    iso = image()
    command = linux_command(["timeout", "--signal=KILL", "25s", "qemu-system-x86_64",
        "-machine", "q35", "-accel", "tcg,thread=single", "-cpu", "qemu64",
        "-m", "256M", "-smp", "1", "-cdrom", linux_path(iso), "-display", "none",
        "-serial", "stdio", "-monitor", "none", "-nic", "none", "-no-reboot", "-no-shutdown",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
    print("+ " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=35)
    output = result.stdout + result.stderr
    (BUILD / "boot.log").write_text(output, encoding="utf-8")
    print(output, end="")
    events = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{"event":')]
    required = {"boot", "memory", "frames_ok", "exception_ok", "timer_ok", "boot_ok"}
    if result.returncode != 33 or not required <= {e["event"] for e in events}:
        raise RuntimeError(f"kernel smoke failed (QEMU status {result.returncode}); see build/boot.log")
    print("Kernel smoke passed: memory, exception, and timer checks.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "image", "boot"])
    args = parser.parse_args()
    {"build": build, "image": image, "boot": smoke}[args.command]()


if __name__ == "__main__":
    try:
        main()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
