# Raspberry Pi demonstration

This deployment runs the actual prebuilt **x86-64 MnemosyneOS kernel** in
`qemu-system-x86_64` using TCG on a Raspberry Pi 4 or 5 running 64-bit Linux.
It is not native ARM boot, a Pi firmware image, or a browser simulation. The
Python dashboard controls the guest through its serial console and serves
local browser assets. Rust, Cargo, npm, cloud services and model credentials
are not runtime dependencies. Hardware execution and performance on a real Pi
have not been verified; rehearse on the intended device before presenting.

Use Raspberry Pi OS 64-bit, preferably a Pi with at least 2 GiB RAM, a reliable
power supply and cooling. This is an operational recommendation, not a measured
minimum. The guest uses one emulated CPU and 256 MiB RAM, with no guest NIC.
The host needs additional RAM for Linux, Python, QEMU and any local browser.
TCG translates the x86-64 instructions on ARM64; KVM cannot accelerate this
cross-architecture guest. Keep the default one CPU and modest allocation.

Official references: [Raspberry Pi OS documentation](https://www.raspberrypi.com/documentation/computers/os.html)
describes its 64-bit edition and supported Pi families;
[QEMU invocation documentation](https://www.qemu.org/docs/master/system/invocation.html)
documents TCG, CPU, memory and serial options. These recommendations were
checked against those sources on 2026-10-22.

## Install the prebuilt bundle

Build/package on the development machine after committing and testing the
source. These commands need the repository's build environment, not the Pi:

```sh
python tools/dev.py guest-test --release
python tools/package-demo.py
scp build/mnemosyne-os-demo.tar.gz pi@raspberrypi.local:~/
```

Use your actual SSH username and hostname. On the Pi:

```sh
tar -xzf mnemosyne-os-demo.tar.gz
cd mnemosyne-os-demo
sh deployment/install-demo.sh
sh deployment/start-demo.sh --guest-timeout 300
```

The installer explicitly installs only Debian's `python3` and
`qemu-system-x86` packages through apt. Start does not install anything.
Preflight checks 64-bit Linux, Python 3.10+, QEMU TCG, available memory,
the ISO's build provenance and every file hash in the bundle manifest. A
source checkout without a bundle manifest is labeled accordingly. Hashes detect
accidental changes; obtain the archive through a trusted transfer.

On the Pi desktop, open <http://127.0.0.1:8765>. To present from your laptop,
leave the server running and open a second laptop terminal:

```sh
ssh -N -L 8765:127.0.0.1:8765 pi@raspberrypi.local
```

Open <http://127.0.0.1:8765> on the laptop. The server binds to loopback and
validates local Host/Origin plus its mutation token. Keep the tunnel's local
port equal to the dashboard port. If occupied, use `--port 8766` and change
both tunnel ports and browser URL to 8766. Do not put an unauthenticated public
proxy in front of this service.

## Rehearse the browser flow

For repeatable acceptance before the walkthrough, run:

```sh
python3 tools/guest-e2e.py --timeout 300
```

This starts a real loopback HTTP server, serves the UI assets and drives its
API against QEMU on a new isolated disk under `build/guest-e2e-*`. It copies
the release ISO and UI into that directory and leaves an `acceptance.json`
record there, including failures. It checks the guest demonstration,
benchmarks, file/snapshot restore, rejected recovery authority, reboot
persistence and live recovery. It does not drive the browser DOM; complete
the manual interaction rehearsal below as well. The timeout defaults to 300
seconds per guest action and can be raised to 600 for a slower device.
This acceptance leaves the normal dashboard disk untouched. Remove its
isolated directory only after the test has finished if disk space is needed.

1. Boot/connect the guest in the live guest workspace. Wait for its ready
   prompt and inspect the live serial console. A timeout is a failure, never
   a saved success; allow up to 300 seconds on the Pi before investigating.
2. Use the filesystem workspace to write a small text file, list it and read
   its contents. Take a snapshot, edit the file, then restore the snapshot;
   read it again to establish that the earlier contents returned.
3. Inspect tasks/processes and run the guest demonstration. Show the actual
   guest events for IPC, capability rejection, memory isolation, preemption,
   copy-on-write and filesystem checks in the console.
4. Exercise the failure/recovery controls: crash the service, inspect its
   recovery context/plan, recover it and run a service request again. Explain
   that policy verifies the plan; model inference is optional host tooling.
5. Write a distinctive persistence note, shut down the guest cleanly and boot
   again. Read that note from the same filesystem. The persistent disk is
   `build/dashboard/guest-disk.img`; restarting the dashboard also retains it.

The browser's saved/host evidence views describe separate evidence. Only
actions in the live guest workspace establish what this boot executed. Host
suite actions require a source development environment and are not part of
the prebuilt Pi route. The distribution excludes archived presentation decks.

Stop with Ctrl+C after shutting down the guest. Before moving/deleting a disk,
stop both guest and dashboard. Back up the disk while stopped to preserve demo
files. To intentionally start clean, move the disk aside while stopped; the
next boot creates a fresh dedicated 16-MiB image. Never point the emulator at a
physical SD card or host block device. An abrupt Pi power loss may interrupt
the current operation: do not promise clean-shutdown persistence for an
operation that never acknowledged completion.

## Optional user service

After preflight and a successful interactive rehearsal, run:

```sh
python3 deployment/install-service.py
systemctl --user enable --now mnemosyne-demo.service
journalctl --user -u mnemosyne-demo.service -f
```

This installs a service for the current user and absolute unpacked directory;
keep that directory in place. It still binds only to loopback. The service
starts the dashboard; boot the guest from the browser. User services normally
follow login-session lifetime. If you explicitly want it after logout/reboot,
enable lingering with `sudo loginctl enable-linger "$USER"`.

Stop/disable with `systemctl --user disable --now mnemosyne-demo.service`.
Remove `~/.config/systemd/user/mnemosyne-demo.service` and run
`systemctl --user daemon-reload` to uninstall. Disable lingering separately
if you enabled it solely for this demonstration.

## Troubleshooting

- Missing QEMU: rerun the prerequisite installer and confirm
  `qemu-system-x86_64 --version`; `qemu-system-aarch64` is a different emulator.
- Hash mismatch: restore a fresh bundle; do not silently regenerate a manifest
  over modified files. Build provenance and the archive manifest record the
  source revision and release ISO.
- Slow or timed-out boot: inspect the console and Pi temperature/power/RAM;
  close other applications, use the laptop browser through SSH and retry.
  Increase `--guest-timeout` for this device if necessary. No Pi timing claim
  follows from an x86 host smoke test.
- Port busy: stop the previous service or choose another loopback port.
- Disk locked or guest already running: stop the earlier process; one QEMU
  instance owns a disk at a time. Do not copy a live disk as a backup. After an
  interrupted host session, a stale `build/dashboard/guest-disk.lock` may remain.
  Stop the dashboard and inspect `pgrep -af qemu-system-x86_64` on the Pi. Only
  after confirming no QEMU process owns this disk, remove that specific lock with
  `rm -- build/dashboard/guest-disk.lock` from the unpacked directory and restart.
  On Windows development hosts, also inspect QEMU inside the configured WSL
  distribution. Removing a lock while its guest is alive risks disk corruption.
- Package creation fails on dirty provenance: commit the intended files,
  rebuild the release ISO from that clean revision, then package again.
