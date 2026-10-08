# MnemosyneOS source-reading guide

Inspected on 8 October 2026. Revisions below are the recorded default-branch commits, not automatically the versions used in papers or recommended releases. Findings come from targeted source and documentation reading. Downloading a file is not a complete audit; no external repository was compiled, tested, or benchmarked in this review.

Use the [deep research review](C:/Users/ludic/Desktop/mnemosyne/docs/deep-research.md) for the literature synthesis and proposed protocol. The [source manifest](C:/Users/ludic/Desktop/mnemosyne/research/source-manifest.json) records all collected files, immutable URLs, and content digests. Source links below have line anchors checked against the cached revision.

## Pinned repositories

| Repository | Recorded commit | Commit date | Intended role |
| --- | --- | --- | --- |
| [seL4/seL4](https://github.com/seL4/seL4) | [6df0b6ee61f6](https://github.com/seL4/seL4/tree/6df0b6ee61f6a9e8a4ee7de8eb8fd698c0211b64) | 2026-10-04 | Capability and IPC lifecycle reference |
| [seL4/microkit](https://github.com/seL4/microkit) | [39e7561ef07c](https://github.com/seL4/microkit/tree/39e7561ef07c353e8f333d77ed70f6c712174a05) | 2026-10-05 | Static service configuration and fault delivery |
| [oxidecomputer/hubris](https://github.com/oxidecomputer/hubris) | [e8a43a75ba92](https://github.com/oxidecomputer/hubris/tree/e8a43a75ba92a654979218a9527bfe6a4f33e382) | 2026-10-07 | Rust task lifecycle and supervisor reference |
| [theseus-os/Theseus](https://github.com/theseus-os/Theseus) | [1fbfe567075a](https://github.com/theseus-os/Theseus/tree/1fbfe567075a65ed749b6680db1aeb538819c70c) | 2024-09-22 | State ownership and component recovery reference |
| [mars-research/redleaf](https://github.com/mars-research/redleaf) | [7194295d1968](https://github.com/mars-research/redleaf/tree/7194295d1968c8013ae6b3d104a9192f03516449) | 2022-05-09 | Typed domain failure and shadow-driver reference |
| [wasmi-labs/wasmi](https://github.com/wasmi-labs/wasmi) | [2970aa871cc1](https://github.com/wasmi-labs/wasmi/tree/2970aa871cc1001b57b267ccecdcd1e42306199e) | 2026-09-01 | First Wasm integration candidate |
| [rr-debugger/rr](https://github.com/rr-debugger/rr) | [858a9fbcc5ac](https://github.com/rr-debugger/rr/tree/858a9fbcc5ac71cc578c2e1966f573d7c4ff8257) | 2026-10-01 | Native replay complexity reference |
| [cberner/redb](https://github.com/cberner/redb) | [f403a514fd55](https://github.com/cberner/redb/tree/f403a514fd557f633bd4725248f0d19b434eedff) | 2026-10-05 | Experimental storage comparison or fallback |
| [littlefs-project/littlefs](https://github.com/littlefs-project/littlefs) | [6cb4e86540ec](https://github.com/littlefs-project/littlefs/tree/6cb4e86540eca0d9ba62500a298385c9d863c8be) | 2026-03-24 | Small storage design and failure-injection reference |
| [mit-pdos/fscq](https://github.com/mit-pdos/fscq) | [2c7ef9c268fd](https://github.com/mit-pdos/fscq/tree/2c7ef9c268fd79a81b26b44ef720f8e6a1e938a0) | 2022-10-21 | Crash specification and proof reference |
| [microsoft/verified-storage](https://github.com/microsoft/verified-storage) | [41cab9505122](https://github.com/microsoft/verified-storage/tree/41cab95051229ebd457b4eed7fc111e3e7c9099c) | 2026-09-22 | Modern crash-consistency proof patterns |
| [golemcloud/golem](https://github.com/golemcloud/golem) | [490a77d835cf](https://github.com/golemcloud/golem/tree/490a77d835cfc02be386ec72b30f4e94ac74c2ca) | 2026-10-08 | Durable Wasm host-call semantics reference |
| [bytecodealliance/wizer](https://github.com/bytecodealliance/wizer) | [61301504fd49](https://github.com/bytecodealliance/wizer/tree/61301504fd49c18ddd61c672231f6b280f6c37db) | 2026-03-10 | Initialization snapshot reference |
| [dphilla/weave](https://github.com/dphilla/weave) | [55e36b5f5d5b](https://github.com/dphilla/weave/tree/55e36b5f5d5b31d14f1521c3a4327c248ddeba19) | 2026-10-07 | Optional transformed-continuation experiment |
| [haoyuwang99/AgentSpec](https://github.com/haoyuwang99/AgentSpec) | [e6fa3902e2cf](https://github.com/haoyuwang99/AgentSpec/tree/e6fa3902e2cfb9681f454b355691b771f70543f8) | 2026-01-05 | Agent DSL comparison, not trusted checker code |

## What to inspect and how to use it

### 1. seL4/seL4

**Observed:** `cteRevoke` walks descendants, deletes entries, and calls a preemption point inside the loop. Revocation is real work with interruption semantics, not necessarily an O(1) operation. Endpoint cancellation has separate handling for queued senders, receivers, and reply waiters. The IPC fast path checks message shape, endpoint rights, receiver state, and scheduling conditions before proceeding; other cases take the slow path.

**Decision:** Design revocation completion and process-death behavior before optimizing IPC. Define the point after which revoked descendants cannot be invoked, including during incremental cleanup. Use the implementation as a reference; copying or rewriting its algorithms does not inherit seL4's proofs. Benchmark guarded fast-path cases separately from general IPC.

Source entry points:

- [Capability descendant revocation](https://github.com/seL4/seL4/blob/6df0b6ee61f6a9e8a4ee7de8eb8fd698c0211b64/src/object/cnode.c#L528)
- [IPC cancellation](https://github.com/seL4/seL4/blob/6df0b6ee61f6a9e8a4ee7de8eb8fd698c0211b64/src/object/endpoint.c#L327)
- [IPC fast-path guards](https://github.com/seL4/seL4/blob/6df0b6ee61f6a9e8a4ee7de8eb8fd698c0211b64/src/fastpath/fastpath.c#L19)

**License observation:** Selected kernel files: GPL-2.0-only; repository also contains differently licensed material. [Inspected license text](https://github.com/seL4/seL4/blob/6df0b6ee61f6a9e8a4ee7de8eb8fd698c0211b64/LICENSES/GPL-2.0-only.txt)

### 2. seL4/microkit

**Observed:** The manual describes static resource assignment, protection-domain entry points, CPU budgets/periods, channels, and parent fault handling. Protected calls require a strictly higher-priority callee. The bounded handler-execution rule is a programmer responsibility in the inspected manual; it says future static analysis is intended. The monitor is a fault-handling component, not a complete automatic recovery supervisor.

**Decision:** Borrow the declarative initial system description and explicitly budget the supervisor, broker, and storage service. Acyclic protected-call structure is useful, but it does not prove that a callee terminates or a device responds. Keep dynamic service creation outside the first kernel milestone.

Source entry points:

- [Resource and execution model](https://github.com/seL4/microkit/blob/39e7561ef07c353e8f333d77ed70f6c712174a05/docs/manual.md#L34)
- [Protected-call restrictions](https://github.com/seL4/microkit/blob/39e7561ef07c353e8f333d77ed70f6c712174a05/docs/manual.md#L304)
- [Fault monitor](https://github.com/seL4/microkit/blob/39e7561ef07c353e8f333d77ed70f6c712174a05/monitor/src/main.c)

**License observation:** Selected monitor code: BSD-2-Clause; repository has additional file-specific licenses. [Inspected license text](https://github.com/seL4/microkit/blob/39e7561ef07c353e8f333d77ed70f6c712174a05/LICENSES/BSD-2-Clause.txt)

### 3. oxidecomputer/hubris

**Observed:** The ABI represents a task incarnation with an index and generation. `Task::reinitialize` advances a restart counter and clears kernel task state; the exposed generation is masked into the compact task ID. The selected ABI uses a 16-bit ID and ten index bits. Jefe's supervisor includes restart delays and fault-handling policy. This is an embedded system with statically configured tasks, not a 64-bit virtual-memory OS.

**Decision:** Use the generation-aware peer-death contract and restart throttling as design examples. Do not copy the small wrapping generation field as a long-lived security identity. MnemosyneOS needs checked, wider generations and explicit handling of wrap/exhaustion. Reinitializing kernel state is separate from reinitializing or restoring application memory.

Source entry points:

- [Task identity and dead-peer ABI](https://github.com/oxidecomputer/hubris/blob/e8a43a75ba92a654979218a9527bfe6a4f33e382/sys/abi/src/lib.rs#L30)
- [Task reset and generation](https://github.com/oxidecomputer/hubris/blob/e8a43a75ba92a654979218a9527bfe6a4f33e382/sys/kern/src/task.rs#L318)
- [Restart throttling](https://github.com/oxidecomputer/hubris/blob/e8a43a75ba92a654979218a9527bfe6a4f33e382/task/jefe/src/main.rs#L57)

**License observation:** MPL-2.0 in the selected files and root license. [Inspected license text](https://github.com/oxidecomputer/hubris/blob/e8a43a75ba92a654979218a9527bfe6a4f33e382/LICENSE.txt)

### 4. theseus-os/Theseus

**Observed:** The selected `state_store` is a `no_std` in-memory store built around typed values and shared/weak references. It is not a disk persistence engine. `fault_crate_swap` reloads/replaces crates and tracks address-range changes. Those mechanisms explain how state and component replacement interact, but do not implement the proposed service checkpoint manifest.

**Decision:** Read it for boundaries, state ownership, restartable tasks, and fault recovery. Avoid making a Theseus fork the default project base: its language-centered OS organization differs from hardware-isolated capability processes. The pinned main-branch commit is from September 2024; this review does not infer current maintenance status from that fact alone.

Source entry points:

- [In-memory state ownership](https://github.com/theseus-os/Theseus/blob/1fbfe567075a65ed749b6680db1aeb538819c70c/kernel/state_store/src/lib.rs#L32)
- [Fault-driven crate replacement](https://github.com/theseus-os/Theseus/blob/1fbfe567075a65ed749b6680db1aeb538819c70c/kernel/fault_crate_swap/src/lib.rs#L56)
- [Restartable-task example](https://github.com/theseus-os/Theseus/blob/1fbfe567075a65ed749b6680db1aeb538819c70c/applications/test_restartable/src/lib.rs)

**License observation:** MIT license file inspected; review individual components and dependencies before reuse. [Inspected license text](https://github.com/theseus-os/Theseus/blob/1fbfe567075a65ed749b6680db1aeb538819c70c/LICENSE-MIT)

### 5. mars-research/redleaf

**Observed:** The block shadow domain holds a constructor and current domain object, recreates the memory block-device domain after an error, and retries reads/writes in loops. This is operation-specific recovery around a persistent in-memory backing object. It is not a generic rollback mechanism, and the inspected retry loops themselves do not establish a finite retry bound.

**Decision:** Study the facade and typed failure boundary. In MnemosyneOS, the broker should retry only operations whose semantics support it, with a recorded budget. Do not assume retrying a block overwrite justifies retrying an arbitrary external transaction. The pinned main revision is from May 2022; an old build environment and submodule inventory would need separate investigation before reproduction.

Source entry points:

- [Block-domain replacement and retry](https://github.com/mars-research/redleaf/blob/7194295d1968c8013ae6b3d104a9192f03516449/domains/usr/shadow/bdev/src/lib.rs#L70)
- [NVMe shadow implementation to compare](https://github.com/mars-research/redleaf/blob/7194295d1968c8013ae6b3d104a9192f03516449/domains/usr/shadow/nvme/src/lib.rs)
- [Build and environment notes](https://github.com/mars-research/redleaf/blob/7194295d1968c8013ae6b3d104a9192f03516449/README.md#L7)

**License observation:** No project-wide top-level license found in the recorded tree. Third-party subdirectories have their own licenses; reuse of project code is unresolved. [Recorded tree](https://github.com/mars-research/redleaf/tree/7194295d1968c8013ae6b3d104a9192f03516449)

### 6. wasmi-labs/wasmi

**Observed:** Current source includes `ResumableCallOutOfFuel`, `required_fuel`, and `resume`, with private engine/stack state. Default features include `std`, `wat`, validation, and memory64; `wat` requires `std`. Validation can explicitly disable floating-point types/instructions. Fuel consumption is off by default. Configuration defaults to lazy translation, while a separate fully lazy mode has a determinism warning.

**Decision:** For a cross-build experiment disable defaults but explicitly keep validation; consider deterministic and portable-dispatch features and extra execution checks. Turn fuel on, use eager translation initially, and restrict proposals/memory at admission. Source support for resumable calls is established; portable serialization of a suspended call and successful MnemosyneOS linking are not. Charge and bound host work separately from Wasm fuel.

Source entry points:

- [Fuel suspension and resumption](https://github.com/wasmi-labs/wasmi/blob/2970aa871cc1001b57b267ccecdcd1e42306199e/crates/wasmi/src/engine/resumable.rs#L384)
- [Embedding feature choices](https://github.com/wasmi-labs/wasmi/blob/2970aa871cc1001b57b267ccecdcd1e42306199e/crates/wasmi/Cargo.toml#L38)
- [Floating-point restriction and fuel configuration](https://github.com/wasmi-labs/wasmi/blob/2970aa871cc1001b57b267ccecdcd1e42306199e/crates/wasmi/src/engine/config.rs#L344)
- [Compilation semantics](https://github.com/wasmi-labs/wasmi/blob/2970aa871cc1001b57b267ccecdcd1e42306199e/crates/wasmi/src/engine/config.rs#L39)

**License observation:** Workspace declares MIT/Apache-2.0; MIT text inspected. [Inspected license text](https://github.com/wasmi-labs/wasmi/blob/2970aa871cc1001b57b267ccecdcd1e42306199e/LICENSE-MIT)

### 7. rr-debugger/rr

**Observed:** `PerfCounters.cc` contains CPU-specific counter configuration, tick semantics, and skid allowances. `ReplaySession.cc` handles compatibility of recorded execution state and detailed replay positioning. These mechanisms address a much lower execution boundary than a Wasm host-call log.

**Decision:** Use rr to understand nondeterminism and as a possible host-side debugging aid in a suitable environment, not as a guest replay library. Current code, rather than the older paper's architecture list, should guide platform claims. Keep arbitrary native replay out of the first milestone; supporting it would require instruction/time sources, signals, concurrency, and kernel state beyond simple IPC recording.

Source entry points:

- [Hardware tick semantics](https://github.com/rr-debugger/rr/blob/858a9fbcc5ac71cc578c2e1966f573d7c4ff8257/src/PerfCounters.cc#L166)
- [Replay mechanics](https://github.com/rr-debugger/rr/blob/858a9fbcc5ac71cc578c2e1966f573d7c4ff8257/src/ReplaySession.cc)
- [Trace representation](https://github.com/rr-debugger/rr/blob/858a9fbcc5ac71cc578c2e1966f573d7c4ff8257/src/TraceStream.h)

**License observation:** MIT license inspected. [Inspected license text](https://github.com/rr-debugger/rr/blob/858a9fbcc5ac71cc578c2e1966f573d7c4ff8257/LICENSE)

### 8. cberner/redb

**Observed:** The inspected manifest documents experimental `no_std` support when defaults are disabled and `experimental-api-5` is enabled; `alloc` is still required. A dedicated no-std test package uses that combination. Savepoint restoration treats data roots differently from system metadata. The design assumes atomic single-byte writes, durability after sync, and no collateral changes outside a written range. A crash regression covers a header becoming persistent before corresponding file growth.

**Decision:** Run a small adapter/cross-build experiment before deciding portability. Preserve the backend's durability and overwrite contract on the block device. Persistent savepoints are not automatically independent writable filesystem branches. This is a KV engine candidate, not an already implemented ChronicleFS, and the experimental feature has compatibility risk.

Source entry points:

- [Experimental no-std contract](https://github.com/cberner/redb/blob/f403a514fd557f633bd4725248f0d19b434eedff/Cargo.toml#L49)
- [Concrete feature combination](https://github.com/cberner/redb/blob/f403a514fd557f633bd4725248f0d19b434eedff/crates/redb-no-std-test/Cargo.toml#L11)
- [Storage assumptions](https://github.com/cberner/redb/blob/f403a514fd557f633bd4725248f0d19b434eedff/docs/design.md#L683)
- [File-growth crash regression](https://github.com/cberner/redb/blob/f403a514fd557f633bd4725248f0d19b434eedff/tests/crash_consistency.rs#L129)
- [Rollback and metadata lifetime](https://github.com/cberner/redb/blob/f403a514fd557f633bd4725248f0d19b434eedff/docs/design.md#L443)

**License observation:** Manifest declares MIT OR Apache-2.0; MIT text inspected. [Inspected license text](https://github.com/cberner/redb/blob/f403a514fd557f633bd4725248f0d19b434eedff/LICENSE-MIT)

### 9. littlefs-project/littlefs

**Observed:** The design combines metadata pairs and copy-on-write data structures for flash-oriented storage. Its power-loss tests include partial programming and metadata corner cases. The emulated block backend exposes test hooks and has host-platform dependencies, which distinguishes it from the filesystem core.

**Decision:** Borrow the discipline of an explicit media model and tests that continue after recovery to check future writes. Study metadata publication and allocator behavior. Do not treat littlefs as a ready-made transactional snapshot filesystem: the reviewed mechanisms do not establish the required multi-object transaction, versioned namespace, and candidate-branch API. Its flash assumptions also need adaptation to virtio-backed block storage.

Source entry points:

- [Metadata update protocol](https://github.com/littlefs-project/littlefs/blob/6cb4e86540eca0d9ba62500a298385c9d863c8be/DESIGN.md#L298)
- [Partial-program fault cases](https://github.com/littlefs-project/littlefs/blob/6cb4e86540eca0d9ba62500a298385c9d863c8be/tests/test_powerloss.toml#L91)
- [Storage emulator](https://github.com/littlefs-project/littlefs/blob/6cb4e86540eca0d9ba62500a298385c9d863c8be/bd/lfs_emubd.c)

**License observation:** BSD-3-Clause in the selected block emulator and root license. [Inspected license text](https://github.com/littlefs-project/littlefs/blob/6cb4e86540eca0d9ba62500a298385c9d863c8be/LICENSE.md)

### 10. mit-pdos/fscq

**Observed:** The README explicitly calls the repository an unmaintained research prototype and says main/master roughly corresponds to DFSCQ, not the original 2015 FSCQ artifact. It distinguishes verified Coq components from unverified Haskell/FUSE integration. The selected proof files expose disk-state and log-replay specifications.

**Decision:** Read the specifications and recovery structure, especially legal states after a crash. Use the correct publication branch if reproducing a result. A Rust translation does not preserve the proof automatically, and this is not a suitable production storage dependency for a new microkernel. Keep the initial proof goal to a small commit/recovery primitive.

Source entry points:

- [Maintenance and proof boundaries](https://github.com/mit-pdos/fscq/blob/2c7ef9c268fd79a81b26b44ef720f8e6a1e938a0/README.md#L5)
- [Abstract disk replay](https://github.com/mit-pdos/fscq/blob/2c7ef9c268fd79a81b26b44ef720f8e6a1e938a0/src/LogReplay.v#L41)
- [Asynchronous disk model](https://github.com/mit-pdos/fscq/blob/2c7ef9c268fd79a81b26b44ef720f8e6a1e938a0/src/AsyncDisk.v)

**License observation:** MIT-style license text inspected. [Inspected license text](https://github.com/mit-pdos/fscq/blob/2c7ef9c268fd79a81b26b44ef720f8e6a1e938a0/LICENSE)

### 11. microsoft/verified-storage

**Observed:** The README separates persistent logs, transactional multilogs, KV storage, soundness proofs, and unverified mocks. It points to `osdi25-artifact` for the PoWER paper artifact. The inspected persistent-memory write interface requires permission for every modeled partial-write result and distinguishes trusted interfaces from verified wrappers.

**Decision:** Study the proof architecture and permissions over recovery outcomes. Adapt the model to block writes and flushes before using it for ChronicleFS. Inspect trusted definitions and external bodies rather than counting all Verus files as proven implementation. Reading this public repository requires no cloud infrastructure.

Source entry points:

- [Paper artifact versus current source](https://github.com/microsoft/verified-storage/blob/41cab95051229ebd457b4eed7fc111e3e7c9099c/README.md#L7)
- [Write preconditions over crash states](https://github.com/microsoft/verified-storage/blob/41cab95051229ebd457b4eed7fc111e3e7c9099c/capybaraKV/capybarakv/src/pmem/power_t.rs#L93)
- [Verified wrapper](https://github.com/microsoft/verified-storage/blob/41cab95051229ebd457b4eed7fc111e3e7c9099c/capybaraKV/capybarakv/src/pmem/power_v.rs)
- [Crash-invariant interface](https://github.com/microsoft/verified-storage/blob/41cab95051229ebd457b4eed7fc111e3e7c9099c/capybaraKV/capybarakv/src/pmem/crashinv_t.rs)

**License observation:** MIT license inspected. [Inspected license text](https://github.com/microsoft/verified-storage/blob/41cab95051229ebd457b4eed7fc111e3e7c9099c/LICENSE.txt)

### 12. golemcloud/golem

**Observed:** The selected durability implementation classifies local/remote reads and writes. It permits internal retry of remote writes only under its `assume_idempotence` state and reuses that classification when replay finds a committed start without an end. The nearby tests cover these distinctions. Replay and rollback involve additional state and scope machinery, not just serializing return values.

**Decision:** Study the incomplete-effect state machine and request classification. Do not import the full runtime into the kernel or copy an assumption flag without a trusted connector contract. The BSL terms need a separate reuse decision. Use Golem as related work demonstrating that durable Wasm execution already exists; MnemosyneOS must justify its capability and publication boundary.

Source entry points:

- [Retry eligibility](https://github.com/golemcloud/golem/blob/490a77d835cfc02be386ec72b30f4e94ac74c2ca/golem-worker-executor/src/durable_host/durability.rs#L2593)
- [Incomplete-call replay decision](https://github.com/golemcloud/golem/blob/490a77d835cfc02be386ec72b30f4e94ac74c2ca/golem-worker-executor/src/durable_host/durability.rs#L2610)
- [Rollback-region calculation](https://github.com/golemcloud/golem/blob/490a77d835cfc02be386ec72b30f4e94ac74c2ca/golem-worker-executor/src/durable_host/replay_state/rollback.rs#L6)

**License observation:** Business Source License 1.1 with an additional use grant and a stated change license of Apache-2.0 after the change date. Not a blanket permissive open-source dependency. [Inspected license text](https://github.com/golemcloud/golem/blob/490a77d835cfc02be386ec72b30f4e94ac74c2ca/LICENSE)

### 13. bytecodealliance/wizer

**Observed:** The README identifies Wizer as a pre-initializer: execute initialization and emit a new module containing the resulting initialized state. It also states that development moved to the Wasmtime repository under `crates/wizer`, with `wasmtime wizer` replacing the separate CLI for future use.

**Decision:** Potentially useful for reducing repeated initialization, but not evidence of arbitrary in-flight continuation recovery. Treat the inspected repository as historical reference and check the upstream Wasmtime implementation before integrating. Do not promise its documented startup improvements for MnemosyneOS without measuring the chosen workload. The migration notice also prevents a misleading claim that this standalone repo is the current development location.

Source entry points:

- [Upstream move](https://github.com/bytecodealliance/wizer/blob/61301504fd49c18ddd61c672231f6b280f6c37db/README.md#L34)
- [Initialization restrictions](https://github.com/bytecodealliance/wizer/blob/61301504fd49c18ddd61c672231f6b280f6c37db/README.md#L114)

**License observation:** Apache-2.0 with LLVM exceptions in the inspected license. [Inspected license text](https://github.com/bytecodealliance/wizer/blob/61301504fd49c18ddd61c672231f6b280f6c37db/LICENSE)

### 14. dphilla/weave

**Observed:** The project describes rewriting Wasm control flow so live locals and continuation state can be stored in linear memory. The selected transform entry point validates its generated module. The snapshot container includes module identity, memories, control globals, and host-service blobs. Tests explicitly reject unsupported proposals including table64 and exception tags. The README describes uncertain migration commit outcomes rather than always restarting both copies.

**Decision:** A concrete lead if handler boundaries become too restrictive. First independently test a tiny supported module and measure code-size/runtime overhead. The README's end-to-end claims were not reproduced here. This is an extra compiler and snapshot-format dependency, not a proven replacement for the capability-aware broker. Host-service blobs still need ownership and external-effect semantics.

Source entry points:

- [Transform and output validation](https://github.com/dphilla/weave/blob/55e36b5f5d5b31d14f1521c3a4327c248ddeba19/crates/weave-transform/src/lib.rs#L74)
- [Portable state container](https://github.com/dphilla/weave/blob/55e36b5f5d5b31d14f1521c3a4327c248ddeba19/crates/weave-core/src/snapshot.rs#L20)
- [Unsupported-feature rejection](https://github.com/dphilla/weave/blob/55e36b5f5d5b31d14f1521c3a4327c248ddeba19/crates/weave-transform/tests/unsupported_features.rs#L33)
- [Migration outcome boundary](https://github.com/dphilla/weave/blob/55e36b5f5d5b31d14f1521c3a4327c248ddeba19/README.md#L82)

**License observation:** Apache-2.0 license inspected. [Inspected license text](https://github.com/dphilla/weave/blob/55e36b5f5d5b31d14f1521c3a4327c248ddeba19/LICENSE)

### 15. haoyuwang99/AgentSpec

**Observed:** The grammar encodes triggers, predicates, and enforcement actions. The enforcement implementation has stop/skip operations, interactive user inspection, and an LLM self-reflection path. Therefore, a typed language in this family can still call nondeterministic decision procedures.

**Decision:** Use the paper and language design as related work. Implement a much smaller deterministic acceptance core for MnemosyneOS, with no LLM calls or interactive predicates inside authorization. Planner generation may remain nondeterministic; acceptance and enforcement must have explicit semantics. The repository was inspected as a research artifact, not selected as a dependency.

Source entry points:

- [Policy grammar](https://github.com/haoyuwang99/AgentSpec/blob/e6fa3902e2cfb9681f454b355691b771f70543f8/src/spec_lang/AgentSpec.g4#L2)
- [LLM-based enforcement path](https://github.com/haoyuwang99/AgentSpec/blob/e6fa3902e2cfb9681f454b355691b771f70543f8/src/enforcement.py#L58)
- [Interactive enforcement path](https://github.com/haoyuwang99/AgentSpec/blob/e6fa3902e2cfb9681f454b355691b771f70543f8/src/enforcement.py#L33)

**License observation:** No license-named file found in the recorded repository tree; reuse terms unresolved. [Recorded tree](https://github.com/haoyuwang99/AgentSpec/tree/e6fa3902e2cfb9681f454b355691b771f70543f8)

## Reuse shortlist

| Choice | Recommendation | Gate before adoption |
| --- | --- | --- |
| Wasmi | First runtime candidate | Pinned freestanding cross-build, validation profile, bounded host imports, replay experiment. |
| redb | Storage comparison/fallback experiment | Exact feature set, adapter durability contract, allocator/lock portability, namespace and branch semantics. |
| Weave | Optional later continuation experiment | Independent semantic tests and overhead measurements; supported-proposal contract. |
| seL4, Microkit, Hubris | Kernel/service design references | Implement and validate MnemosyneOS's own lifecycle and authority model. |
| Theseus, RedLeaf | State ownership and failure-boundary references | Do not substitute language isolation for the proposed native-code threat boundary. |
| littlefs, FSCQ, verified-storage | Storage testing/specification references | Translate media assumptions and preserve distinction between code, model, and proof. |
| rr, Golem | Replay/effect semantics references | Avoid treating host-specific machinery as a guest library. |
| AgentSpec | Related work and DSL comparison | Deterministic, bounded checker implemented separately. |
| Wizer | Possible initialization optimization | Follow its new upstream location; does not solve general live recovery. |

## Reading is not reproduction

To reproduce a paper, use its stated artifact branch, compiler, configuration, workload, fault model, and hardware assumptions. In particular, FSCQ's current master is associated with DFSCQ, the PoWER artifact uses a dedicated branch, and Wizer development moved upstream. Main-branch source inspections answer implementation questions but do not independently validate published performance or proofs.

License entries describe the files observed, not all transitive dependencies or a completed reuse review. No project code from these repositories has been incorporated into the MnemosyneOS kernel by this research task.
