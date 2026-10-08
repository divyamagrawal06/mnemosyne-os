"""Render pinned, line-checked source references from the research manifest."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MANIFEST = json.loads((ROOT / "source-manifest.json").read_text(encoding="utf-8"))
FILES = {(item["repo"], item["path"]): item for item in MANIFEST}


def link(repo, path, needle=None, label=None):
    item = FILES[(repo, path)]
    url = item["url"]
    if needle is not None:
        lines = (ROOT / "cache" / repo.replace("/", "__") / "source" / path).read_text(encoding="utf-8").splitlines()
        line = next(i for i, text in enumerate(lines, 1) if needle in text)
        url += f"#L{line}"
    return f"[{label or path}]({url})"


PROFILES = [
    {
        "repo": "seL4/seL4", "role": "Capability and IPC lifecycle reference",
        "license": "Selected kernel files: GPL-2.0-only; repository also contains differently licensed material.",
        "license_path": "LICENSES/GPL-2.0-only.txt",
        "finding": "`cteRevoke` walks descendants, deletes entries, and calls a preemption point inside the loop. Revocation is real work with interruption semantics, not necessarily an O(1) operation. Endpoint cancellation has separate handling for queued senders, receivers, and reply waiters. The IPC fast path checks message shape, endpoint rights, receiver state, and scheduling conditions before proceeding; other cases take the slow path.",
        "decision": "Design revocation completion and process-death behavior before optimizing IPC. Define the point after which revoked descendants cannot be invoked, including during incremental cleanup. Use the implementation as a reference; copying or rewriting its algorithms does not inherit seL4's proofs. Benchmark guarded fast-path cases separately from general IPC.",
        "pointers": [("src/object/cnode.c", "exception_t cteRevoke", "Capability descendant revocation"), ("src/object/endpoint.c", "void cancelIPC", "IPC cancellation"), ("src/fastpath/fastpath.c", "void NORETURN fastpath_call", "IPC fast-path guards")],
    },
    {
        "repo": "seL4/microkit", "role": "Static service configuration and fault delivery",
        "license": "Selected monitor code: BSD-2-Clause; repository has additional file-specific licenses.",
        "license_path": "LICENSES/BSD-2-Clause.txt",
        "finding": "The manual describes static resource assignment, protection-domain entry points, CPU budgets/periods, channels, and parent fault handling. Protected calls require a strictly higher-priority callee. The bounded handler-execution rule is a programmer responsibility in the inspected manual; it says future static analysis is intended. The monitor is a fault-handling component, not a complete automatic recovery supervisor.",
        "decision": "Borrow the declarative initial system description and explicitly budget the supervisor, broker, and storage service. Acyclic protected-call structure is useful, but it does not prove that a callee terminates or a device responds. Keep dynamic service creation outside the first kernel milestone.",
        "pointers": [("docs/manual.md", "static architecture", "Resource and execution model"), ("docs/manual.md", "strictly higher priority", "Protected-call restrictions"), ("monitor/src/main.c", None, "Fault monitor")],
    },
    {
        "repo": "oxidecomputer/hubris", "role": "Rust task lifecycle and supervisor reference",
        "license": "MPL-2.0 in the selected files and root license.", "license_path": "LICENSE.txt",
        "finding": "The ABI represents a task incarnation with an index and generation. `Task::reinitialize` advances a restart counter and clears kernel task state; the exposed generation is masked into the compact task ID. The selected ABI uses a 16-bit ID and ten index bits. Jefe's supervisor includes restart delays and fault-handling policy. This is an embedded system with statically configured tasks, not a 64-bit virtual-memory OS.",
        "decision": "Use the generation-aware peer-death contract and restart throttling as design examples. Do not copy the small wrapping generation field as a long-lived security identity. MnemosyneOS needs checked, wider generations and explicit handling of wrap/exhaustion. Reinitializing kernel state is separate from reinitializing or restoring application memory.",
        "pointers": [("sys/abi/src/lib.rs", "pub struct TaskId", "Task identity and dead-peer ABI"), ("sys/kern/src/task.rs", "pub fn reinitialize", "Task reset and generation"), ("task/jefe/src/main.rs", "Minimum amount of time", "Restart throttling")],
    },
    {
        "repo": "theseus-os/Theseus", "role": "State ownership and component recovery reference",
        "license": "MIT license file inspected; review individual components and dependencies before reuse.", "license_path": "LICENSE-MIT",
        "finding": "The selected `state_store` is a `no_std` in-memory store built around typed values and shared/weak references. It is not a disk persistence engine. `fault_crate_swap` reloads/replaces crates and tracks address-range changes. Those mechanisms explain how state and component replacement interact, but do not implement the proposed service checkpoint manifest.",
        "decision": "Read it for boundaries, state ownership, restartable tasks, and fault recovery. Avoid making a Theseus fork the default project base: its language-centered OS organization differs from hardware-isolated capability processes. The pinned main-branch commit is from September 2024; this review does not infer current maintenance status from that fact alone.",
        "pointers": [("kernel/state_store/src/lib.rs", "struct SystemStateList", "In-memory state ownership"), ("kernel/fault_crate_swap/src/lib.rs", "pub fn do_self_swap", "Fault-driven crate replacement"), ("applications/test_restartable/src/lib.rs", None, "Restartable-task example")],
    },
    {
        "repo": "mars-research/redleaf", "role": "Typed domain failure and shadow-driver reference",
        "license": "No project-wide top-level license found in the recorded tree. Third-party subdirectories have their own licenses; reuse of project code is unresolved.",
        "finding": "The block shadow domain holds a constructor and current domain object, recreates the memory block-device domain after an error, and retries reads/writes in loops. This is operation-specific recovery around a persistent in-memory backing object. It is not a generic rollback mechanism, and the inspected retry loops themselves do not establish a finite retry bound.",
        "decision": "Study the facade and typed failure boundary. In MnemosyneOS, the broker should retry only operations whose semantics support it, with a recorded budget. Do not assume retrying a block overwrite justifies retrying an arbitrary external transaction. The pinned main revision is from May 2022; an old build environment and submodule inventory would need separate investigation before reproduction.",
        "pointers": [("domains/usr/shadow/bdev/src/lib.rs", "unsafe fn restart_bdev", "Block-domain replacement and retry"), ("domains/usr/shadow/nvme/src/lib.rs", None, "NVMe shadow implementation to compare"), ("README.md", "## Setup", "Build and environment notes")],
    },
    {
        "repo": "wasmi-labs/wasmi", "role": "First Wasm integration candidate",
        "license": "Workspace declares MIT/Apache-2.0; MIT text inspected.", "license_path": "LICENSE-MIT",
        "finding": "Current source includes `ResumableCallOutOfFuel`, `required_fuel`, and `resume`, with private engine/stack state. Default features include `std`, `wat`, validation, and memory64; `wat` requires `std`. Validation can explicitly disable floating-point types/instructions. Fuel consumption is off by default. Configuration defaults to lazy translation, while a separate fully lazy mode has a determinism warning.",
        "decision": "For a cross-build experiment disable defaults but explicitly keep validation; consider deterministic and portable-dispatch features and extra execution checks. Turn fuel on, use eager translation initially, and restrict proposals/memory at admission. Source support for resumable calls is established; portable serialization of a suspended call and successful MnemosyneOS linking are not. Charge and bound host work separately from Wasm fuel.",
        "pointers": [("crates/wasmi/src/engine/resumable.rs", "pub struct ResumableCallOutOfFuel", "Fuel suspension and resumption"), ("crates/wasmi/Cargo.toml", "default =", "Embedding feature choices"), ("crates/wasmi/src/engine/config.rs", "pub fn floats", "Floating-point restriction and fuel configuration"), ("crates/wasmi/src/engine/config.rs", "pub enum CompilationMode", "Compilation semantics")],
    },
    {
        "repo": "rr-debugger/rr", "role": "Native replay complexity reference",
        "license": "MIT license inspected.", "license_path": "LICENSE",
        "finding": "`PerfCounters.cc` contains CPU-specific counter configuration, tick semantics, and skid allowances. `ReplaySession.cc` handles compatibility of recorded execution state and detailed replay positioning. These mechanisms address a much lower execution boundary than a Wasm host-call log.",
        "decision": "Use rr to understand nondeterminism and as a possible host-side debugging aid in a suitable environment, not as a guest replay library. Current code, rather than the older paper's architecture list, should guide platform claims. Keep arbitrary native replay out of the first milestone; supporting it would require instruction/time sources, signals, concurrency, and kernel state beyond simple IPC recording.",
        "pointers": [("src/PerfCounters.cc", "PMU_TICKS_RCB", "Hardware tick semantics"), ("src/ReplaySession.cc", None, "Replay mechanics"), ("src/TraceStream.h", None, "Trace representation")],
    },
    {
        "repo": "cberner/redb", "role": "Experimental storage comparison or fallback",
        "license": "Manifest declares MIT OR Apache-2.0; MIT text inspected.", "license_path": "LICENSE-MIT",
        "finding": "The inspected manifest documents experimental `no_std` support when defaults are disabled and `experimental-api-5` is enabled; `alloc` is still required. A dedicated no-std test package uses that combination. Savepoint restoration treats data roots differently from system metadata. The design assumes atomic single-byte writes, durability after sync, and no collateral changes outside a written range. A crash regression covers a header becoming persistent before corresponding file growth.",
        "decision": "Run a small adapter/cross-build experiment before deciding portability. Preserve the backend's durability and overwrite contract on the block device. Persistent savepoints are not automatically independent writable filesystem branches. This is a KV engine candidate, not an already implemented ChronicleFS, and the experimental feature has compatibility risk.",
        "pointers": [("Cargo.toml", "Turning it off requires", "Experimental no-std contract"), ("crates/redb-no-std-test/Cargo.toml", "default-features", "Concrete feature combination"), ("docs/design.md", "# Assumptions about underlying media", "Storage assumptions"), ("tests/crash_consistency.rs", "fn crash_during_growing_commit_is_recoverable", "File-growth crash regression"), ("docs/design.md", "Restoring a savepoint", "Rollback and metadata lifetime")],
    },
    {
        "repo": "littlefs-project/littlefs", "role": "Small storage design and failure-injection reference",
        "license": "BSD-3-Clause in the selected block emulator and root license.", "license_path": "LICENSE.md",
        "finding": "The design combines metadata pairs and copy-on-write data structures for flash-oriented storage. Its power-loss tests include partial programming and metadata corner cases. The emulated block backend exposes test hooks and has host-platform dependencies, which distinguishes it from the filesystem core.",
        "decision": "Borrow the discipline of an explicit media model and tests that continue after recovery to check future writes. Study metadata publication and allocator behavior. Do not treat littlefs as a ready-made transactional snapshot filesystem: the reviewed mechanisms do not establish the required multi-object transaction, versioned namespace, and candidate-branch API. Its flash assumptions also need adaptation to virtio-backed block storage.",
        "pointers": [("DESIGN.md", "## Metadata pairs", "Metadata update protocol"), ("tests/test_powerloss.toml", "partial prog", "Partial-program fault cases"), ("bd/lfs_emubd.c", None, "Storage emulator")],
    },
    {
        "repo": "mit-pdos/fscq", "role": "Crash specification and proof reference",
        "license": "MIT-style license text inspected.", "license_path": "LICENSE",
        "finding": "The README explicitly calls the repository an unmaintained research prototype and says main/master roughly corresponds to DFSCQ, not the original 2015 FSCQ artifact. It distinguishes verified Coq components from unverified Haskell/FUSE integration. The selected proof files expose disk-state and log-replay specifications.",
        "decision": "Read the specifications and recovery structure, especially legal states after a crash. Use the correct publication branch if reproducing a result. A Rust translation does not preserve the proof automatically, and this is not a suitable production storage dependency for a new microkernel. Keep the initial proof goal to a small commit/recovery primitive.",
        "pointers": [("README.md", "## Unmaintained", "Maintenance and proof boundaries"), ("src/LogReplay.v", "Definition replay_disk", "Abstract disk replay"), ("src/AsyncDisk.v", None, "Asynchronous disk model")],
    },
    {
        "repo": "microsoft/verified-storage", "role": "Modern crash-consistency proof patterns",
        "license": "MIT license inspected.", "license_path": "LICENSE.txt",
        "finding": "The README separates persistent logs, transactional multilogs, KV storage, soundness proofs, and unverified mocks. It points to `osdi25-artifact` for the PoWER paper artifact. The inspected persistent-memory write interface requires permission for every modeled partial-write result and distinguishes trusted interfaces from verified wrappers.",
        "decision": "Study the proof architecture and permissions over recovery outcomes. Adapt the model to block writes and flushes before using it for ChronicleFS. Inspect trusted definitions and external bodies rather than counting all Verus files as proven implementation. Reading this public repository requires no cloud infrastructure.",
        "pointers": [("README.md", "artifact associated", "Paper artifact versus current source"), ("capybaraKV/capybarakv/src/pmem/power_t.rs", "pub exec fn write<Perm>", "Write preconditions over crash states"), ("capybaraKV/capybarakv/src/pmem/power_v.rs", None, "Verified wrapper"), ("capybaraKV/capybarakv/src/pmem/crashinv_t.rs", None, "Crash-invariant interface")],
    },
    {
        "repo": "golemcloud/golem", "role": "Durable Wasm host-call semantics reference",
        "license": "Business Source License 1.1 with an additional use grant and a stated change license of Apache-2.0 after the change date. Not a blanket permissive open-source dependency.", "license_path": "LICENSE",
        "finding": "The selected durability implementation classifies local/remote reads and writes. It permits internal retry of remote writes only under its `assume_idempotence` state and reuses that classification when replay finds a committed start without an end. The nearby tests cover these distinctions. Replay and rollback involve additional state and scope machinery, not just serializing return values.",
        "decision": "Study the incomplete-effect state machine and request classification. Do not import the full runtime into the kernel or copy an assumption flag without a trusted connector contract. The BSL terms need a separate reuse decision. Use Golem as related work demonstrating that durable Wasm execution already exists; MnemosyneOS must justify its capability and publication boundary.",
        "pointers": [("golem-worker-executor/src/durable_host/durability.rs", "fn is_eligible_for_internal_retry", "Retry eligibility"), ("golem-worker-executor/src/durable_host/durability.rs", "pub fn can_reexecute_on_incomplete_replay", "Incomplete-call replay decision"), ("golem-worker-executor/src/durable_host/replay_state/rollback.rs", "pub fn suffix_rollback_region", "Rollback-region calculation")],
    },
    {
        "repo": "bytecodealliance/wizer", "role": "Initialization snapshot reference",
        "license": "Apache-2.0 with LLVM exceptions in the inspected license.", "license_path": "LICENSE",
        "finding": "The README identifies Wizer as a pre-initializer: execute initialization and emit a new module containing the resulting initialized state. It also states that development moved to the Wasmtime repository under `crates/wizer`, with `wasmtime wizer` replacing the separate CLI for future use.",
        "decision": "Potentially useful for reducing repeated initialization, but not evidence of arbitrary in-flight continuation recovery. Treat the inspected repository as historical reference and check the upstream Wasmtime implementation before integrating. Do not promise its documented startup improvements for MnemosyneOS without measuring the chosen workload. The migration notice also prevents a misleading claim that this standalone repo is the current development location.",
        "pointers": [("README.md", "Development of Wizer", "Upstream move"), ("README.md", "## Caveats", "Initialization restrictions")],
    },
    {
        "repo": "dphilla/weave", "role": "Optional transformed-continuation experiment",
        "license": "Apache-2.0 license inspected.", "license_path": "LICENSE",
        "finding": "The project describes rewriting Wasm control flow so live locals and continuation state can be stored in linear memory. The selected transform entry point validates its generated module. The snapshot container includes module identity, memories, control globals, and host-service blobs. Tests explicitly reject unsupported proposals including table64 and exception tags. The README describes uncertain migration commit outcomes rather than always restarting both copies.",
        "decision": "A concrete lead if handler boundaries become too restrictive. First independently test a tiny supported module and measure code-size/runtime overhead. The README's end-to-end claims were not reproduced here. This is an extra compiler and snapshot-format dependency, not a proven replacement for the capability-aware broker. Host-service blobs still need ownership and external-effect semantics.",
        "pointers": [("crates/weave-transform/src/lib.rs", "pub fn transform", "Transform and output validation"), ("crates/weave-core/src/snapshot.rs", "pub struct Snapshot", "Portable state container"), ("crates/weave-transform/tests/unsupported_features.rs", "fn table64_declarations", "Unsupported-feature rejection"), ("README.md", "uncertain commit", "Migration outcome boundary")],
    },
    {
        "repo": "haoyuwang99/AgentSpec", "role": "Agent DSL comparison, not trusted checker code",
        "license": "No license-named file found in the recorded repository tree; reuse terms unresolved.",
        "finding": "The grammar encodes triggers, predicates, and enforcement actions. The enforcement implementation has stop/skip operations, interactive user inspection, and an LLM self-reflection path. Therefore, a typed language in this family can still call nondeterministic decision procedures.",
        "decision": "Use the paper and language design as related work. Implement a much smaller deterministic acceptance core for MnemosyneOS, with no LLM calls or interactive predicates inside authorization. Planner generation may remain nondeterministic; acceptance and enforcement must have explicit semantics. The repository was inspected as a research artifact, not selected as a dependency.",
        "pointers": [("src/spec_lang/AgentSpec.g4", "grammar AgentSpec", "Policy grammar"), ("src/enforcement.py", "class LLMSelfReflect", "LLM-based enforcement path"), ("src/enforcement.py", "class UserInspection", "Interactive enforcement path")],
    },
]


def main():
    lines = [
        "# MnemosyneOS source-reading guide",
        "",
        "Inspected on 8 October 2026. Revisions below are the recorded default-branch commits, not automatically the versions used in papers or recommended releases. Findings come from targeted source and documentation reading. Downloading a file is not a complete audit; no external repository was compiled, tested, or benchmarked in this review.",
        "",
        "Use the [deep research review](C:/Users/ludic/Desktop/mnemosyne/docs/deep-research.md) for the literature synthesis and proposed protocol. The [source manifest](C:/Users/ludic/Desktop/mnemosyne/research/source-manifest.json) records all collected files, immutable URLs, and content digests. Source links below have line anchors checked against the cached revision.",
        "",
        "## Pinned repositories",
        "",
        "| Repository | Recorded commit | Commit date | Intended role |",
        "| --- | --- | --- | --- |",
    ]
    for profile in PROFILES:
        repo = profile["repo"]
        inv = json.loads((ROOT / "cache" / repo.replace("/", "__") / "inventory.json").read_text(encoding="utf-8"))
        sha = inv["sha"]
        lines.append(f"| [{repo}](https://github.com/{repo}) | [{sha[:12]}](https://github.com/{repo}/tree/{sha}) | {inv['commit_date'][:10]} | {profile['role']} |")
    lines += ["", "## What to inspect and how to use it", ""]
    for number, profile in enumerate(PROFILES, 1):
        repo = profile["repo"]
        lines += [f"### {number}. {repo}", "", f"**Observed:** {profile['finding']}", "", f"**Decision:** {profile['decision']}", "", "Source entry points:", ""]
        for path, needle, label in profile["pointers"]:
            lines.append("- " + link(repo, path, needle, label))
        license_text = profile["license"]
        if "license_path" in profile:
            license_text += " " + link(repo, profile["license_path"], label="Inspected license text")
        else:
            inv = json.loads((ROOT / "cache" / repo.replace("/", "__") / "inventory.json").read_text(encoding="utf-8"))
            license_text += f" [Recorded tree](https://github.com/{repo}/tree/{inv['sha']})"
        lines += ["", f"**License observation:** {license_text}", ""]
    lines += [
        "## Reuse shortlist",
        "",
        "| Choice | Recommendation | Gate before adoption |",
        "| --- | --- | --- |",
        "| Wasmi | First runtime candidate | Pinned freestanding cross-build, validation profile, bounded host imports, replay experiment. |",
        "| redb | Storage comparison/fallback experiment | Exact feature set, adapter durability contract, allocator/lock portability, namespace and branch semantics. |",
        "| Weave | Optional later continuation experiment | Independent semantic tests and overhead measurements; supported-proposal contract. |",
        "| seL4, Microkit, Hubris | Kernel/service design references | Implement and validate MnemosyneOS's own lifecycle and authority model. |",
        "| Theseus, RedLeaf | State ownership and failure-boundary references | Do not substitute language isolation for the proposed native-code threat boundary. |",
        "| littlefs, FSCQ, verified-storage | Storage testing/specification references | Translate media assumptions and preserve distinction between code, model, and proof. |",
        "| rr, Golem | Replay/effect semantics references | Avoid treating host-specific machinery as a guest library. |",
        "| AgentSpec | Related work and DSL comparison | Deterministic, bounded checker implemented separately. |",
        "| Wizer | Possible initialization optimization | Follow its new upstream location; does not solve general live recovery. |",
        "",
        "## Reading is not reproduction",
        "",
        "To reproduce a paper, use its stated artifact branch, compiler, configuration, workload, fault model, and hardware assumptions. In particular, FSCQ's current master is associated with DFSCQ, the PoWER artifact uses a dedicated branch, and Wizer development moved upstream. Main-branch source inspections answer implementation questions but do not independently validate published performance or proofs.",
        "",
        "License entries describe the files observed, not all transitive dependencies or a completed reuse review. No project code from these repositories has been incorporated into the MnemosyneOS kernel by this research task.",
        "",
    ]
    target = ROOT.parent / "docs" / "source-reading-guide.md"
    target.write_text("\n".join(lines), encoding="utf-8")
    print(json.dumps({"output": str(target), "repos": len(PROFILES), "collected_files": len(MANIFEST), "source_entry_points": sum(len(p['pointers']) for p in PROFILES), "words": len(target.read_text(encoding='utf-8').split())}))


if __name__ == "__main__":
    main()
