// Editable PowerPoint source. Requires the Codex bundled @oai/artifact-tool runtime.
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, i, all) => {
  if (value.startsWith('--')) pairs.push([value.slice(2), all[i + 1]]);
  return pairs;
}, []));
for (const key of ['runtime-modules', 'skill-dir', 'python']) {
  if (!args[key]) throw new Error(`Missing --${key}`);
}
// The bundled finalizer uses this documented variable in its import subprocess.
process.env.RUNTIME_NODE_MODULES = args['runtime-modules'];
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TMP = path.join(ROOT, 'build', 'pitch');
const OUT = path.join(ROOT, 'presentation', 'output');
await fs.mkdir(TMP, { recursive: true });
await fs.mkdir(OUT, { recursive: true });
const { Presentation, PresentationFile } = await import(pathToFileURL(path.join(args['runtime-modules'], '@oai/artifact-tool/dist/artifact_tool.mjs')).href);
const { finalizePresentation } = await import(pathToFileURL(path.join(args['skill-dir'], 'container_tools/artifact_tool_utils.mjs')).href);
const evidence = JSON.parse(await fs.readFile(path.join(ROOT, 'presentation/evidence/replay-demo.json'), 'utf8'));
const manifest = JSON.parse(await fs.readFile(path.join(ROOT, 'presentation/evidence/manifest.json'), 'utf8'));
const baseline = manifest.code_commit.slice(0, 7);
const C = { ink: '#122632', paper: '#F6F4EE', white: '#FCFCF8', mint: '#B8D8C6', teal: '#146B5F', copper: '#B35B33', muted: '#58656A', pale: '#E9EDE7', line: '#D4D9D1' };
const SOURCES = {
  kernel: 'Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline ' + manifest.code_commit,
  capabilities: 'Repository: crates/capability/src/lib.rs and its eight unit tests. Recorded output: presentation/evidence/tests.log.',
  replay: 'Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: ' + manifest.captured_at_utc,
  plan: 'Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.',
  sel4: 'Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf',
  theseus: 'Boos et al. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos',
  rifl: 'Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf',
  crash: 'Mohan et al. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan',
  llm: 'Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484, 2024. https://arxiv.org/html/2411.14484v1',
  reuse: 'Repository: THIRD_PARTY.md and Cargo.lock. Upstream: https://github.com/limine-bootloader/limine and https://github.com/rust-osdev/x86_64 and https://github.com/wasmi-labs/wasmi. Copied template notice: third-party/licenses/limine-rust-template-0BSD.txt.',
  demo: 'Repository: tools/present.py, tools/dev.py and presentation/demo-runbook.md. Rehearsal evidence and checksums: presentation/evidence/manifest.json.',
};

