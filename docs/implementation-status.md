# Implementation status

MnemosyneOS is a bootable, single-core x86-64 research OS. The guest console,
filesystem and recovery demonstration run under its own kernel. Linux/Windows
are development hosts; QEMU supplies virtual hardware.

| Area | Implemented | Limit |
| --- | --- | --- |
| Boot | Limine BIOS, serial, GDT/TSS/IDT, exceptions, PIC/PIT | UEFI not tested; no SMP/APIC |
| Memory | Frame allocation, kernel/user heaps, private page tables, RX code, RW/NX data, guarded stacks, checked buffers | Fixed boot-created address spaces; no swapping/general mmap |
| Scheduling | Four ring-3 tasks, 100-Hz round robin, integer contexts, blocking receive, timed sleep, fault termination | One CPU; compiled-in roles; no arbitrary ELF admission |
| Capabilities | Caller tables, typed rights, generation checks, bounded IPC, receive authority, device/process checks, endpoint revocation on restore | No cross-process delegation syscall or general object manager |
| Native recovery | Checkpoint at blocked receive, write protection, COW copying, register restore, stale endpoint rejection, IPC replay | One counter-service checkpoint; no multi-service consistent snapshot |
| ChronicleFS | ATA PIO driver; userspace files, atomic batch commits, checksums, snapshots, restore, reboot mount | Flat names; 32 files; 192-KiB/file; 255.5-KiB/root; 64 generations on 16-MiB disk; no GC |
| Wasm | Pinned no-std Wasmi in guest userspace; integer KV fixture; 10,000 fuel/request; 64-KiB memory | Fixed admitted module, no general WASI loader |
| Replay | Durable checkpoint/input-log bundle; trap poisoning; fresh restore; state/effect equality; resumed live requests | Full-copy Wasm checkpoints; synthetic time/random; effects held in memory |
| Policy | Typed JSON DSL, identity/authority/privilege/budget/rollback checks, execution-time revalidation | One action: restore-replay; tested deterministic verifier, no formal proof |
| AI | Optional host OpenAI adapter, strict schema, bounded I/O, offline candidates, guest verification | Online mode needs user credentials/model; not exercised by offline tests |
| Frontend | Live guest start/stop, serial console, files/snapshots/tasks, fault/replay/policy actions and evidence export | Host UI over loopback; remote access through SSH tunnel |
| Evaluation | Failure tests, real HTTP/two-boot acceptance, task/COW counters, 35 guest benchmark samples across seven operations | Coarse QEMU/10-ms measurements, no hardware performance conclusions |
| Pi deployment | Prebuilt package, integrity preflight, install/start scripts and optional user service | ARM64 Linux hosts x86-64 TCG; physical Pi testing and native ARM boot not completed |

## Trust boundary

The kernel owns page tables, task contexts, endpoint queues, capability tables,
serial and the small ATA driver. The privileged ring-3 init process contains the
shell, filesystem, Wasmi embedding and policy executor. Counter, CPU-hog and fault
services have separate address spaces. Further decomposition of filesystem and
Wasm management into independently restartable processes remains future work.

All tasks run the same compiled-in ELF with distinct roles and private pages.
Initial grants are installed by the kernel. Native code uses the freestanding
Rust integer/soft-float ABI; arbitrary native modules, FP/SIMD context sharing,
physical-device support and side-channel resistance are outside this model.
An unexpected native fault in trusted init is fatal. A Wasm trap discards its
failed interpreter instance. CPU fault tests target the separate fault service.

## Storage and recovery contract

ChronicleFS writes a new root's payload into an unused slot, flushes, writes its
checksummed header, and flushes again. Prior roots remain untouched. Mount checks
header, payload and structure, then selects the highest valid generation. Failed
I/O poisons the mount until remount because durability may be ambiguous. Full
volumes fail without overwriting history. Nonzero commit metadata is never
automatically erased or formatted.

This assumes the block device honors flush ordering. Host tests model interrupted
writes, torn headers and payload corruption. Guest tests establish graceful
shutdown/reboot persistence, not every physical power-loss scenario.

The Wasm bundle commits checkpoint, module/profile identity, expected state/effects,
requests and trace together. Candidate plan and service epoch commit in the same
filesystem generation. Recovery remains private until replay/equality succeed;
the executor records its result before replacing the active worker. No external
effects are delivered. Checksums detect corruption, not adversarial rewrites.

There is no networking, guest desktop, POSIX compatibility, arbitrary application
loader, general driver stack, formal kernel proof, or exactly-once external output.

## Reproduce

```sh
python tools/dev.py test
python tools/dev.py build
python tools/dev.py check
python tools/test-dashboard.py
python tools/test-ai-plan.py
python tools/test-guest-controller.py
python tools/test-guest-controller.py --live
python tools/guest-e2e.py
python tools/dev.py boot
python tools/dev.py boot --release
python tools/dev.py guest-test
python tools/dev.py guest-test --release
```

Guest acceptance runs all six demos on a fresh disk, stores a marker, shuts down,
boots that disk again, rejects a privilege-amplifying plan, recovers the Wasm
bundle and accepts a new KV request. Logs: `build/guest-demo.log` and
`build/guest-reboot.log`. Context: `build/recovery-context.json`. Each QEMU boot is
bounded to 120 seconds, one CPU, 256 MiB, and no NIC. Cargo uses two jobs.

The original presentation/host dashboard archive describes its earlier `44b739d`
baseline. Use the [end-to-end status](end-to-end-status.md) and
[Pi runbook](raspberry-pi-demo.md) for the current live frontend and deployment.

The [recorded guest log](evidence/guest-demo.log),
[reboot/rejection log](evidence/guest-reboot.log) and
[capture manifest](evidence/manifest.json) preserve the successful native-Linux
prebuilt-bundle run, with source and ISO identities. They are evidence, not a
substitute for running the current acceptance command.
