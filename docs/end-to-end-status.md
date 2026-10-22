# Integrated demo status

Updated 22 October 2026. The deliverable is a bootable research OS with a real
browser control path and a prebuilt Raspberry Pi deployment bundle. It is not a
general-purpose desktop OS or a native Raspberry Pi firmware image.

## Executable path

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
SMP, guest networking, POSIX, a guest graphical desktop and multi-service
consistent recovery remain outside this delivered prototype. There is no
exactly-once external-effect protocol.

## Validation

- 40 shared Rust tests and formatting/Clippy checks pass.
- 14 dashboard HTTP tests, 5 controller boundary tests and 4 AI adapter tests pass.
- A real controller/QEMU test checks timeout cleanup and restart, four tasks,
  rejection of forged file-based evidence, error propagation, graceful shutdown
  with exit status 33, and file/snapshot persistence after reboot.
- `tools/guest-e2e.py` exercises the real HTTP server and QEMU on an isolated
  disk: complete demo, 35 benchmark samples, file edits/snapshot restore, unsafe
  plan rejection, command-injection rejection, reboot, replay and live KV request.
- Browser interactions are checked separately from HTTP acceptance. See the
  final handoff for the browser run and hardware limitations.

Run these yourself:

```sh
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

Follow [the deployment and classroom runbook](raspberry-pi-demo.md). Keep the
frontend on loopback and use the documented SSH tunnel for a laptop display.