const slides = [
  { type: 'cover', title: 'MnemosyneOS', subtitle: 'A research microkernel for isolation\nand deterministic service replay', note: 'Research proposal and working prototype\nOctober 2026', notes: 'Introduce MnemosyneOS as a research operating-system project. Its intended contribution joins capability authority, repeatable service recovery, and a deterministic check on AI-proposed actions. Today there are two working foundations: an x86-64 Rust kernel and a separate host Wasmi replay experiment. The full recovery system remains a proposed implementation target. The title artwork is a conceptual illustration, not a hardware photograph or program output.', refs: ['plan'] },
  { type: 'statement', title: 'Research objective', statement: 'Can a failed service recover its state\nwithout regaining revoked authority\nor repeating committed effects?', body: 'MnemosyneOS studies a capability-secure microkernel, deterministic service replay, and policy-checked recovery proposals.', aside: 'The recovery protocol is the central research question. The custom kernel provides the experimental environment.', notes: 'Frame this as a question to investigate, not as a solved claim. Correct restoration has several obligations: reconstruct state, preserve current permissions, respect resource limits, and account for external effects. An AI proposer is useful only if a trusted mechanism can reject unsafe suggestions. A system that merely restarts a service or reproduces a log would not establish this full contribution.', refs: ['plan', 'rifl'] },
  { type: 'rows', title: 'Why service recovery is difficult', rows: [
    ['Containment', 'A component failure must leave unrelated services able to run.'],
    ['State', 'A restart needs a consistent checkpoint and an agreed input history.'],
    ['Authority', 'Recovery must use permissions that remain valid now.'],
    ['Effects', 'A retry can repeat an operation that another service already committed.'],
  ], foot: 'Research problem. The current prototype tests a deliberately restricted recovery case.', notes: 'Use the example of a key-value service that commits an increment and then crashes before replying. A client retry must have a defined outcome. Separately, a checkpoint may contain a capability that an administrator has since revoked. Restoring the old bytes cannot authorize reuse of that capability. These are protocol questions rather than just memory-copying questions.', refs: ['plan', 'rifl', 'theseus'] },
  { type: 'table', title: 'Scope and implementation status', heads: ['Component', 'Current state', 'Next acceptance gate'], widths: [320, 260, 572], rowHeight: 43, font: 23, rows: [
    ['Rust microkernel', 'Booting', 'Process lifecycle and syscall boundary'],
    ['Processes and IPC', 'Mapping tested', 'Ring 3, preemption, bounded endpoints'],
    ['Capabilities', 'Table tested', 'Enforce handles across processes'],
    ['Service isolation', 'Planned', 'A fault leaves a sibling service running'],
    ['ChronicleFS', 'Planned', 'Persistent roots and reboot recovery'],
    ['Wasm runtime', 'Host prototype', 'Move runtime into a userspace service'],
    ['Checkpoint / replay', 'Host prototype', 'Durable logs and guest integration'],
    ['Recovery verifier', 'Planned', 'Check typed plans before execution'],
    ['Fault tests / dashboard', 'CLI tests', 'Measured fault campaign and host UI'],
  ], foot: 'Current implementation baseline: ' + baseline + '. Full-system recovery remains a completion target.', notes: 'Read this slide as the status map for the whole proposal. The kernel already boots and handles memory/protection tests. The capability table runs both in host tests and in a kernel smoke check. Wasm and replay run only on the host. There are no ring-3 services, ChronicleFS implementation, AI model calls, recovery DSL compiler, or benchmark dashboard in this baseline.', refs: ['kernel', 'capabilities', 'replay', 'plan'] },
  { type: 'table', title: 'Proposed system architecture', heads: ['Layer', 'Responsibility', 'Authority boundary'], widths: [265, 505, 382], rowHeight: 92, font: 25, rows: [
    ['Wasm workloads', 'Run bounded applications through a small import ABI', 'No ambient device or file access'],
    ['Userspace services', 'IPC, ChronicleFS, checkpoints and service supervision', 'Explicit delegated capabilities'],
    ['Recovery control', 'Propose, verify, stage and publish recovery actions', 'The proposer cannot execute directly'],
    ['Rust microkernel', 'Address spaces, scheduling, endpoints and object handles', 'Kernel enforces each privileged operation'],
  ], foot: 'Target architecture. The implemented kernel and host runtime are currently separate.', notes: 'Keep scheduling and capability enforcement in the kernel, while services own storage and recovery policy. The AI proposer is outside the trusted decision path. The deterministic verifier and executor need a narrow, reviewable interface and must check the current state when committing an action. The host harness observes and injects faults for evaluation. This table describes the intended architecture, not current process deployment.', refs: ['plan', 'sel4'] },
  { type: 'table', title: 'Capability table semantics', heads: ['Operation', 'Example', 'Checked outcome'], widths: [270, 445, 437], rowHeight: 75, font: 25, rows: [
    ['Mint root', 'Memory object 7 with READ + DELEGATE', 'Trusted object manager only'],
    ['Delegate', 'Child receives READ', 'Rights can only decrease'],
    ['Resolve', 'Use READ handle to request WRITE', 'Denied'],
    ['Revoke', 'Remove a derivation subtree', 'Descendant handles become invalid'],
    ['Reuse slot', 'Slot receives a new generation', 'Old handle cannot alias the new object'],
  ], foot: 'Implemented table behavior. Caller-table selection and object lifecycle enforcement still need integration.', notes: 'Explain that a handle is local to the caller\'s kernel-owned table. Knowing an integer from another process must not itself grant authority. Root minting is trusted, and a generation number prevents stale handles from validating after a slot is reused. Current subtree revocation scans a bounded table synchronously in O(N squared). Before recycling an object ID, trusted object teardown must invalidate references in every relevant table. These are prototype contracts, not a system-wide proof.', refs: ['capabilities'] },
  { type: 'rows', title: 'Processes, preemption and IPC', rows: [
    ['Address spaces', 'Enter ring 3 with separate code, data, stack and kernel entry state.'],
    ['Scheduling', 'Preempt a spinning process on one CPU and keep a peer responsive.'],
    ['Endpoints', 'Use bounded copy-based messages with capability checks on each call.'],
    ['Fault delivery', 'Terminate or restart only the affected service and report a structured fault.'],
  ], foot: 'Next kernel milestone. Timer interrupts currently work, but process preemption is not implemented.', notes: 'The first real isolation demo should use two ring-3 services and an unrelated healthy service. Demonstrate unauthorized memory access, invalid endpoint handles, oversized messages, full queues, and a dead receiver. Keep the first version single-core to make scheduling and replay assumptions explicit. Shared writable memory, DMA, SMP, and general device recovery would broaden the trusted-state problem substantially.', refs: ['plan', 'kernel'] },
  { type: 'metrics', title: 'The current Wasm execution profile', metrics: [['64 KiB', 'fixed linear memory'], ['10,000', 'fuel per request'], ['4,096', 'maximum trace events']], body: 'Wasmi 2.0.0 runs one reviewed integer key-value fixture.\nIts state consists of linear memory and one mutable i64 global.', detail: 'No threads, tables, floating point or start function. Host imports provide clock, random and an in-memory effect observation.', foot: 'Host prototype. These limits describe this fixture, not a universal Wasm execution guarantee.', notes: 'Reusing Wasmi gives us upstream validation, interpretation, resource limits, and fuel metering. The public snapshot API accepts only the bundled fixture so that hidden mutable state cannot silently escape capture. The extra eight bytes for the exported global make the mutable-state payload 65,544 bytes. Clock and random inputs are synthetic controlled providers. The effect import does not send anything over a network or to a persistent filesystem.', refs: ['replay', 'reuse'] },
  { type: 'replayConcept', title: 'Checkpoint and deterministic replay', body: 'A checkpoint preserves the fixture state\nat a completed-handler boundary.', detail: 'Record the subsequent input history. Restore into a fresh instance. Inject recorded inputs and compare replies and effects.', foot: 'Implemented with full state copies in the host fixture. COW and durable recovery are planned.', notes: 'The checkpoint contains both memory and the mutable version counter. It binds the module hash, runtime profile, schema, and state digest. Replay restores a fresh instance whose live providers have different seeds. The tests confirm that those providers are never consulted during replay. Snapshots do not capture an active interpreter stack or suspended host call. The illustration expresses the concept and is not a trace or measurement.', refs: ['replay'] },
  { type: 'table', title: 'A strict trace contract', heads: ['Event', 'Replay action', 'Failure response'], widths: [250, 480, 422], rowHeight: 69, font: 25, rows: [
    ['Request', 'Compare key and delta', 'Reject a changed invocation'],
    ['Clock', 'Inject the recorded value', 'Reject a missing or wrong event'],
    ['Random', 'Check bound and inject value', 'Reject invalid bounds or results'],
    ['Effect', 'Compare the complete payload', 'Discard pending effects on failure'],
    ['Reply', 'Compare the result', 'Poison the instance on mismatch'],
  ], foot: 'Completion also requires consuming the entire log. Extra trailing events fail replay.', notes: 'Each successful request contributes five events. A trap or mismatch leaves partially mutated guest state unusable. The worker refuses further execution, new snapshots, and a valid recording until a fresh instance is restored. A successful handler appends its effects only to an in-memory observation list. The test suite removes every event position in a recorded segment and requires rejection. This is validation against a trace, not authentication of the trace.', refs: ['replay'] },
  { type: 'rows', title: 'ChronicleFS and durable recovery', rows: [
    ['Immutable roots', 'A small versioned filesystem exposes snapshots through committed roots.'],
    ['Commit protocol', 'Define write ordering, checksums and device flush assumptions explicitly.'],
    ['Recovery descriptor', 'Bind the filesystem root, checkpoint, input position and pending effects.'],
    ['Crash testing', 'Interrupt every persistence transition and check state after reboot.'],
  ], foot: 'Proposed storage design. ChronicleFS and power-loss recovery are not implemented in the baseline.', notes: 'Start with a minimal append-only storage design and a narrow filesystem interface. A snapshot pointer is meaningful only when the data it references is durably ordered under a stated block-device model. Reboot testing must check the last committed root rather than relying on a normal shutdown. Research on bounded crash testing informs the test strategy. Garbage collection must also preserve data and completion records still needed by recovery.', refs: ['plan', 'crash'] },
  { type: 'code', title: 'AI-assisted recovery proposals', code: 'plan RecoveryPlanV1\n  service: kv\n  checkpoint: approved_snapshot\n  requires: current_capability_epoch\n  limits: {steps: 4, replay_events: 4096}\n  actions: [quiesce, restore, replay, validate]\n  publish: only_if_authorized_and_valid\n  on_failure: preserve_safe_state_and_stop', sideTitle: 'Proposed control flow', side: 'Structured fault trace\n\nUntrusted plan proposal\n\nDeterministic verification\n\nTrusted staged execution', foot: 'Illustrative plan syntax only. The DSL, verifier, executor and AI adapter remain planned.', notes: 'The model receives a bounded structured fault trace and proposes a small typed action sequence. The model does not own a raw shell or a privileged kernel API. The example shows the intended vocabulary, not an implemented grammar. Begin with hand-authored plans and adversarial tests. Integrate a model only after verifier/executor behavior is explicit. The LLM-Modulo work motivates the separation of candidate generation and external checking, but it is not a proof of this OS design.', refs: ['plan', 'llm'] },
  { type: 'table', title: 'Deterministic verification obligations', heads: ['Obligation', 'Required decision before execution'], widths: [340, 812], rowHeight: 65, font: 25, rows: [
    ['Capability authority', 'Every referenced object and operation is authorized now.'],
    ['No amplification', 'The plan cannot mint stronger privileges during recovery.'],
    ['Resource bounds', 'Memory, handles, log capacity and fuel fit reserved budgets.'],
    ['Bounded execution', 'Each action terminates or reaches a defined time/resource limit.'],
    ['Rollback availability', 'A permitted safe state remains available if a staged step fails.'],
    ['State freshness', 'Execution rejects stale epochs and changed preconditions.'],
  ], foot: 'Proposed verifier specification. Parsing a typed plan alone does not establish these properties.', notes: 'The authority and resource checks must hold at execution time. A plan that was valid when analyzed can become stale after a capability revocation, resource allocation, or service generation change. Reserve resources and bind execution to the checked state or revalidate atomically. Rollback must not resurrect revoked capabilities or contradict effects already visible to peers. A deterministic rejection path is required when an action cannot meet its preconditions.', refs: ['plan', 'sel4', 'rifl'] },
  { type: 'metrics', title: 'Working prototype evidence', metrics: [['24', 'passing host tests'], ['3', 'CPU protection probes'], ['12', 'requests replayed exactly']], body: 'The kernel boots in debug and release configurations.\nThe host experiment rejects altered traces and corrupted state.', detail: 'The rehearsal captures real command output and a reproducible replay report. Performance evaluation remains a later milestone.', foot: 'Recorded code baseline ' + baseline + '. Correctness evidence applies to the stated prototype scope.', notes: 'There are eight capability tests and sixteen runtime/replay tests. The three CPU probes test NX execution, read-only writes with supervisor write protection, and access after unmapping. They operate in the boot address space. Both build modes complete the smoke path. The replay report compares the complete Snapshot object as well as showing its digest, so the displayed equality is not merely a manually matched string. These results do not establish isolation between processes or durable recovery.', refs: ['kernel', 'capabilities', 'replay'] },
  { type: 'demo', title: 'Live demo through the CLI', commands: [['1', 'python tools/dev.py boot', 'Boot the kernel and inspect serial events'], ['2', 'python tools/dev.py test', 'Check capability and replay failure cases'], ['3', 'python tools/dev.py demo', 'Restore, replay and compare complete state']], foot: 'Guided route: python tools/present.py. The current product interface is terminal output.', notes: 'Switch to PowerShell in C:\\Users\\ludic\\Desktop\\mnemosyne. Run python tools/present.py and press Enter before each step. Explain that the kernel runs in QEMU and the Wasmi lab runs separately on the host. After the final report, return to the deck. For WSL failure use --host-only and say the boot was skipped. For a fully offline recorded fallback use --recorded and identify it as saved evidence. Detailed narration and troubleshooting are in presentation/demo-runbook.md.', refs: ['demo'] },
  { type: 'code', dark: true, title: 'Kernel boot evidence', code: '{"event":"frames_ok","count":2}\n{"event":"capabilities_ok"}\n{"event":"paging_ok","fault_probes":3}\n{"event":"exception_ok","vector":3}\n{"event":"timer_ok","ticks":5}\n{"event":"boot_ok"}', sideTitle: 'What the events establish', side: 'Usable frames\n\nCapability table behavior\n\nCPU protection faults\n\nException and timer delivery', foot: 'Excerpt from the saved serial log. Full log: presentation/evidence/boot.log.', notes: 'This is an editable transcription of actual recorded output, not a screenshot of a GUI. The live command produces the same class of events. Usable RAM counts can change with build layout and are intentionally omitted here. The runner also checks QEMU\'s expected exit status. Explicitly distinguish interrupt delivery from process preemption. The general page-fault behavior is still fatal outside the reserved test probes.', refs: ['kernel'] },
  { type: 'result', dark: true, title: 'Replay reaches the same fixture state', hash: evidence.final_state_sha256, labels: ['Final state SHA-256', 'Restored and replayed SHA-256'], metrics: [['12', 'requests'], ['60', 'events'], ['65,544 B', 'mutable state']], body: 'State equal: true    Effects equal: true\nMissing, reordered and extra events: rejected\nCorrupted checkpoint: rejected', foot: 'Actual host experiment result. Checkpoint metadata is additional to the mutable-state payload.', notes: 'The two displayed digests come directly from the saved replay JSON. The report additionally compares full snapshot contents and observed effects. Twelve requests each record five events. The checkpoint payload consists of one 64-KiB memory plus one eight-byte mutable global. The replacement instance uses different provider seeds but consumes recorded input values. There are no external effects, durable commits, or runtime upgrades in this test.', refs: ['replay'] },
  { type: 'table', title: 'Reuse and research foundations', heads: ['Foundation', 'Role in MnemosyneOS'], widths: [380, 772], rowHeight: 66, font: 25, rows: [
    ['Limine and Rust OSDev', 'Reused boot, CPU, UART, interrupt and paging support'],
    ['Wasmi and wasm-tools', 'Reused interpreter, validation, fuel and WAT compilation'],
    ['seL4', 'Reference for capability design and precise proof scope'],
    ['Theseus', 'Reference for component state ownership and recovery'],
    ['RIFL and CrashMonkey', 'References for effect handling and crash-test design'],
    ['LLM-Modulo planning', 'Reference for proposer/checker separation'],
  ], foot: 'Dependencies are pinned and attributed. Research references are not automatically vendored code.', notes: 'Reuse is intentional: the custom work should focus on the experimental security/recovery contracts. Cargo.lock records dependency versions and checksums. Copied Limine template code and boot binaries retain notices. seL4 and other research systems are design references, not claimed bases of our verified implementation. Theseus highlights state ownership, RIFL treats duplicate request completion, and CrashMonkey uses bounded crash workloads. Our candidate contribution is their interaction with current authority in a new recovery protocol, not a claim to invent restart or replay.', refs: ['reuse', 'sel4', 'theseus', 'rifl', 'crash', 'llm'] },
  { type: 'table', title: 'Evaluation plan', heads: ['Measure', 'Controlled experiment'], widths: [345, 807], rowHeight: 66, font: 25, rows: [
    ['IPC and capability cost', 'Vary payload size, rights checks and table occupancy'],
    ['Wasm performance', 'Run the same workload natively and in the interpreter'],
    ['Checkpoint overhead', 'Measure pause time, copied bytes and write amplification'],
    ['Replay fidelity', 'Compare state, replies and effect records after injected faults'],
    ['Failure containment', 'Track healthy-service progress while a peer fails'],
    ['Recovery time', 'Compare restart, checkpoint restore, replay and checked plans'],
  ], foot: 'Proposed experiments. Report distributions and raw artifacts. No performance claims yet.', notes: 'Measure p50 and p95/p99 where appropriate, report sample sizes, and keep QEMU correctness runs separate from hardware timing. Include mean recovery time but do not let the mean hide tail latency. Split recovery into detection, restore, replay, validation and publication. For the AI control plane, record valid-plan rate, rejection reasons, latency and success under the same fault cases as a rule-based baseline. Fix workloads and seeds before comparing alternatives. Healthy-service progress and absence of duplicated effects are correctness gates.', refs: ['plan'] },
  { type: 'table', title: 'Implementation milestones', heads: ['Gate', 'Demonstration required to advance'], widths: [340, 812], rowHeight: 75, font: 25, rows: [
    ['1  Kernel isolation', 'Two ring-3 services, preemption and capability-checked IPC'],
    ['2  Runtime service', 'The Wasm fixture runs inside an isolated guest service'],
    ['3  Durable state', 'ChronicleFS recovers its last committed root after a crash'],
    ['4  Recovery protocol', 'Restore and replay without duplicated committed effects'],
    ['5  Checked planning', 'Reject forbidden plans and evaluate recovery baselines'],
  ], foot: 'Proposed sequence. Calendar estimates follow scope and team-capacity review.', notes: 'Acceptance gates make the schedule evidence-based. A working CLI harness comes before a dashboard. Start with one CPU and one restricted workload, then add complexity only when tests establish the preceding contract. The current foundation already covers boot, page protection, table semantics and host replay. Each gate produces a small demonstrable increment and a testable failure case. Avoid promising SMP, broad POSIX compatibility or arbitrary native replay in the first complete version.', refs: ['plan'] },
  { type: 'table', title: 'Main engineering risks', heads: ['Risk', 'Design response'], widths: [345, 807], rowHeight: 85, font: 25, rows: [
    ['Scope exceeds capacity', 'Use staged gates and a single first workload'],
    ['Hidden nondeterminism', 'Mediate host inputs and restrict shared mutable state'],
    ['Duplicate external effects', 'Design stable IDs, durable completion records and peer handling'],
    ['Stale authorization', 'Bind recovery to current epochs and recheck before publication'],
    ['Overstated assurance', 'Keep tests, policy checking and formal proofs separate'],
  ], foot: 'The full protocol must address these risks before claiming safe autonomous recovery.', notes: 'A microkernel, filesystem, runtime, replay system and AI control plane can each become a large independent project. The controlled workload reduces the initial state space. External effects need a receiver protocol and cannot be fixed by local snapshots alone. Checkpoint hashes do not establish authenticity or semantic correctness. The first full-system claim should be limited to the supported workload, fault model, storage model, and authorized recovery operations.', refs: ['plan', 'rifl', 'sel4'] },
  { type: 'closing', title: 'The project deliverable', statement: 'A reproducible testbed for\ncapability-aware service recovery', body: 'Custom Rust kernel and isolated services\nA specified checkpoint and replay protocol\nA checked recovery executor with an untrusted proposer\nA fault suite with reproducible measurements', foot: 'Current foundation: booting kernel, capability tests and exact host-fixture replay.', notes: 'Close with the concrete research output rather than a claim that all nine subsystems are already complete. Ask the teacher to review the scope, protocol invariants and staged acceptance criteria. The working foundation makes the implementation route reviewable. A successful final project should show a service failure, recovery of committed state, rejection of an unauthorized plan, no duplicated committed effects under the supported protocol, and continued progress of a healthy service.', refs: ['plan', 'kernel', 'replay'] },
  { type: 'table', title: 'Technical backup: trust model', heads: ['Party or component', 'Assumption or obligation'], widths: [370, 782], rowHeight: 69, font: 25, rows: [
    ['Wasm application', 'Potentially faulty or malicious within its admitted interface'],
    ['AI proposer', 'Untrusted and unable to bypass deterministic checks'],
    ['Microkernel', 'Trusted enforcement of memory, CPU and object authority'],
    ['Verifier and executor', 'Trusted state checks, bounded actions and publication logic'],
    ['Log and storage service', 'Explicit integrity and persistence assumptions'],
    ['Hardware and boot chain', 'Part of the stated trusted environment'],
  ], foot: 'Proposed threat boundary. Whole-system compromise, side channels and physical attacks need separate scope.', notes: 'Define the adversary instead of saying the system is secure without qualification. For the prototype, trace data and snapshots are generated by the experiment itself. The checksum detects corruption but an attacker who rewrites both data and digest is outside that integrity model. The final protocol must decide where authentication is required. Rust reduces some memory-safety risks, while unsafe hardware access, capability authority, and crash consistency still require separate reasoning.', refs: ['plan', 'sel4', 'replay'] },
  { type: 'rows', title: 'Technical backup: replay boundaries', rows: [
    ['Safe point', 'Capture state only between completed handlers in the first runtime.'],
    ['State inventory', 'Account for globals, memory, queues, handles and pending operations.'],
    ['External effects', 'Bind publication to durable state and receiver-side duplicate handling.'],
    ['Concurrency', 'Coordinate related service histories before claiming group recovery.'],
  ], foot: 'Current lab covers one fixture and in-memory observations. General native replay is outside this milestone.', notes: 'The current fixture has no tables, threads, files, sockets, native continuations or suspended host calls. Adding any of these changes the snapshot schema. Restoring one service may conflict with messages already observed by a peer, so service-group recovery needs a consistent history and output publication rules. Exactly-once claims depend on the full receiver and durability protocol. At present the worker does not switch a replay into live execution.', refs: ['plan', 'replay', 'rifl'] },
  { type: 'table', title: 'Technical backup: capability edge cases', heads: ['Case', 'Current rule'], widths: [380, 772], rowHeight: 69, font: 25, rows: [
    ['Stale slot handle', 'Reject mismatched generation'],
    ['Generation exhaustion', 'Retire the slot permanently'],
    ['Delegation amplification', 'Reject rights outside the parent subset'],
    ['Full table', 'Fail allocation without changing existing authority'],
    ['Subtree deletion', 'Delete descendants before dangling ancestry can arise'],
    ['Object ID reuse', 'Trusted teardown must invalidate all referencing tables'],
  ], foot: 'Implemented allocation-free table. Revocation is synchronous O(N²) within a bounded capacity.', notes: 'These failure cases explain why an integer permission flag is insufficient. The test suite includes exhaustive valid rights-subset combinations for memory capabilities. Descendant membership is computed before deleting ancestors so that grandchildren are not missed. Generation exhaustion does not wrap to an earlier identity. The table itself does not establish global object uniqueness or cross-process ownership; those belong to future trusted kernel integration.', refs: ['capabilities'] },
  { type: 'code', title: 'Technical backup: demo commands', code: "Set-Location 'C:\\Users\\ludic\\Desktop\\mnemosyne'\npython tools/present.py\n\n# Rehearsal without pauses\npython tools/present.py --auto\n\n# Host tests and replay only\npython tools/present.py --host-only\n\n# Saved evidence, clearly labeled\npython tools/present.py --recorded", sideTitle: 'Preparation', side: 'Warm the build cache\n\nUse a readable terminal font\n\nOpen the demo guide\n\nKeep the PDF as a fallback', foot: 'Exact setup, expected output and troubleshooting: presentation/demo-runbook.md.', notes: 'The guided script runs existing developer commands and records the results under build. It pauses for narration before each step and stops on a nonzero exit. The recorded mode performs no build or VM run and announces its capture date and code baseline. WSL and QEMU are needed only for the kernel portion; native host tests and the Wasmi demo need no VM. Rehearse while connected so Rust dependencies and the pinned bootloader are already cached.', refs: ['demo'] },
  { type: 'qa', title: 'Technical backup: likely review questions', items: [
    ['Why a custom kernel?', 'The course artifact exposes the authority and recovery boundary directly.'],
    ['Why Rust?', 'Use ownership and types while keeping unsafe hardware code small and reviewable.'],
    ['Why Wasm first?', 'A restricted import ABI gives a tractable state and input model.'],
    ['What does AI add?', 'Candidate recovery strategies under the same deterministic enforcement boundary.'],
    ['What is the contribution?', 'Evaluate recovery semantics that combine current authority, history and effects.'],
    ['What proves completion?', 'A fault-and-recovery scenario with preserved state, authority and peer progress.'],
  ], foot: 'The thesis and evaluation are proposed. Existing tests establish the current prototype behavior.', notes: 'If asked why not simply build on seL4, explain that a seL4-based implementation is a credible alternative when the recovery experiment is the only research goal. A custom kernel is justified by the stated project deliverable and learning objective, but increases effort and assurance work. If asked whether the AI is necessary, say that a rule-based baseline is mandatory and the model must demonstrate measurable benefit. Never substitute confident model output for proof or authority.', refs: ['plan', 'sel4', 'llm'] },
  { type: 'table', title: 'Technical backup: assurance claims', heads: ['Evidence or mechanism', 'What it establishes', 'Limit'], widths: [330, 405, 417], rowHeight: 86, font: 24, rows: [
    ['Unit tests', 'Expected behavior for exercised cases', 'No exhaustive whole-system proof'],
    ['CPU fault probes', 'Protection on the test mapping', 'No process-isolation demo yet'],
    ['Snapshot checksum', 'Detection of accidental changes', 'No authenticity or semantic validity'],
    ['Deterministic verifier', 'Planned rejection against a policy', 'Needs a specification and sound executor'],
    ['Formal proof', 'A theorem under explicit assumptions', 'Not completed for MnemosyneOS'],
  ], foot: '“Verified AI-assisted recovery” names the proposed check-before-execution objective.', notes: 'This slide is important for an academic review. Keep empirical evidence, deterministic policy enforcement and machine-checked proof as distinct categories. The kernel is not seL4-verified merely because its design references seL4. Wasmi validation is an upstream feature, not proof of the full embedding or of recovery correctness. The intended verifier must establish concrete obligations under a stated model, and its executor must preserve them while the system changes.', refs: ['plan', 'kernel', 'replay', 'sel4'] },
  { type: 'abstract', title: 'Formal project proposal', formal: 'MnemosyneOS: Design and Implementation of a Capability-Secure Microkernel with Deterministic Service Replay and Verified AI-Assisted Recovery', abstract: 'MnemosyneOS is a research-oriented 64-bit microkernel designed to isolate, monitor and recover autonomous and potentially untrusted workloads. The system implements virtual memory, preemptive process scheduling, capability-based access control, inter-process communication, userspace services, a transactional snapshotting filesystem and a WebAssembly application runtime. Individual services may be checkpointed using copy-on-write memory and replayed using recorded IPC messages and nondeterministic system inputs. An AI-assisted control plane analyzes structured fault traces and proposes recovery actions in a typed policy language. Proposed actions are checked by a deterministic verifier for capability authority, resource bounds, privilege amplification, bounded execution and rollback availability before they are executed. The project evaluates IPC overhead, capability-validation cost, WebAssembly performance, checkpoint overhead, replay fidelity, failure-containment effectiveness and mean recovery time under injected faults.', foot: 'Original proposed title and abstract. Feature statements describe completion targets, not current implementation.', notes: 'This preserves the author\'s supplied project title and abstract. Present it as the proposed final system. For progress reporting, use the implementation status slide and source evidence instead. In a formal submission, retain the proposal status or use future tense until these features are implemented and evaluated.', refs: ['plan'] },
  { type: 'references', title: 'Selected papers and source repositories', items: [
    ['seL4', 'Klein et al., SOSP 2009', 'sel4'],
    ['Theseus', 'Boos et al., OSDI 2020', 'theseus'],
    ['RIFL', 'Lee et al., SOSP 2015', 'rifl'],
    ['CrashMonkey / B3', 'Mohan et al., OSDI 2018', 'crash'],
    ['LLM-Modulo planning', 'Gundawar et al., arXiv 2024', 'llm'],
    ['Implementation reuse', 'Limine, Rust OSDev, Wasmi, wasm-tools', 'reuse'],
  ], foot: 'Full citations and links appear in speaker notes. Repository research docs provide the extended review.', notes: 'The reading guide contains pinned repository revisions and concrete source pointers. These sources support design decisions and limitations, not a claim that MnemosyneOS inherits another project\'s proof or benchmark results. Reuse permissions are recorded per component in THIRD_PARTY.md. Local evidence for this deck is preserved under presentation/evidence with its capture time, code baseline and file checksums.', refs: ['sel4', 'theseus', 'rifl', 'crash', 'llm', 'reuse', 'demo'] },
];

