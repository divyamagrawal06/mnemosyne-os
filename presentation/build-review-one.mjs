// Editable Review I deck. Uses the bundled @oai/artifact-tool runtime.
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
process.env.RUNTIME_NODE_MODULES = args['runtime-modules'];
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TMP = path.join(ROOT, 'build', 'review-one');
const OUT = path.join(ROOT, 'presentation', 'output');
const outputName = args.output || 'mnemosyne-os-review-one.pptx';
if (path.basename(outputName) !== outputName || !outputName.endsWith('.pptx')) {
  throw new Error('--output must be a new PPTX filename, without directories');
}
await fs.mkdir(TMP, { recursive: true });
await fs.mkdir(OUT, { recursive: true });
const { Presentation, PresentationFile } = await import(pathToFileURL(path.join(args['runtime-modules'], '@oai/artifact-tool/dist/artifact_tool.mjs')).href);
const { finalizePresentation } = await import(pathToFileURL(path.join(args['skill-dir'], 'container_tools/artifact_tool_utils.mjs')).href);
const manifest = JSON.parse(await fs.readFile(path.join(ROOT, 'docs/evidence/manifest.json'), 'utf8'));
const C = { ink: '#122632', paper: '#F6F4EE', white: '#FCFCF8', mint: '#B8D8C6', teal: '#146B5F', copper: '#B35B33', muted: '#58656A', pale: '#E9EDE7', line: '#D4D9D1' };
const SOURCES = {
  review: 'Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.',
  status: 'Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.',
  kernel: 'Repository: kernel/src/main.rs, arch.rs, memory.rs, paging.rs, tasks.rs, user-memory.rs and task-entry.S. The user ELF is compiled into the kernel; no arbitrary application loader is claimed.',
  cap: 'Repository: crates/capability/src/lib.rs and kernel/src/tasks.rs. Eight capability tests, typed handles, generation checks, attenuation/subtree revocation in the table, checked syscalls and endpoint invalidation on restore. No cross-process delegation syscall.',
  fs: 'Repository: crates/chronicle-fs/src/lib.rs, kernel/src/ata.rs and userland/init/src/shell.rs. Eight shared filesystem tests. ATA is in the kernel; filesystem policy runs in trusted userspace init. Flush ordering is an assumption.',
  runtime: 'Repository: crates/service-runtime/src/lib.rs and tests.rs, workloads/kv/service.wat, userland/init/src/shell.rs. Eighteen runtime tests. The fixed profile has full-copy Wasm snapshots, synthetic clock/random inputs and in-memory effects.',
  policy: 'Repository: crates/recovery-policy/src/lib.rs, userland/init/src/shell.rs, tools/ai-plan.py and tools/test-ai-plan.py. Six policy and four adapter tests. The optional online model path has not been evaluated by the offline acceptance run.',
  evidence: `Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture ${manifest['recorded-at']}, source ${manifest['source-commit']}, ISO SHA-256 ${manifest['iso-sha256']}. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.`,
  tests: 'Validation completed for the integrated OS increment: 40 Rust unit tests (8 capability, 8 filesystem, 6 policy, 18 runtime); 17 Python tests (13 dashboard, 4 AI adapter); debug/release kernel boots and two-boot guest acceptance. The presentation update does not rerun all software checks. Reproduce via tools/dev.py test, check, boot and guest-test; Python test files run directly.',
  demo: 'Repository: docs/cloud-demo.md, tools/run-image.py and tools/dev.py. The prebuilt bundle requires Python 3 and QEMU on a non-Azure x86-64 Linux host. Run-image uses a 256-MiB, one-CPU guest without a NIC. The self-test uses fresh disks and two bounded boots.',
  reuse: 'Repository: THIRD_PARTY.md, Cargo.lock and third-party/licenses. Limine bootloader v10.8.5-binary commit 7a9013f305de0bea1f6310e76e7baba30499fef0; Rust OSDev x86_64 0.15.4; linked_list_allocator 0.10.6; Wasmi 2.0.0; wat 1.228.0. https://github.com/limine-bootloader/limine https://github.com/rust-osdev/x86_64 https://github.com/wasmi-labs/wasmi',
  sel4: 'Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf',
  theseus: 'Boos, Liyanage, Ijaz and Zhong. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos',
  rifl: 'Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf',
  crash: 'Mohan, Martinez, Ponnapalli, Raju and Chidambaram. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan',
  llm: 'Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484v1, 2024. https://arxiv.org/html/2411.14484v1',
};

