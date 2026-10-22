# Professor brief

For the department's Review I rubric, use the [review guide](review-one-guide.md)
for literature, architecture, algorithms, suggested module ownership and the
live demo. It also identifies the unconfirmed completion percentage and team names.

## Thirty-second explanation

“MnemosyneOS is a small research operating system written in Rust. Its kernel
isolates user tasks with virtual memory and capabilities, schedules them
preemptively, and provides bounded IPC. It includes a persistent snapshotting
filesystem and a WebAssembly runtime. My research focus is predictable service
recovery: record inputs, restore a checkpoint, replay the inputs, and verify that
state and outputs match. Recovery proposals must pass a deterministic policy
verifier before execution.”

## What existed before this increment

A booting kernel with memory/interrupt checks, a capability table, and a separate
host Wasmi replay experiment. The browser dashboard was a host experiment viewer.

## What now runs in the OS

- Four ring-3 tasks, separate page tables, guarded stacks, and a 100-Hz scheduler.
- A guest console that remains usable while a non-yielding task runs.
- Capability IPC, rejected invalid pointers/handles, contained native CPU faults.
- Native service COW checkpoint, restoration and IPC request replay.
- ChronicleFS files/snapshots on a dedicated virtual disk, preserved across boots.
- Guest Wasmi: durable checkpoint/trace, injected trap, fresh replay, equal state
  and effects, and resumed live requests.
- Typed recovery plans checked for identity, authority, privileges, resource
  limits and rollback. An optional host AI adapter proposes plans.

## Scope and limits

A bounded research prototype, not a Linux replacement. One x86-64 CPU, fixed task
and Wasm workload sets, a flat filesystem with finite retained generations.
Shell, filesystem, Wasmi and policy execution share trusted init; the counter
and fault services are separately isolated. No networking, desktop, POSIX,
arbitrary application loader, general driver stack or formal security proof.
AI output is untrusted; offline tests do not establish live model reliability.
Effects are compared in memory, so no exactly-once external delivery is claimed.

## What to show

For the Pi presentation, use the [prebuilt deployment runbook](raspberry-pi-demo.md).
Open the live workspace, choose **Start OS**, then **Experiments → Run demo**.
The browser controls the actual guest serial console and displays its files,
tasks, recovery events and benchmark samples. On the Pi, Linux hosts QEMU's x86
emulator; this is not a native ARM port. See [current validation](end-to-end-status.md).

Boot QEMU and type `demo`, then `ps`, `ls`, and `snapshots`. Write a file, shut down,
reboot and read it. Run `recover` and `kv-add 1 2` to show a recovered application
accepting a new request. Automated `guest-test --release` also rejects an unsafe
recovery plan. See [the runbook](cloud-demo.md) for exact commands.

## Research question

Can bounded checkpoint/replay and checked recovery restore services predictably
without granting an AI model operating-system authority? The next evaluation
compares restart, checkpoint restore and replay under seeded faults, with raw
measurements and explicit integrity assumptions.

Suggested title: **MnemosyneOS: A Capability-Based Research OS with Deterministic
Service Replay and Policy-Checked Recovery**. “Formally verified” would overstate
the current evidence; the verifier is implemented and tested.