const presentation = Presentation.create({ slideSize: { width: 1280, height: 720 } });
const nativeTables = [];
const allNotes = [];
const W = 1152;

function text(slide, value, left, top, width, height, size = 26, color = C.ink, bold = false, family = 'Arial') {
  const box = slide.shapes.add({ geometry: 'textbox', name: `text-${slide.shapes.items.length + 1}`, position: { left, top, width, height }, fill: 'none', line: { fill: 'none', width: 0 } });
  box.text = value;
  box.text.style = { typeface: family, fontSize: size, bold, color, verticalAlignment: 'top', wrap: 'square', insets: 0 };
  return box;
}

async function image(slide, filename, left, top, width, height, fit = 'contain') {
  const bytes = await fs.readFile(path.join(ROOT, 'presentation/assets', filename));
  slide.images.add({ blob: bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength), contentType: 'image/png', alt: filename === 'cover-memory.png' ? 'Concept artwork of layered memory over a silicon core' : 'Concept artwork of checkpoint state reconstructed from recorded inputs', position: { left, top, width, height }, fit });
}

function table(slide, spec, index) {
  const values = [spec.heads, ...spec.rows];
  const height = 48 + spec.rows.length * spec.rowHeight;
  const t = slide.tables.add({ rows: values.length, columns: values[0].length, left: 64, top: 177, width: W, height, columnWidths: spec.widths, values });
  t.styleOptions = { headerRow: true, bandedRows: false };
  t.borders.assign({ fill: C.line, width: 0.65, style: 'solid' });
  t.cells.block({ row: 0, column: 0, rowCount: values.length, columnCount: values[0].length }).assign({ margins: { left: 14, right: 14, top: 7, bottom: 7 }, anchor: 'center', textStyle: { typeface: 'Arial', fontSize: spec.font ?? 25, color: C.ink } });
  t.rows[0].height = 48;
  for (let r = 0; r < values.length; r++) {
    if (r) t.rows[r].height = spec.rowHeight;
    for (let c = 0; c < values[0].length; c++) {
      const cell = t.getCell(r, c);
      cell.fill = r === 0 ? C.ink : r % 2 ? C.paper : C.pale;
      cell.text.style = { typeface: 'Arial', fontSize: r === 0 ? 23 : spec.font ?? 25, bold: r === 0 || c === 0, color: r === 0 ? C.white : C.ink };
    }
  }
  nativeTables.push(index + 1);
}

