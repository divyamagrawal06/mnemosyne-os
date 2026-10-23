"""Host tests of pure guest device decoding and blit bounds; no QEMU or cloud."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build" / "device-tests"


def main():
    BUILD.mkdir(parents=True, exist_ok=True)
    library = BUILD / "libmnemosyne_abi.rlib"
    binary = BUILD / ("device-model-tests.exe" if os.name == "nt" else "device-model-tests")
    # Inherit rustup/toolchain configuration and environment. The pure ABI crate
    # has no dependencies; this exercises the exact decoder source used by the
    # kernel without pretending that host tests establish functioning hardware.
    commands = [
        ["rustc", "--edition=2024", "--crate-type=rlib", "--crate-name=mnemosyne_abi",
         str(ROOT / "crates/abi/src/lib.rs"), "-o", str(library)],
        ["rustc", "--edition=2024", "--test", str(ROOT / "kernel/src/device-model.rs"),
         "--extern", f"mnemosyne_abi={library}", "-o", str(binary)],
        [str(binary)],
    ]
    for command in commands:
        subprocess.run(command, cwd=ROOT, check=True, timeout=120)


if __name__ == "__main__":
    main()