const slides = [
  { type: 'cover', title: 'MnemosyneOS', subtitle: 'A capability-based research OS\nwith deterministic service replay', note: 'Review I\nExecutable prototype / October 2026', notes: 'Introduce the research OS and state that the demonstration now boots its own kernel with protected userspace tasks and persistent storage. The original proposal title ends with Verified AI-Assisted Recovery. In this review, verified refers to implemented deterministic policy checks; no formal proof is claimed. The cover is conceptual artwork, not experimental evidence.', refs: ['review', 'status'] },
  { type: 'statement', title: 'Problem statement', statement: 'A failed service must recover state\nwhile current permissions still apply.', body: 'A restart loses state. An incomplete replay can diverge.\nAn unchecked recovery proposal can exceed its authority.', aside: 'Research question: can bounded checkpoint/replay restore a service predictably while its peers continue running?', notes: 'Use a counter or key-value service as the example. Restoration must account for both state and authority. The current prototype reconstructs a restricted worker and compares its effects in memory. It does not implement an external recipient protocol for exactly-once effects.', refs: ['review', 'rifl'] },
  { type: 'rows', title: 'Objectives', rows: [
    ['Isolation', 'Private user address spaces, preemptive scheduling and capability-checked IPC.'],
    ['Persistence', 'Atomic filesystem roots, snapshots and a recovery bundle that survives reboot.'],
    ['Recovery', 'Native COW replay and a fresh Wasm worker with matching state and effects.'],
    ['Authority', 'A typed proposal must pass deterministic policy checks before execution.'],
  ], foot: 'Acceptance uses executable guest scenarios and failure-oriented shared-code tests.', notes: 'The kernel is the experimental environment for recovery. Explain acceptance as observable behavior rather than a feature list: a CPU-hog is preempted, a sibling survives a fault, a file survives reboot, stale handles fail, and a recovered worker accepts a new request.', refs: ['review', 'status'] },
  { type: 'table', title: 'Background: kernel structure and state', heads: ['Study', 'Design lesson', 'Project boundary'], widths: [275, 425, 452], rowHeight: 166, rows: [
    ['seL4\nKlein et al., 2009', 'Keep authority and kernel mechanisms explicit, with precise correctness obligations.', 'Tests and runtime checks do not inherit seL4’s formal proof.'],
    ['Theseus\nBoos et al., 2020', 'State ownership and component boundaries influence recovery.', 'Native services are isolated, but filesystem and runtime management still share init.'],
  ], foot: 'Primary papers inform the design. MnemosyneOS uses its own kernel implementation.', notes: 'seL4 is a reference for carefully defined kernel authority and verification obligations. Theseus explores a different Rust OS structure with strong component state boundaries. Our implementation does not reuse their proofs, structures wholesale, or reported results.', refs: ['sel4', 'theseus', 'status'] },
  { type: 'table', title: 'Background: recovery and verification', heads: ['Study', 'Design lesson', 'Current application'], widths: [275, 425, 452], rowHeight: 127, rows: [
    ['RIFL\nLee et al., 2015', 'Retries need operation identity and durable completion handling.', 'External exactly-once effects remain outside the prototype.'],
    ['CrashMonkey / B3\nMohan et al., 2018', 'Bound failure cases and inspect recovered persistent state.', 'Interrupt commit operations and corrupt stored roots in tests.'],
    ['LLM-Modulo\nGundawar et al., 2024', 'Separate proposal generation from external validity checks.', 'The model proposes. Trusted code checks a narrow recovery action.'],
  ], foot: 'These are design implications, not claims of reproduced paper evaluations.', notes: 'RIFL explains why replay equality by itself cannot solve distributed duplicate effects. CrashMonkey motivates bounded crash testing, though its tool is not ported here. LLM-Modulo motivates separating generation from checking. Soundness of the checker and correctness of execution still require their own argument.', refs: ['rifl', 'crash', 'llm'] },
  { type: 'table', title: 'Reuse and project contribution', heads: ['Upstream component', 'Reused implementation', 'Project integration'], widths: [310, 395, 447], rowHeight: 91, rows: [
    ['Limine', 'Boot protocol and bootloader', 'Kernel startup and physical memory setup'],
    ['Rust OSDev crates', 'CPU tables, serial, PIC and heaps', 'Page mappings, trap ABI and task scheduling'],
    ['Wasmi 2.0.0', 'Validation, interpretation and fuel', 'Restricted profile, checkpoint and replay binding'],
    ['Project code', 'Original integration and protocols', 'Capabilities, ChronicleFS, recovery policy and shell'],
  ], foot: 'Pinned versions, license notices and adaptations are recorded in THIRD_PARTY.md.', notes: 'Distinguish using a library from claiming to have written an interpreter or bootloader. The project contribution is the integrated OS and its constrained recovery protocol. THIRD_PARTY.md and Cargo.lock record the reuse boundary and exact resolved dependencies.', refs: ['reuse'] },
  { type: 'architecture', title: 'Current architecture', foot: 'Each native task has a private address space. Several trusted services still share init.', notes: 'Walk down the diagram. Linux is the development or cloud host and runs QEMU. QEMU supplies virtual hardware to the separate MnemosyneOS kernel. The guest console, filesystem, interpreter manager and recovery executor are inside privileged init. Counter, hog and fault services are separate ring-3 tasks. ATA is currently a kernel driver. The optional browser dashboard shows older host experiment suites, and an optional host AI adapter supplies candidate JSON through the operator.', refs: ['kernel', 'status', 'demo'] },
  { type: 'rows', title: 'System design contracts', rows: [
    ['Memory and CPU', 'One x86-64 CPU, private page tables, guarded stacks and integer contexts.'],
    ['Authority', 'Kernel-owned per-task tables, typed rights, generation handles and checked buffers.'],
    ['Storage', 'Dedicated 16-MiB disk, immutable roots and explicit flush ordering.'],
    ['Wasm execution', 'One admitted integer fixture, 64-KiB memory and 10,000 fuel per request.'],
  ], foot: 'The narrow profile makes state, authority and failure behavior observable.', notes: 'These are intentional implementation limits. No FP/SIMD sharing or arbitrary native ELF loader is supported. The fixed Wasm profile excludes threads, tables, floating point and a start function. Filesystem checksums detect corruption; they do not authenticate an adversarial rewrite.', refs: ['kernel', 'fs', 'runtime'] },
  { type: 'scheduler', title: 'Algorithm: scheduling and IPC', foot: '100-Hz round robin keeps the console running while cpu-hog never yields.', notes: 'The timer or a blocking syscall saves the current task context. The scheduler wakes eligible sleepers and selects the next ready task, changes CR3 and restores its frame. IPC first checks the handle and user buffer, then queues a bounded message stamped with the sender identity. A receive can block; a full queue produces an error. The idle path is implemented, but the standard demo keeps cpu-hog runnable.', refs: ['kernel', 'cap', 'evidence'] },
  { type: 'storage', title: 'Algorithm: filesystem commit', foot: 'On ambiguous I/O failure, the mount refuses further operations until remount.', notes: 'ChronicleFS commits a complete immutable root in an unused slot. Payload and header have separate flushes. Mount accepts only structurally valid checksummed roots and chooses the newest generation. A failed final flush can be ambiguous, so remount is required. The current volume has 64 retained generations, no GC, 32 files and a 192-KiB file bound. Guest reboot success is narrower evidence than physical power-loss correctness.', refs: ['fs', 'evidence', 'crash'] },
  { type: 'table', title: 'Two checkpoint paths', heads: ['Property', 'Native counter service', 'Wasm worker'], widths: [260, 446, 446], rowHeight: 78, rows: [
    ['Boundary', 'Blocked IPC receive', 'Completed request handler'],
    ['Saved state', 'Registers and page mappings', 'Linear memory and mutable global'],
    ['Copy mechanism', 'Page protection and copy-on-write', 'Full state copy'],
    ['Replay input', 'Two recorded IPC requests', 'Requests, clock and random values'],
    ['Success check', 'Equal replies; stale handles fail', 'Complete trace; equal state and effects'],
  ], foot: 'Native COW and Wasm snapshots are separate implementations with restricted scopes.', notes: 'Do not describe Wasm recovery as COW. The native path retains a checkpoint and copies modified pages on write. The saved native run reports two page copies. The Wasm path uses a durable recovery bundle and a new interpreter instance. Neither is a universal multi-service snapshot or an external effect delivery protocol.', refs: ['kernel', 'runtime', 'evidence'] },
  { type: 'recovery', title: 'Algorithm: policy-checked recovery', foot: 'A candidate remains unpublished until replay checks and result persistence succeed.', notes: 'The failed interpreter instance is discarded. Parse a bounded plan, verify identity, authority, requested rights, resource budgets and rollback. The private token is consumed only after revalidating execution-time context. Replay uses a fresh private instance and compares every event, state digest and effect. Commit the result before promoting the recovered live worker. Any check or commit failure rejects publication. The flowchart summarizes the implemented restricted Wasm path.', refs: ['policy', 'runtime', 'fs'] },
  { type: 'code', title: 'Typed recovery proposal', code: '{\n  "version": 1,\n  "action": "restore-replay",\n  "service": "kv-service",\n  "epoch": 6,\n  "checkpoint-sha256": "199be84b...",\n  "max-events": 60,\n  "fuel-per-request": 10000,\n  "memory-pages": 1,\n  "requested-rights": 3,\n  "require-rollback": true\n}', sideTitle: 'Checks before execution', side: 'Identity and epoch\nCurrent authority\nNo extra privileges\nEvent, fuel and memory limits\nRollback availability', foot: 'Display example: digest shortened. Use the full plan from plan-show for execution.', notes: 'This is a shortened display of the recorded candidate, not a paste-ready plan. plan-show supplies the full digest and current epoch. The DSL has one action and denies unknown fields. An optional host model may propose this JSON, but cannot execute it. The demo uses an offline deterministic candidate; online model reliability and benefit remain unevaluated.', refs: ['policy', 'evidence'] },
  { type: 'table', title: 'Implemented kernel functionality', heads: ['Module', 'Working behavior', 'Visible evidence'], widths: [285, 545, 322], rowHeight: 77, rows: [
    ['Boot and memory', 'Rust kernel, page tables, heaps and faults', 'Boot checks pass'],
    ['Task scheduling', 'Four ring-3 tasks and timer preemption', 'ps; hog preemptions'],
    ['Capability IPC', 'Typed handles, bounded queues and buffers', 'ipc-demo; cap-demo'],
    ['Fault containment', 'Faulting worker dies; peers continue', 'Three CPU fault probes'],
    ['Native recovery', 'COW restore, endpoint refresh and replay', 'cow-demo passes'],
  ], foot: 'Compiled-in tasks on one CPU; general application admission remains future work.', notes: 'These behaviors now execute in the guest under the custom kernel. Earlier host-only experiments are not being substituted for these results. The negative syscall demo rejects forged or inappropriate authority and invalid user pointers. Native containment targets the separate fault-service, not the trusted init process.', refs: ['kernel', 'cap', 'evidence'] },
  { type: 'table', title: 'Implemented storage and recovery', heads: ['Module', 'Working behavior', 'Current limit'], widths: [270, 535, 347], rowHeight: 92, rows: [
    ['ChronicleFS', 'Files, atomic roots, snapshots and reboot mount', 'Flat names; finite generations'],
    ['Guest Wasmi', 'Fuel-limited integer workload in userspace', 'One admitted fixture'],
    ['Durable replay', 'Saved bundle; fresh replay; resumed requests', 'In-memory effect comparison'],
    ['Recovery policy', 'Typed action and execution-time checks', 'No formal soundness proof'],
  ], foot: 'These modules share the trusted init address space in this prototype.', notes: 'The filesystem, Wasmi embedding and verifier execute inside the OS but are not yet independent service processes. The disk is real virtual block storage and persists across QEMU boots. A Wasm trap discards the failed interpreter; a native fault in trusted init is fatal. Keep these different containment boundaries explicit.', refs: ['fs', 'runtime', 'policy', 'status'] },
  { type: 'evidence', title: 'Executable prototype evidence', foot: 'Recorded native-Linux guest run, 9 October 2026. Source and ISO hash are in the notes.', notes: 'These are excerpts and counts from the saved two-boot acceptance, not a screenshot of a new live run. All six guest stages pass. The next boot reads the persistence marker, rejects a plan requesting excessive rights, replays the durable bundle and accepts a new live request. The integrated increment also passed 40 Rust and 17 Python tests; those counts do not measure implementation completeness. Zero elapsed ticks in one sample means below timer resolution, not zero work.', refs: ['evidence', 'tests'] },
  { type: 'completion', title: 'Implementation progress and Review I', foot: 'A scope decision is still needed before asserting the required 60–70% completion.', notes: 'Be direct: six of six bounded prototype acceptance gates have evidence. That number cannot establish the percentage of the original proposal. The final denominator must be an agreed weighted set of implementation acceptance criteria. Split partial work into concrete deliverables, attach evidence and keep slide preparation out of the numerator. The project is executable; its full-project percentage is currently unquantified.', refs: ['review', 'status'] },
  { type: 'table', title: 'Proposed module division', heads: ['Workstream', 'Owner', 'Modules and review responsibility'], widths: [355, 185, 612], rowHeight: 90, rows: [
    ['Kernel and scheduling', 'Unassigned', 'Boot, memory, syscalls and preemption demonstration'],
    ['Security and native recovery', 'Unassigned', 'Capabilities, IPC, COW and stale-handle rejection'],
    ['Storage and Wasm', 'Unassigned', 'ChronicleFS, durable bundles and reboot recovery'],
    ['Policy and evaluation', 'Unassigned', 'Verifier, AI adapter, fault campaigns and dashboard'],
  ], foot: 'Suggested workstreams. Actual names, ownership and contribution records are pending.', notes: 'This is an editable allocation proposal, not evidence of individual work already performed. Assign real names and agree responsibilities before submission. Combine workstreams if there are fewer than four team members. Every member should understand the architecture and be able to run the core demo. Use actual commits and reviewed work to support individual attribution.', refs: ['review'] },
  { type: 'code', title: 'Live demo: boot and execute the core', code: 'Linux host, inside the demo bundle:\n\npython3 tools/run-image.py --self-test\npython3 tools/run-image.py \\\n  --disk build/review-one.img\n\nMnemosyneOS guest console:\n\nabout\nps\ndemo\nls\nsnapshots', sideTitle: 'What the professor sees', side: 'Own kernel and console\nProtected user tasks\nPreemption and IPC\nContained faults\nFilesystem snapshots\nChecked service recovery', foot: 'Use an existing non-Azure Linux VM. QEMU runs one CPU and 256 MiB without a guest NIC.', notes: 'Run the two-boot self-test before the presentation. In the live guest, narrate each of the six demo stages and require the final passed:true event. The run-image commands need the prebuilt ISO in the extracted bundle; a source checkout can instead run tools/dev.py guest-test --release and run --release. Use a fresh disk filename for a fresh rehearsal because ChronicleFS retains a finite number of generations.', refs: ['demo', 'evidence'] },
  { type: 'code', title: 'Live demo: persistence and recovery', code: 'Guest, after demo:\nwrite professor-note.txt survives-reboot\ncat professor-note.txt\nshutdown\n\nHost, boot the same disk:\npython3 tools/run-image.py \\\n  --disk build/review-one.img\n\nGuest:\ncat professor-note.txt\nrecover\nkv-add 1 2', sideTitle: 'Expected outcome', side: 'The file still contains\nsurvives-reboot\n\nReplay checks pass.\n\nThe fixed KV fixture accepts\na new request and returns 180.', foot: 'The saved bundle is durable. Later live KV changes are volatile in this demonstration.', notes: 'A full QEMU exit and another boot demonstrates persistence beyond a running process. Immediately after recovering the saved fixture, the specified KV request returns 180. Type shutdown when finished; the intentional CPU-hog otherwise continues to run. The self-test also exercises a privilege-amplifying plan rejection. Do not paste a plan from a different disk or epoch.', refs: ['demo', 'evidence'] },
  { type: 'rows', title: 'Remaining implementation and evaluation', rows: [
    ['Service boundaries', 'Separate storage/runtime management from init and broaden task lifecycle coverage.'],
    ['Recovery coverage', 'Admit additional workloads and exercise more state, trace and storage failures.'],
    ['Measured evaluation', 'Collect repeated IPC, capability, Wasm, checkpoint and recovery measurements.'],
    ['Control plane', 'Connect current guest evidence to the dashboard and compare model proposals with a deterministic baseline.'],
  ], foot: 'The current 10-ms QEMU timer cannot support fine-grained performance conclusions.', notes: 'Prioritize measurable acceptance instead of adding a desktop or unrelated features. Use seeded fault cases, raw samples, a stated measurement environment and baselines such as restart-only versus checkpoint/replay. Define an external-effect protocol only if retained in the final agreed scope. A live AI call alone would not show an improvement over a deterministic proposer.', refs: ['review', 'status', 'policy'] },
  { type: 'statement', title: 'Research contribution to evaluate', statement: 'Predictable recovery under\ncurrent authority and bounded execution.', body: 'The integrated prototype is executable.\nThe next evidence must quantify correctness coverage and recovery cost.', aside: 'Review decisions: confirm the final scope, assign team ownership and agree the completion criteria.', notes: 'End the main presentation with what is working and what must be evaluated next. Do not claim a novel protocol or a percentage merely because every module has some code. The following slides are backup for questions and source references.', refs: ['review', 'status'] },
  { type: 'rows', title: 'Review questions', rows: [
    ['Why a custom kernel?', 'Kernel construction is part of the project, and it exposes the authority and recovery mechanisms.'],
    ['Why a browser UI?', 'It is optional host experiment tooling. The guest OS has its own serial console.'],
    ['Does AI run the OS?', 'No. A model may propose a typed plan; trusted code checks and executes an allowed action.'],
    ['Is it formally verified?', 'No. Runtime policy checks and failure tests support the restricted prototype claims.'],
  ], foot: 'Current limits include no SMP, guest networking, desktop, arbitrary loader or external exactly-once delivery.', notes: 'If asked about microkernel purity, explain that ATA is still in the kernel and several userspace services are co-located in init. If asked why not seL4, say a seL4-based recovery study is a credible alternative, but the agreed project includes kernel construction. The OS does not need a GUI to demonstrate its kernel, filesystem or scheduler.', refs: ['status', 'review', 'sel4'] },
  { type: 'references', title: 'References and reproducibility', items: [
    ['seL4', 'Klein et al., SOSP 2009'],
    ['Theseus', 'Boos et al., OSDI 2020'],
    ['RIFL', 'Lee et al., SOSP 2015'],
    ['CrashMonkey / B3', 'Mohan et al., OSDI 2018'],
    ['LLM-Modulo', 'Gundawar et al., arXiv 2024'],
    ['Project evidence', 'Source, recorded logs, build pins and cloud runbook'],
  ], foot: 'Full paper links, source paths and evidence identities appear in slide notes and the review guide.', notes: 'Primary paper links are provided below. Extended background is in docs/deep-research.md and pinned repository source pointers are in docs/source-reading-guide.md. Current executable evidence is under docs/evidence, separate from the older presentation/evidence baseline. The private GitHub repository is https://github.com/divyamagrawal06/mnemosyne-os and may require access for the professor.', refs: ['sel4', 'theseus', 'rifl', 'crash', 'llm', 'reuse', 'evidence', 'demo'] },
];

