"""Install an optional systemd user service for this unpacked demo directory."""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def quoted(value):
    # systemd command-line quoting; percent specifiers require doubling.
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def main():
    if sys.platform != "linux":
        raise SystemExit("The optional service requires Linux with a systemd user session.")
    subprocess.run([sys.executable, str(ROOT / "deployment/preflight.py")], check=True)
    destination = Path.home() / ".config/systemd/user/mnemosyne-demo.service"
    if destination.exists():
        raise SystemExit(f"Service already exists: {destination}; inspect/remove it before reinstalling.")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        "[Unit]\nDescription=MnemosyneOS local guest demonstration\n\n[Service]\n"
        f"WorkingDirectory={quoted(ROOT)}\n"
        f"ExecStart={quoted(sys.executable)} {quoted(ROOT / 'tools/dashboard.py')} --guest-timeout 300\n"
        "Restart=on-failure\nRestartSec=5\nTimeoutStopSec=330\n"
        "UMask=0077\nNoNewPrivileges=true\n\n[Install]\nWantedBy=default.target\n",
        encoding="utf-8")
    subprocess.run(["systemctl", "--user", "daemon-reload"], check=True)
    print(f"Installed {destination}")
    print("Start: systemctl --user enable --now mnemosyne-demo.service")
    print("Logs: journalctl --user -u mnemosyne-demo.service -f")


if __name__ == "__main__":
    main()
