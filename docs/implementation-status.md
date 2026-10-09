# Implementation status

The first build uses native Windows Cargo and an existing Ubuntu WSL distribution for Limine image construction and QEMU. Local development uses two Cargo jobs and a short-lived QEMU instance with one vCPU and 256 MiB RAM. No cloud VM, model inference, or performance campaign is needed for these checks.

## Working increments

| Area | Implemented and checked | Boundary still missing |
| --- | --- | --- |
| Boot and interrupts | x86-64 Limine boot, serial JSON events, GDT/TSS, double-fault stack, IDT, breakpoint handling, PIC/PIT timer interrupts | UEFI validation, APIC, SMP, scheduling and interrupt-driven preemption |
| Physical memory | Bounded boot allocator over usable Limine regions; two allocated frames checked through the direct map | Reclamation, heap allocation, process address spaces and COW |
| Page mapping | Reuses `x86_64::OffsetPageTable` to map, protect and unmap a new page; checks contents through the direct-map alias; actual CPU faults validate NX, read-only protection and access after unmapping | Separate address spaces, user/supervisor access tests, TLB shootdown across cores, mapping ownership and frame reclamation |
| Capabilities | Allocation-free typed table, generation handles, rights attenuation, delegation subtree revocation, object invalidation; host tests and kernel smoke | Kernel object registry, cross-process transfer, caller-table lookup, syscall enforcement, concurrency and incremental revocation |
| Wasm execution | Host Wasmi interpreter, integer fixture, fuel, fixed memory, restricted imports and validation profile | Port to an isolated userspace service, module admission for arbitrary applications, scheduling integration |
| Replay | Host fixture checkpoint at handler boundaries, full memory/global capture, checkpoint-bound event log, input injection, output validation, poisoned-instance recovery | Durable log, IPC transport, COW checkpoints, consistent service-group snapshots and replay beyond the fixed fixture |
| Recovery policy | Architecture and threat assumptions researched | Typed plan representation, deterministic authority/resource verifier, transactional executor and AI adapter |
| Storage | Protocols and failure cases researched | Block driver, ChronicleFS, commit records, flush ordering, reboot recovery and crash tests |
| Evaluation and UI | Bounded QEMU smoke test and host correctness tests; replay demo emits JSON with its actual trace; local browser dashboard runs suites, inspects evidence and exports results | Statistical benchmark runner, fault campaign and performance conclusions |

The timer test establishes interrupt delivery. It does not establish that a scheduler can preempt a running process. Capability tests establish table behavior under its trusted-caller contract, not system-wide confinement. Wasmi's upstream validation is reused; neither the kernel nor the recovery design is formally verified.

The paging test operates on the boot address space, on one CPU with interrupts disabled. Its allocated frames are retained for that boot. The page-fault handler resumes only explicitly armed assembly probes whose instruction pointer, fault address, and error flags match. Other page faults are fatal. Both debug and optimized builds must pass this test because it modifies a saved exception return address.

## Next acceptance gates

1. **Virtual memory and userspace.** Reuse `x86_64` page-table machinery; track frame ownership; map code read-only/executable and data writable/non-executable. Enter ring 3 with a dedicated kernel entry stack. Reject access to kernel pages and another process's pages. A user fault must terminate only that process.
2. **Scheduling and IPC.** Begin with one core, fixed process/thread limits, a bounded endpoint queue, and simple fixed-priority round robin. Two ring-3 services exchange bounded messages through capability handles. A spinning service is preempted. Invalid handles, oversized messages, full queues, and a crashed receiver have defined outcomes. Add copy-based IPC before shared-memory grants.
3. **Service boundary.** Move the exact Wasmi fixture and import ABI into a userspace runtime service. A deliberately trapping workload must leave a sibling service responsive. Match host/guest semantic tests before measuring overhead.
4. **Durable storage.** Add a userspace block service and a minimal append-only ChronicleFS format with versioned records, checksums, immutable roots, and a documented device flush model. Test crash/reboot after every commit transition before adding a richer namespace.
5. **Durable replay.** Bind checkpoint, capability epoch, input-log position, filesystem root, and pending effects in one recovery descriptor. Define durable input acceptance and output publication ordering. A peer deduplicates stable effect IDs; it participates in the protocol. Restoring memory alone does not make an external effect exactly once.
6. **Checked recovery actions.** An untrusted proposer emits a bounded, typed plan. A trusted executor checks authority and resource reservations against current state, binds execution to that checked state, and falls back deterministically when a plan is rejected or becomes stale. Start with hand-authored plans; add the model only after the verifier and executor are tested.
7. **Evaluation.** Keep baseline restart, checkpoint restart, replay recovery, and policy-assisted recovery separate. Measure correctness, lost/duplicated effects, overhead, and recovery time using seeded fault cases. Save raw measurements and configuration. QEMU smoke-test durations are not hardware performance claims.

## Replay experiment assumptions

- Public construction accepts only the bundled `workloads/kv/service.wat`. It has exactly one fixed memory and one mutable exported global. Supporting arbitrary modules requires a complete state schema and admission validation; copying exported memory alone is insufficient.
- Checkpoints occur between handlers. There is no live operand stack, suspended host call, table state, file descriptor, socket, or native thread to serialize.
- Synthetic clock/random providers let tests deliberately change the live environment on restore. Replay must consume the recorded values and never call the replacement providers.
- The log has at most 4,096 typed events. A request consumes Wasmi fuel; host imports perform bounded in-memory work. A failed request poisons the worker and discards its pending observations. The whole instance is disposable.
- State checksums detect accidental corruption. They are not signatures, semantic validation, a rollback policy, or authorization. An attacker able to rewrite both data and its digest is outside this prototype's integrity model.
- Effects are in-memory observations only. The lab contains no durable acceptance point, durable outbox, peer protocol, or exactly-once guarantee.
- Module bytes, runtime version/configuration, and checkpoint schema are part of the replay contract. Changing any requires a new profile or a separately tested migration. Cross-version replay is not claimed.

## Reproduce the current checks

```sh
python tools/dev.py test
python tools/dev.py check
python tools/dev.py demo
python tools/dev.py boot
python tools/dev.py boot --release
```

Host checks run without WSL. `demo` saves `build/replay-demo.json`; `boot` saves `build/boot.log` and `build/mnemosyne.iso`. Generated outputs are ignored by Git. Dependency versions and registry checksums are committed in `Cargo.lock`; copied code and bootloader pins are documented in `THIRD_PARTY.md`.

`python tools/dashboard.py --open` starts the host UI at `http://127.0.0.1:8765`. It runs the same bounded CLI stages, keeps recorded and live evidence separate, and saves the latest run under `build/dashboard/latest.json`. Its event inspector reads the actual exported replay events. The browser UI does not run in the kernel or establish statistical performance measurements. Runner boundary tests use `python -m unittest discover -s tools -p test_dashboard.py`.