const presentation = Presentation.create({ slideSize: { width: 1280, height: 720 } });
const nativeTables = [];
const allNotes = [];
const W = 1152;
function text(slide, value, x, y, width, height, size = 28, color = C.ink, bold = false, family = 'Arial') {
  const box = slide.shapes.add({ geometry: 'textbox', name: `text-${slide.shapes.items.length + 1}`, position: { left: x, top: y, width, height }, fill: 'none', line: { fill: 'none', width: 0 } });
  box.text = value;
  box.text.style = { typeface: family, fontSize: size, bold, color, verticalAlignment: 'top', wrap: 'square', insets: 0 };
  return box;
}
// Native shapes are used only for the requested architecture/algorithm diagrams.
function node(slide, label, x, y, width, height, options = {}) {
  const box = slide.shapes.add({ geometry: options.geometry || 'rect', name: `diagram-${slide.shapes.items.length + 1}`, position: { left: x, top: y, width, height }, fill: options.fill || C.white, line: { fill: options.line || C.teal, width: 1.5 } });
  box.text = label;
  box.text.style = { typeface: 'Arial', fontSize: options.size || 25, color: options.color || C.ink, bold: options.bold || false, alignment: 'center', verticalAlignment: 'middle', wrap: 'square', insets: 10 };
  return box;
}
function arrow(slide, x, y, width, height, direction = 'down', color = C.teal) {
  slide.shapes.add({ geometry: direction === 'right' ? 'rightArrow' : 'downArrow', name: `flow-${slide.shapes.items.length + 1}`, position: { left: x, top: y, width, height }, fill: color, line: { fill: 'none', width: 0 } });
}
function table(slide, spec, index) {
  const values = [spec.heads, ...spec.rows];
  const t = slide.tables.add({ rows: values.length, columns: values[0].length, left: 64, top: 177, width: W, height: 48 + spec.rows.length * spec.rowHeight, columnWidths: spec.widths, values });
  t.styleOptions = { headerRow: true, bandedRows: false };
  t.borders.assign({ fill: C.line, width: 0.65, style: 'solid' });
  t.cells.block({ row: 0, column: 0, rowCount: values.length, columnCount: values[0].length }).assign({ margins: { left: 13, right: 13, top: 7, bottom: 7 }, anchor: 'center', textStyle: { typeface: 'Arial', fontSize: 24, color: C.ink } });
  for (let r = 0; r < values.length; r++) {
    t.rows[r].height = r ? spec.rowHeight : 48;
    for (let c = 0; c < values[0].length; c++) {
      const cell = t.getCell(r, c);
      cell.fill = r === 0 ? C.ink : r % 2 ? C.paper : C.pale;
      cell.text.style = { typeface: 'Arial', fontSize: r === 0 ? 23 : 24, bold: r === 0 || c === 0, color: r === 0 ? C.white : C.ink };
    }
  }
  nativeTables.push(index + 1);
}

