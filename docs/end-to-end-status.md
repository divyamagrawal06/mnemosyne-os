# Integrated demo status

Updated 23 October 2026. The primary interface is now a graphical desktop running
inside the custom OS. It includes a terminal, Files editor, Task manager and
System tools, with PS/2 keyboard and mouse input. A native QEMU window or VNC
viewer shows the guest framebuffer. The older external browser controller is
optional. This remains a bounded prototype, not a general-purpose distribution
or a native Raspberry Pi firmware image.

## Executable path

```text
Native QEMU window / native VNC viewer (SSH tunnel on EC2)
  QEMU VGA framebuffer + PS/2 devices
    custom Rust kernel: checked display/input syscalls, device capabilities
      ring-3 native desktop + shell: terminal, files, tasks, system tools
        actual ChronicleFS state and kernel task counters
```

The pixels, layout, input handling and file/task actions execute inside the
guest. The four desktop views share the existing trusted init process. They
are not four newly isolated application processes or a general window server.
The file editor supports single-line ASCII text up to 420 bytes; larger or
multiline files are read-only previews. Process controls operate on the fixed
worker set. The counter resumes from its quiescent checkpoint; CPU/fault workers
restart from their initial saved context. Init cannot be stopped by these controls.

The separately retained browser experiment path is:

```text
Browser on Pi or laptop
  loopback HTTP (SSH tunnel when using a laptop)
    Python dashboard with Host/Origin/token checks
      owned QEMU process and persistent dedicated disk
        custom x86-64 kernel, protected tasks and guest console
          ChronicleFS, Wasmi, replay and deterministic policy checks
```

The dashboard sends only allowlisted guest commands. It cannot execute arbitrary
host shell text. Each guest command completes at the serial prompt and returns
actual output. Current-run events come only from designated demo commands, so
reading a file containing JSON cannot forge a passing experiment. Guest output
is escaped in the frontend. An interrupted or timed-out command reports failure.
The controller owns one disk, bounds console history and cleans up its QEMU
process. Idle Stop OS requests a graceful shutdown before forced-stop fallback.

## Original modules and current deliverable

| Module | Executable deliverable |
| --- | --- |
| Rust kernel | Limine boot, exception/interrupt setup, physical allocation, page tables and heaps |
| Processes and scheduling | Four ring-3 tasks, separate page tables, guarded stacks, 100-Hz preemption and bounded IPC |
| Capabilities | Typed rights, generation validation, revocation and checked syscall buffers |
| Service isolation | Native CPU fault containment, peer progress and endpoint renewal after restore |
| ChronicleFS | Persistent files, immutable root generations, snapshots and rollback |
| Wasm runtime | Fuel/memory-limited guest Wasmi running a restricted integer KV fixture |
| Checkpoint/replay | Native COW/IPC replay plus durable Wasm checkpoint/trace and resumed worker |
| Recovery control | Typed candidate JSON, deterministic authority/identity/budget/rollback checks; optional host model adapter |
| Fault tests and frontend | Live experiments, task/filesystem views, repeated benchmark observations and downloadable evidence |

The proposal's major mechanisms have an integrated demonstration. This does not
establish general application support, formal verification, exhaustive security,
or a measured benefit from an online AI model. The kernel still includes ATA,
and filesystem/runtime management share trusted init. Arbitrary ELF admission,
SMP, guest networking, POSIX, general desktop applications and multi-service
consistent recovery remain outside this delivered prototype. There is no
exactly-once external-effect protocol.

## Validation

- Native QEMU acceptance passed real framebuffer rendering, PS/2 keyboard and
  pointer navigation, worker stop/start, file-editor writes, display/input
  invalid-pointer rejection, service fault containment and two clean boots with
  file persistence. Screenshots come directly from the guest framebuffer.
- Nine pure device tests cover framebuffer rectangle overflow/bounds, keyboard
  modifiers and prefixes, mouse overflow/resynchronization, PS/2 ACK origin,
  bounded input queues and direction changes at screen edges.
- 40 shared Rust tests and formatting/Clippy checks pass.
- 14 dashboard HTTP tests, 6 controller boundary tests and 4 AI adapter tests pass.
  A concurrent-restart regression checks that stale cleanup cannot stop a new guest.
- A real controller/QEMU test checks timeout cleanup and restart, four tasks,
  rejection of forged file-based evidence, error propagation, graceful shutdown
  with exit status 33, and file/snapshot persistence after reboot.
- `tools/guest-e2e.py` exercises the real HTTP server and QEMU on an isolated
  disk: complete demo, 35 benchmark samples, file edits/snapshot restore, unsafe
  plan rejection, command-injection rejection, reboot, replay and live KV request.
- The extracted prebuilt bundle passed the same HTTP/QEMU acceptance on Linux,
  including manifest/ISO integrity and dependency-notice checks.
- Chromium browser interactions passed boot, file creation, integrated demo,
  candidate-plan save/rejection, recovery, rendering all 35 benchmark samples,
  stop/reboot persistence and the four-task view. Desktop and 390-pixel layouts
  were visually inspected. This is a host browser test, not physical Pi evidence.

Run these yourself:

```sh
python tools/test-devices.py
python3 tools/test-desktop.py --timeout 300
python3 tools/guest-e2e.py --timeout 300
python3 tools/dashboard.py --guest-timeout 300
```

The automated script writes an `acceptance.json` in its printed isolated build
directory, including failure evidence. It never substitutes a saved log for a
new result. The package's manifest records the source commit and file digests.

## Benchmark interpretation

Guest `bench-suite` collects five samples per operation: 100 IPC round trips,
1,000 capability-query syscalls, 1,000 rejected-capability syscalls, 100 recorded
Wasm requests, 20 Wasm snapshots with integrity digests, five complete Wasm
restore/replays and three contained native faults. Each measurement includes
guest scheduling and emulator overhead. The fault sample also includes deliberate
sleep intervals used to observe containment, so it is not fault-handler latency.

The clock has 10-ms resolution. Zero elapsed ticks means below resolution. These
are raw repeatable observations, not hardware IPC latency or isolated capability
validation cost. A comparative performance study with baselines, uncertainty and
real-device measurements remains future evaluation. The benchmark does not write
filesystem generations or replace the user's durable recovery bundle.

## Raspberry Pi boundary

Pi 4/5 with 64-bit Linux runs `qemu-system-x86_64` using TCG and serves the browser
UI. There is no Rust build step on the Pi. Physical Pi hardware has not been
available in this environment, so compatibility and speed must be rehearsed on
the target device. A native Pi port would need ARM64 startup, exception handling,
page tables, interrupt/timer support and Pi-specific drivers.

Follow the [native desktop/EC2 runbook](ec2-desktop-demo.md) to present the guest
screen, or the [external controller runbook](raspberry-pi-demo.md) for browser
experiments. Actual EC2 deployment and physical Pi execution remain untested.
