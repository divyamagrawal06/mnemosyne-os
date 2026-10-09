"""Run the prebuilt OS ISO. Requires Python 3 and QEMU, not a Rust toolchain."""
import argparse
from pathlib import Path
import subprocess
import time
import dev


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", type=Path, default=dev.BUILD / "mnemosyne-os-release.iso")
    parser.add_argument("--disk", type=Path, default=dev.BUILD / "chronicle-disk.img")
    parser.add_argument("--self-test", action="store_true", help="bounded two-boot acceptance on a fresh test disk")
    args = parser.parse_args()
    if not args.image.is_file():
        parser.error("ISO missing; build with python tools/dev.py os-image --release, or unpack the demo bundle")
    if args.self_test:
        disk = dev.disk_image(dev.BUILD / f"prebuilt-test-{time.time_ns()}.img")
        dev.guest_acceptance(args.image, disk)
        return
    disk = dev.disk_image(args.disk)
    print("Booting mnemosyne-os; type help, demo, or shutdown. Disk:", disk, flush=True)
    result = subprocess.call(dev.guest_command(args.image, disk), cwd=dev.ROOT)
    if result != 33:
        raise SystemExit(f"QEMU exited with status {result}")


if __name__ == "__main__":
    main()