for (const [i, spec] of slides.entries()) {
  const slide = presentation.slides.add();
  const dark = spec.type === 'cover' || spec.type === 'evidence';
  slide.background.fill = dark ? C.ink : C.paper;
  const fg = dark ? C.white : C.ink;
  if (spec.type === 'cover') {
    const bytes = await fs.readFile(path.join(ROOT, 'presentation/assets/cover-memory.png'));
    slide.images.add({ blob: bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength), contentType: 'image/png', alt: 'Concept illustration of layered memory over a silicon core', position: { left: 0, top: 0, width: 1280, height: 720 }, fit: 'cover' });
    text(slide, spec.title, 64, 184, 650, 104, 76, C.white, true);
    text(slide, spec.subtitle, 68, 310, 625, 160, 33, C.white);
    text(slide, spec.note, 68, 566, 650, 86, 24, C.mint);
  } else {
    text(slide, spec.title, 64, 54, W, 105, 44, fg, true);
    if (spec.type === 'statement') {
      text(slide, spec.statement, 64, 188, W, 206, 43, C.teal, true);
      text(slide, spec.body, 64, 421, W, 112, 29);
      text(slide, spec.aside, 64, 564, W, 70, 25, C.muted);
    } else if (spec.type === 'rows') {
      spec.rows.forEach(([label, body], r) => {
        text(slide, label, 64, 191 + r * 108, 280, 83, 28, C.teal, true);
        text(slide, body, 370, 191 + r * 108, 828, 88, 28);
      });
    } else if (spec.type === 'table') table(slide, spec, i);
    else if (spec.type === 'code') {
      text(slide, spec.code, 64, 178, 798, 465, 24, C.teal, false, 'Consolas');
      text(slide, spec.sideTitle, 898, 182, 315, 82, 28, C.ink, true);
      text(slide, spec.side, 898, 285, 315, 345, 25, C.muted);
    } else if (spec.type === 'architecture') {
      node(slide, 'Linux host: terminal, QEMU and optional experiment tools', 64, 169, W, 56, { fill: C.pale, size: 26 });
      arrow(slide, 625, 230, 24, 26);
      text(slide, 'MnemosyneOS guest / ring 3', 64, 263, W, 34, 24, C.teal, true);
      node(slide, 'Trusted init-console\n\nShell, ChronicleFS, Wasmi\nand recovery verifier/executor', 64, 309, 574, 186, { fill: C.pale, size: 27 });
      node(slide, 'counter-service / native IPC and COW', 664, 309, 552, 57, { size: 23 });
      node(slide, 'cpu-hog\nPreemption probe', 664, 374, 266, 121, { size: 24 });
      node(slide, 'fault-service\nContainment probe', 948, 374, 268, 121, { size: 24 });
      arrow(slide, 327, 501, 24, 25);
      arrow(slide, 928, 501, 24, 25);
      node(slide, 'Ring 0: page tables, scheduler, capabilities, IPC, serial and ATA', 64, 533, W, 57, { fill: C.ink, color: C.white, size: 25 });
      arrow(slide, 625, 596, 24, 19);
      node(slide, 'QEMU hardware and persistent virtual disk', 278, 619, 724, 44, { size: 22 });
    } else if (spec.type === 'scheduler') {
      text(slide, 'Timer or blocking syscall', 64, 176, 490, 41, 28, C.teal, true);
      const labels = ['Save the current task context', 'Wake sleepers; select a ready task', 'Switch page table and restore context'];
      labels.forEach((label, r) => {
        node(slide, label, 64, 238 + r * 121, 510, 78, { size: 26 });
        if (r < 2) arrow(slide, 307, 324 + r * 121, 24, 28);
      });
      text(slide, 'Capability IPC', 665, 176, 500, 41, 28, C.teal, true);
      ['Resolve caller authority and buffer', 'Queue a bounded, sender-stamped message', 'Wake receiver or block until input'].forEach((label, r) => {
        node(slide, label, 664, 238 + r * 121, 552, 78, { size: 26 });
        if (r < 2) arrow(slide, 928, 324 + r * 121, 24, 28);
      });
      text(slide, 'No voluntary yield is required.', 64, 591, 515, 45, 25, C.muted);
      text(slide, 'Invalid rights or full queues return errors.', 664, 591, 552, 65, 25, C.muted);
    } else if (spec.type === 'storage') {
      const labels = ['Build new root\nin an unused slot', 'Write payload\nand flush', 'Write commit header\nand flush', 'Publish committed\ngeneration'];
      labels.forEach((label, j) => {
        node(slide, label, 64 + j * 299, 212, 255, 133, { size: 26, fill: j === 3 ? C.mint : C.white });
        if (j < 3) arrow(slide, 328 + j * 299, 267, 26, 23, 'right');
      });
      text(slide, 'Mount after reboot', 64, 407, 345, 47, 30, C.teal, true);
      text(slide, 'Validate headers, payload checksums and structure.\nSelect the newest valid root.', 440, 407, 773, 101, 29);
      text(slide, '64 retained generations on a 16-MiB disk.\nFull volumes refuse new commits; old roots remain intact.', 64, 553, W, 86, 28, C.muted);
    } else if (spec.type === 'recovery') {
      const labels = [
        'Discard failed worker; load bundle and candidate plan',
        'Verify policy; revalidate context before execution',
        'Restore fresh instance and consume the recorded trace',
        'Require complete trace, equal state and equal effects',
        'Commit recovery result before publishing the worker',
        'Promote recovered worker and accept a live request',
      ];
      labels.forEach((label, j) => {
        node(slide, label, 64, 172 + j * 78, 827, 56, { size: 25, fill: j === 5 ? C.mint : C.white });
        if (j < 5) arrow(slide, 466, 231 + j * 78, 22, 17);
      });
      node(slide, 'Reject proposal\n\nKeep the recovery\nunpublished', 962, 251, 254, 296, { size: 25, line: C.copper });
      [1, 3, 4].forEach(j => arrow(slide, 903, 192 + j * 78, 40, 20, 'right', C.copper));
      text(slide, 'Any failed check', 942, 197, 274, 46, 22, C.copper, true);
    } else if (spec.type === 'evidence') {
      [['6 / 6', 'guest demo stages'], ['12 / 60', 'requests / recorded events'], ['2 boots', 'persistence and recovery']].forEach(([n, label], j) => {
        text(slide, n, 64 + j * 392, 186, 370, 86, 60, C.mint, true);
        text(slide, label, 64 + j * 392, 286, 370, 60, 26, C.white);
      });
      text(slide, 'guest-demo: passed\nstate-equal: true     effects-equal: true\nunsafe plan: PrivilegeAmplification\nrecovered live request: value=180', 64, 390, W, 205, 31, C.white, false, 'Consolas');
    } else if (spec.type === 'completion') {
      text(slide, '6 / 6', 64, 190, 371, 129, 88, C.teal, true);
      text(slide, 'bounded prototype\nacceptance gates evidenced', 64, 341, 470, 106, 30);
      text(slide, 'Full-project completion\nis not yet quantified.', 627, 190, 588, 134, 39, C.ink, true);
      text(slide, 'Agree final criteria and weights.\nAttach evidence to completed criteria.\nKeep unfinished evaluation explicit.', 627, 367, 588, 188, 28, C.muted);
      text(slide, 'Demo coverage and implementation effort measure different things.', 64, 592, W, 54, 28, C.teal);
    } else if (spec.type === 'references') {
      spec.items.forEach(([name, description], r) => {
        text(slide, name, 64, 178 + r * 74, 380, 57, 28, C.teal, true);
        text(slide, description, 450, 178 + r * 74, 760, 57, 27);
      });
    }
    text(slide, `${String(i + 1).padStart(2, '0')} / ${slides.length}     ${spec.foot || ''}`, 64, 681, W, 25, 16, dark ? C.mint : C.muted);
  }
  const refs = (spec.refs || []).map(k => SOURCES[k]);
  if (spec.type === 'cover') refs.push('Concept artwork generated previously with image_gen. Source asset: presentation/assets/cover-memory.png. Full prompt provenance: presentation/assets/provenance.json. This is not a photograph or program output.');
  slide.speakerNotes.text = spec.notes + '\n\nSources and evidence\n' + refs.join('\n\n');
  allNotes.push(`## ${i + 1}. ${spec.title}\n\n${spec.notes}\n\n${refs.map(r => '- ' + r).join('\n')}\n`);
}