for (const [i, spec] of slides.entries()) {
  const slide = presentation.slides.add();
  const dark = spec.dark || ['cover', 'closing'].includes(spec.type);
  slide.background.fill = dark ? C.ink : C.paper;
  const fg = dark ? C.white : C.ink;
  if (spec.type === 'cover') {
    await image(slide, 'cover-memory.png', 0, 0, 1280, 720, 'cover');
    text(slide, spec.title, 64, 184, 650, 104, 76, C.white, true);
    text(slide, spec.subtitle, 68, 310, 594, 150, 33, C.white);
    text(slide, spec.note, 68, 566, 600, 82, 22, C.mint);
  } else {
    text(slide, spec.title, 64, 54, W, 103, 44, fg, true);
    if (spec.type === 'statement') {
      text(slide, spec.statement, 64, 190, W, 225, 43, C.teal, true);
      text(slide, spec.body, 64, 440, 1020, 85, 29);
      text(slide, spec.aside, 64, 562, 1115, 63, 24, C.muted);
    } else if (spec.type === 'rows') {
      spec.rows.forEach(([label, body], row) => {
        const y = 194 + row * 102;
        text(slide, label, 64, y, 280, 74, 29, C.teal, true);
        text(slide, body, 370, y, 828, 82, 28);
      });
    } else if (spec.type === 'table') table(slide, spec, i);
    else if (spec.type === 'metrics') {
      spec.metrics.forEach(([value, label], j) => {
        text(slide, value, 64 + j * 392, 198, 370, 114, 72, C.teal, true);
        text(slide, label, 64 + j * 392, 321, 358, 67, 27);
      });
      text(slide, spec.body, 64, 432, W, 103, 30);
      text(slide, spec.detail, 64, 561, W, 79, 24, C.muted);
    } else if (spec.type === 'replayConcept') {
      text(slide, spec.body, 64, 183, W, 90, 33, C.teal, true);
      await image(slide, 'replay-concept.png', 500, 277, 715, 357, 'cover');
      text(slide, spec.detail, 64, 324, 387, 260, 28);
    } else if (spec.type === 'code') {
      text(slide, spec.code, 64, 186, 807, 451, spec.title.includes('demo commands') ? 23 : 25, dark ? C.mint : C.teal, false, 'Consolas');
      text(slide, spec.sideTitle, 908, 189, 305, 78, 27, fg, true);
      text(slide, spec.side, 908, 281, 305, 351, 25, dark ? C.white : C.muted);
    } else if (spec.type === 'demo') {
      spec.commands.forEach(([num, command, caption], j) => {
        const y = 190 + j * 145;
        text(slide, num, 64, y, 72, 81, 60, C.copper, true);
        text(slide, command, 165, y, 1020, 51, 34, C.teal, false, 'Consolas');
        text(slide, caption, 165, y + 64, 1030, 55, 28);
      });
    } else if (spec.type === 'result') {
      spec.metrics.forEach(([n, label], j) => {
        text(slide, n, 64 + j * 392, 164, 370, 83, 55, C.mint, true);
        text(slide, label, 64 + j * 392, 251, 350, 41, 25, C.white);
      });
      spec.labels.forEach((label, j) => {
        text(slide, label, 64, 333 + j * 100, W, 35, 23, C.mint);
        text(slide, spec.hash, 64, 374 + j * 100, W, 42, 24, C.white, false, 'Consolas');
      });
      text(slide, spec.body, 64, 556, W, 94, 25, C.white);
    } else if (spec.type === 'closing') {
      text(slide, spec.statement, 64, 180, W, 150, 49, C.mint, true);
      text(slide, spec.body, 64, 377, 1100, 233, 32, C.white);
    } else if (spec.type === 'qa') {
      spec.items.forEach(([q, a], j) => {
        const left = 64 + (j % 2) * 588;
        const top = 185 + Math.floor(j / 2) * 157;
        text(slide, q, left, top, 549, 44, 28, C.teal, true);
        text(slide, a, left, top + 51, 549, 91, 25);
      });
    } else if (spec.type === 'abstract') {
      text(slide, spec.formal, 64, 169, W, 125, 29, C.teal, true);
      text(slide, spec.abstract, 64, 312, W, 333, 25);
    } else if (spec.type === 'references') {
      spec.items.forEach(([name, description], j) => {
        text(slide, name, 64, 181 + j * 75, 380, 56, 28, C.teal, true);
        text(slide, description, 454, 181 + j * 75, 745, 56, 27);
      });
    }
    const foot = spec.foot || '';
    text(slide, `${String(i + 1).padStart(2, '0')} / ${slides.length}     ${foot}`, 64, 677, W, 27, 16, dark ? C.mint : C.muted);
  }
  const sources = (spec.refs || []).map(key => SOURCES[key]);
  if (spec.type === 'cover' || spec.type === 'replayConcept') sources.push('Concept artwork generated with the built-in image_gen tool. Prompt and local asset provenance: presentation/assets/provenance.json. Artwork is not empirical evidence.');
  slide.speakerNotes.text = spec.notes + '\n\nSources and evidence\n' + sources.join('\n\n');
  allNotes.push(`## ${i + 1}. ${spec.title}\n\n${spec.notes}\n\n${sources.map(s => '- ' + s).join('\n')}\n`);
}

