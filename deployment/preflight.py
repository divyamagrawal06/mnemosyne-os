"""Check a prebuilt demo installation without building or downloading anything."""
import hashlib
import json
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def check():
    failures = []
    if sys.version_info < (3, 10):
        failures.append("Python 3.10 or newer is required")
    print(f"Host: {platform.system()} {platform.machine()}, {struct.calcsize('P') * 8}-bit Python")
    if platform.system() != "Linux" or struct.calcsize("P") != 8:
        failures.append("The deployment scripts require 64-bit Linux")
    if platform.machine().lower() not in ("aarch64", "arm64"):
        print("NOTE: this is not an ARM64 Pi host; it can validate packaging but cannot establish Pi performance.")
    qemu = shutil.which("qemu-system-x86_64")
    if not qemu:
        failures.append("Missing qemu-system-x86_64: install Debian package qemu-system-x86")
    else:
        try:
            print(subprocess.check_output([qemu, "--version"], text=True, timeout=10).splitlines()[0])
            acceleration = subprocess.check_output([qemu, "-accel", "help"], text=True, timeout=10)
            if "tcg" not in acceleration.split():
                failures.append("QEMU does not advertise the required TCG accelerator")
        except (OSError, subprocess.SubprocessError) as error:
            failures.append(f"QEMU check failed: {error}")
    iso = ROOT / "build/mnemosyne-os-release.iso"
    for name in ("tools/dashboard.py", "tools/guest-controller.py", "tools/dev.py", "ui/index.html"):
        if not (ROOT / name).is_file():
            failures.append(f"Missing dashboard runtime file: {name}")
    provenance = iso.with_suffix(".json")
    try:
        expected = json.loads(provenance.read_text(encoding="utf-8"))
        if expected.get("sha256") != hashlib.sha256(iso.read_bytes()).hexdigest():
            failures.append("ISO checksum does not match build provenance")
        if expected.get("profile") != "release" or expected.get("guest-shell") is not True:
            failures.append("ISO must be the release guest-console build")
    except (OSError, ValueError) as error:
        failures.append(f"Missing or invalid release ISO/provenance: {error}")
    manifest = ROOT / "manifest.json"
    if manifest.exists():
        try:
            records = json.loads(manifest.read_text(encoding="utf-8"))["files"]
            if not isinstance(records, dict) or not records:
                raise ValueError("files must be a nonempty hash mapping")
            for name, digest in records.items():
                path = (ROOT / name).resolve()
                if not path.is_relative_to(ROOT) or not path.is_file():
                    failures.append(f"Missing or unsafe manifest path: {name}")
                elif hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                    failures.append(f"Bundle checksum mismatch: {name}")
            print(f"Verified {len(records)} bundled file hashes")
        except (OSError, ValueError, KeyError, TypeError) as error:
            failures.append(f"Invalid bundle manifest: {error}")
    else:
        print("NOTE: no bundle manifest; checking a source checkout, not a verified distribution.")
    memory = Path("/proc/meminfo")
    if memory.exists():
        lines = dict(line.split(":", 1) for line in memory.read_text().splitlines())
        available = int(lines["MemAvailable"].split()[0]) // 1024
        print(f"Available host RAM: {available} MiB; guest allocation: 256 MiB, one emulated CPU")
        if available < 768:
            failures.append("Less than 768 MiB available RAM; close other applications before starting")
    for failure in failures:
        print("FAIL: " + failure, file=sys.stderr)
    if failures:
        return 1
    print("Preflight passed. Hardware execution and timing still require a real Pi rehearsal.")
    return 0


if __name__ == "__main__":
    raise SystemExit(check())