await fs.writeFile(path.join(TMP, 'presentation.json'), JSON.stringify(presentation.toProto()));
await fs.writeFile(path.join(TMP, 'slide-outline.json'), JSON.stringify(slides.map((s, i) => ({ slide: i + 1, title: s.title, type: s.type, refs: s.refs })), null, 2));
const notesName = outputName.replace(/\.pptx$/, '-speaker-notes.md');
await fs.writeFile(path.join(TMP, notesName), '# MnemosyneOS Review I speaker notes\n\nSlides 1–22 are the main presentation; 23–24 are backup. Team names and the final completion denominator remain open. The full demo route is in docs/review-one-guide.md.\n\n' + allNotes.join('\n'));
const candidatePath = path.join(TMP, 'candidate.pptx');
await (await PresentationFile.exportPptx(presentation)).save(candidatePath);
for (const [i, slide] of presentation.slides.items.entries()) {
  const blob = await presentation.export({ slide, format: 'png', scale: 1 });
  await fs.writeFile(path.join(TMP, `slide-${String(i + 1).padStart(2, '0')}.png`), new Uint8Array(await blob.arrayBuffer()));
  console.log(`Rendered ${i + 1}/${slides.length}`);
}
const result = await finalizePresentation({
  workspaceDir: ROOT, candidatePath, finalPath: path.join(OUT, outputName),
  pythonExecutable: args.python,
  integrityValidatorPath: path.join(args['skill-dir'], 'container_tools/inspect_presentation_package_integrity.py'),
  layoutValidatorPath: path.join(args['skill-dir'], 'container_tools/inspect_presentation_layout_geometry.py'),
  layoutArgs: ['--expected-slide-size-emu', '12192000,6858000', '--validate-heading-fit', '--validate-bullet-geometry', ...nativeTables.flatMap(n => ['--require-native-table-slide', String(n)])],
  requiredNativeTableOwnerSlides: nativeTables,
  fontPolicy: { basis: 'design', families: ['Arial', 'Consolas'] },
  verifyArtifactToolImport: true,
  receiptPath: path.join(TMP, outputName + '.validation.json'),
});
await fs.copyFile(path.join(TMP, notesName), path.join(OUT, notesName), 1);
console.log(JSON.stringify(result));