if (presentation.slides.items.length !== 30) throw new Error('Unexpected slide count');
await fs.writeFile(path.join(TMP, 'presentation.json'), JSON.stringify(presentation.toProto()));
await fs.writeFile(path.join(OUT, 'mnemosyne-os-speaker-notes.md'), '# MnemosyneOS speaker notes\n\nSlides 1-22 are the main presentation. Slides 23-30 are technical backup. See the demo guide for the exact terminal walkthrough.\n\n' + allNotes.join('\n'));
const candidatePath = path.join(TMP, 'candidate.pptx');
await (await PresentationFile.exportPptx(presentation)).save(candidatePath);
for (const [i, slide] of presentation.slides.items.entries()) {
  const blob = await presentation.export({ slide, format: 'png', scale: 1 });
  await fs.writeFile(path.join(TMP, `slide-${String(i + 1).padStart(2, '0')}.png`), new Uint8Array(await blob.arrayBuffer()));
  const layout = await slide.export({ format: 'layout' });
  await fs.writeFile(path.join(TMP, `slide-${String(i + 1).padStart(2, '0')}.layout.json`), await layout.text());
  console.log(`Rendered ${i + 1}/${slides.length}`);
}
const result = await finalizePresentation({
  workspaceDir: ROOT, candidatePath,
  finalPath: path.join(OUT, args.output || 'mnemosyne-os-pitch.pptx'),
  pythonExecutable: args.python,
  integrityValidatorPath: path.join(args['skill-dir'], 'container_tools/inspect_presentation_package_integrity.py'),
  layoutValidatorPath: path.join(args['skill-dir'], 'container_tools/inspect_presentation_layout_geometry.py'),
  layoutArgs: ['--expected-slide-size-emu', '12192000,6858000', '--validate-heading-fit', '--validate-bullet-geometry', ...nativeTables.flatMap(n => ['--require-native-table-slide', String(n)])],
  requiredNativeTableOwnerSlides: nativeTables,
  fontPolicy: { basis: 'design', families: ['Arial', 'Consolas'] },
  verifyArtifactToolImport: true,
  receiptPath: path.join(TMP, 'validation.json'),
});
console.log(JSON.stringify(result));
