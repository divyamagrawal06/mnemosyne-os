# MnemosyneOS speaker notes

Slides 1-22 are the main presentation. Slides 23-30 are technical backup. See the demo guide for the exact terminal walkthrough.

## 1. MnemosyneOS

Introduce MnemosyneOS as a research operating-system project. Its intended contribution joins capability authority, repeatable service recovery, and a deterministic check on AI-proposed actions. Today there are two working foundations: an x86-64 Rust kernel and a separate host Wasmi replay experiment. The full recovery system remains a proposed implementation target. The title artwork is a conceptual illustration, not a hardware photograph or program output.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Concept artwork generated with the built-in image_gen tool. Prompt and local asset provenance: presentation/assets/provenance.json. Artwork is not empirical evidence.

## 2. Research objective

Frame this as a question to investigate, not as a solved claim. Correct restoration has several obligations: reconstruct state, preserve current permissions, respect resource limits, and account for external effects. An AI proposer is useful only if a trusted mechanism can reject unsafe suggestions. A system that merely restarts a service or reproduces a log would not establish this full contribution.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf

## 3. Why service recovery is difficult

Use the example of a key-value service that commits an increment and then crashes before replying. A client retry must have a defined outcome. Separately, a checkpoint may contain a capability that an administrator has since revoked. Restoring the old bytes cannot authorize reuse of that capability. These are protocol questions rather than just memory-copying questions.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
- Boos et al. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos

## 4. Scope and implementation status

Read this slide as the status map for the whole proposal. The kernel already boots and handles memory/protection tests. The capability table runs both in host tests and in a kernel smoke check. Wasm and replay run only on the host. There are no ring-3 services, ChronicleFS implementation, AI model calls, recovery DSL compiler, or benchmark dashboard in this baseline.

- Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline 44b739ddf513031f0b81fee402e484c156ab41a6
- Repository: crates/capability/src/lib.rs and its eight unit tests. Recorded output: presentation/evidence/tests.log.
- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00
- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.

## 5. Proposed system architecture

Keep scheduling and capability enforcement in the kernel, while services own storage and recovery policy. The AI proposer is outside the trusted decision path. The deterministic verifier and executor need a narrow, reviewable interface and must check the current state when committing an action. The host harness observes and injects faults for evaluation. This table describes the intended architecture, not current process deployment.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf

## 6. Capability table semantics

Explain that a handle is local to the caller's kernel-owned table. Knowing an integer from another process must not itself grant authority. Root minting is trusted, and a generation number prevents stale handles from validating after a slot is reused. Current subtree revocation scans a bounded table synchronously in O(N squared). Before recycling an object ID, trusted object teardown must invalidate references in every relevant table. These are prototype contracts, not a system-wide proof.

- Repository: crates/capability/src/lib.rs and its eight unit tests. Recorded output: presentation/evidence/tests.log.

## 7. Processes, preemption and IPC

The first real isolation demo should use two ring-3 services and an unrelated healthy service. Demonstrate unauthorized memory access, invalid endpoint handles, oversized messages, full queues, and a dead receiver. Keep the first version single-core to make scheduling and replay assumptions explicit. Shared writable memory, DMA, SMP, and general device recovery would broaden the trusted-state problem substantially.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline 44b739ddf513031f0b81fee402e484c156ab41a6

## 8. The current Wasm execution profile

Reusing Wasmi gives us upstream validation, interpretation, resource limits, and fuel metering. The public snapshot API accepts only the bundled fixture so that hidden mutable state cannot silently escape capture. The extra eight bytes for the exported global make the mutable-state payload 65,544 bytes. Clock and random inputs are synthetic controlled providers. The effect import does not send anything over a network or to a persistent filesystem.

- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00
- Repository: THIRD_PARTY.md and Cargo.lock. Upstream: https://github.com/limine-bootloader/limine and https://github.com/rust-osdev/x86_64 and https://github.com/wasmi-labs/wasmi. Copied template notice: third_party/licenses/limine-rust-template-0BSD.txt.

## 9. Checkpoint and deterministic replay

The checkpoint contains both memory and the mutable version counter. It binds the module hash, runtime profile, schema, and state digest. Replay restores a fresh instance whose live providers have different seeds. The tests confirm that those providers are never consulted during replay. Snapshots do not capture an active interpreter stack or suspended host call. The illustration expresses the concept and is not a trace or measurement.

- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00
- Concept artwork generated with the built-in image_gen tool. Prompt and local asset provenance: presentation/assets/provenance.json. Artwork is not empirical evidence.

## 10. A strict trace contract

Each successful request contributes five events. A trap or mismatch leaves partially mutated guest state unusable. The worker refuses further execution, new snapshots, and a valid recording until a fresh instance is restored. A successful handler appends its effects only to an in-memory observation list. The test suite removes every event position in a recorded segment and requires rejection. This is validation against a trace, not authentication of the trace.

- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00

## 11. ChronicleFS and durable recovery

Start with a minimal append-only storage design and a narrow filesystem interface. A snapshot pointer is meaningful only when the data it references is durably ordered under a stated block-device model. Reboot testing must check the last committed root rather than relying on a normal shutdown. Research on bounded crash testing informs the test strategy. Garbage collection must also preserve data and completion records still needed by recovery.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Mohan et al. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan

## 12. AI-assisted recovery proposals

The model receives a bounded structured fault trace and proposes a small typed action sequence. The model does not own a raw shell or a privileged kernel API. The example shows the intended vocabulary, not an implemented grammar. Begin with hand-authored plans and adversarial tests. Integrate a model only after verifier/executor behavior is explicit. The LLM-Modulo work motivates the separation of candidate generation and external checking, but it is not a proof of this OS design.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484, 2024. https://arxiv.org/html/2411.14484v1

## 13. Deterministic verification obligations

The authority and resource checks must hold at execution time. A plan that was valid when analyzed can become stale after a capability revocation, resource allocation, or service generation change. Reserve resources and bind execution to the checked state or revalidate atomically. Rollback must not resurrect revoked capabilities or contradict effects already visible to peers. A deterministic rejection path is required when an action cannot meet its preconditions.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf

## 14. Working prototype evidence

There are eight capability tests and sixteen runtime/replay tests. The three CPU probes test NX execution, read-only writes with supervisor write protection, and access after unmapping. They operate in the boot address space. Both build modes complete the smoke path. The replay report compares the complete Snapshot object as well as showing its digest, so the displayed equality is not merely a manually matched string. These results do not establish isolation between processes or durable recovery.

- Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline 44b739ddf513031f0b81fee402e484c156ab41a6
- Repository: crates/capability/src/lib.rs and its eight unit tests. Recorded output: presentation/evidence/tests.log.
- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00

## 15. Live demo through the CLI

Switch to PowerShell in C:\Users\ludic\Desktop\mnemosyne. Run python tools/present.py and press Enter before each step. Explain that the kernel runs in QEMU and the Wasmi lab runs separately on the host. After the final report, return to the deck. For WSL failure use --host-only and say the boot was skipped. For a fully offline recorded fallback use --recorded and identify it as saved evidence. Detailed narration and troubleshooting are in presentation/demo-runbook.md.

- Repository: tools/present.py, tools/dev.py and presentation/demo-runbook.md. Rehearsal evidence and checksums: presentation/evidence/manifest.json.

## 16. Kernel boot evidence

This is an editable transcription of actual recorded output, not a screenshot of a GUI. The live command produces the same class of events. Usable RAM counts can change with build layout and are intentionally omitted here. The runner also checks QEMU's expected exit status. Explicitly distinguish interrupt delivery from process preemption. The general page-fault behavior is still fatal outside the reserved test probes.

- Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline 44b739ddf513031f0b81fee402e484c156ab41a6

## 17. Replay reaches the same fixture state

The two displayed digests come directly from the saved replay JSON. The report additionally compares full snapshot contents and observed effects. Twelve requests each record five events. The checkpoint payload consists of one 64-KiB memory plus one eight-byte mutable global. The replacement instance uses different provider seeds but consumes recorded input values. There are no external effects, durable commits, or runtime upgrades in this test.

- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00

## 18. Reuse and research foundations

Reuse is intentional: the custom work should focus on the experimental security/recovery contracts. Cargo.lock records dependency versions and checksums. Copied Limine template code and boot binaries retain notices. seL4 and other research systems are design references, not claimed bases of our verified implementation. Theseus highlights state ownership, RIFL treats duplicate request completion, and CrashMonkey uses bounded crash workloads. Our candidate contribution is their interaction with current authority in a new recovery protocol, not a claim to invent restart or replay.

- Repository: THIRD_PARTY.md and Cargo.lock. Upstream: https://github.com/limine-bootloader/limine and https://github.com/rust-osdev/x86_64 and https://github.com/wasmi-labs/wasmi. Copied template notice: third_party/licenses/limine-rust-template-0BSD.txt.
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Boos et al. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
- Mohan et al. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan
- Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484, 2024. https://arxiv.org/html/2411.14484v1

## 19. Evaluation plan

Measure p50 and p95/p99 where appropriate, report sample sizes, and keep QEMU correctness runs separate from hardware timing. Include mean recovery time but do not let the mean hide tail latency. Split recovery into detection, restore, replay, validation and publication. For the AI control plane, record valid-plan rate, rejection reasons, latency and success under the same fault cases as a rule-based baseline. Fix workloads and seeds before comparing alternatives. Healthy-service progress and absence of duplicated effects are correctness gates.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.

## 20. Implementation milestones

Acceptance gates make the schedule evidence-based. A working CLI harness comes before a dashboard. Start with one CPU and one restricted workload, then add complexity only when tests establish the preceding contract. The current foundation already covers boot, page protection, table semantics and host replay. Each gate produces a small demonstrable increment and a testable failure case. Avoid promising SMP, broad POSIX compatibility or arbitrary native replay in the first complete version.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.

## 21. Main engineering risks

A microkernel, filesystem, runtime, replay system and AI control plane can each become a large independent project. The controlled workload reduces the initial state space. External effects need a receiver protocol and cannot be fixed by local snapshots alone. Checkpoint hashes do not establish authenticity or semantic correctness. The first full-system claim should be limited to the supported workload, fault model, storage model, and authorized recovery operations.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf

## 22. The project deliverable

Close with the concrete research output rather than a claim that all nine subsystems are already complete. Ask the teacher to review the scope, protocol invariants and staged acceptance criteria. The working foundation makes the implementation route reviewable. A successful final project should show a service failure, recovery of committed state, rejection of an unauthorized plan, no duplicated committed effects under the supported protocol, and continued progress of a healthy service.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline 44b739ddf513031f0b81fee402e484c156ab41a6
- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00

## 23. Technical backup: trust model

Define the adversary instead of saying the system is secure without qualification. For the prototype, trace data and snapshots are generated by the experiment itself. The checksum detects corruption but an attacker who rewrites both data and digest is outside that integrity model. The final protocol must decide where authentication is required. Rust reduces some memory-safety risks, while unsafe hardware access, capability authority, and crash consistency still require separate reasoning.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00

## 24. Technical backup: replay boundaries

The current fixture has no tables, threads, files, sockets, native continuations or suspended host calls. Adding any of these changes the snapshot schema. Restoring one service may conflict with messages already observed by a peer, so service-group recovery needs a consistent history and output publication rules. Exactly-once claims depend on the full receiver and durability protocol. At present the worker does not switch a replay into live execution.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf

## 25. Technical backup: capability edge cases

These failure cases explain why an integer permission flag is insufficient. The test suite includes exhaustive valid rights-subset combinations for memory capabilities. Descendant membership is computed before deleting ancestors so that grandchildren are not missed. Generation exhaustion does not wrap to an earlier identity. The table itself does not establish global object uniqueness or cross-process ownership; those belong to future trusted kernel integration.

- Repository: crates/capability/src/lib.rs and its eight unit tests. Recorded output: presentation/evidence/tests.log.

## 26. Technical backup: demo commands

The guided script runs existing developer commands and records the results under build. It pauses for narration before each step and stops on a nonzero exit. The recorded mode performs no build or VM run and announces its capture date and code baseline. WSL and QEMU are needed only for the kernel portion; native host tests and the Wasmi demo need no VM. Rehearse while connected so Rust dependencies and the pinned bootloader are already cached.

- Repository: tools/present.py, tools/dev.py and presentation/demo-runbook.md. Rehearsal evidence and checksums: presentation/evidence/manifest.json.

## 27. Technical backup: likely review questions

If asked why not simply build on seL4, explain that a seL4-based implementation is a credible alternative when the recovery experiment is the only research goal. A custom kernel is justified by the stated project deliverable and learning objective, but increases effort and assurance work. If asked whether the AI is necessary, say that a rule-based baseline is mandatory and the model must demonstrate measurable benefit. Never substitute confident model output for proof or authority.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484, 2024. https://arxiv.org/html/2411.14484v1

## 28. Technical backup: assurance claims

This slide is important for an academic review. Keep empirical evidence, deterministic policy enforcement and machine-checked proof as distinct categories. The kernel is not seL4-verified merely because its design references seL4. Wasmi validation is an upstream feature, not proof of the full embedding or of recovery correctness. The intended verifier must establish concrete obligations under a stated model, and its executor must preserve them while the system changes.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.
- Repository: kernel/src/main.rs, paging.rs, arch.rs, memory.rs. Recorded evidence: presentation/evidence/boot.log and boot-release.log. Code baseline 44b739ddf513031f0b81fee402e484c156ab41a6
- Repository: host/replay-lab/src/lib.rs, tests.rs, and workloads/kv/service.wat. Recorded report: presentation/evidence/replay-demo.json. Capture: 2026-10-09T05:19:32.155766+00:00
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf

## 29. Formal project proposal

This preserves the author's supplied project title and abstract. Present it as the proposed final system. For progress reporting, use the implementation status slide and source evidence instead. In a formal submission, retain the proposal status or use future tense until these features are implemented and evaluated.

- Project proposal supplied by the author. Repository: docs/research-plan.md, docs/deep-research.md, docs/implementation-status.md. Proposed design, not a completed implementation.

## 30. Selected papers and source repositories

The reading guide contains pinned repository revisions and concrete source pointers. These sources support design decisions and limitations, not a claim that MnemosyneOS inherits another project's proof or benchmark results. Reuse permissions are recorded per component in THIRD_PARTY.md. Local evidence for this deck is preserved under presentation/evidence with its capture time, code baseline and file checksums.

- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Boos et al. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
- Mohan et al. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan
- Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484, 2024. https://arxiv.org/html/2411.14484v1
- Repository: THIRD_PARTY.md and Cargo.lock. Upstream: https://github.com/limine-bootloader/limine and https://github.com/rust-osdev/x86_64 and https://github.com/wasmi-labs/wasmi. Copied template notice: third_party/licenses/limine-rust-template-0BSD.txt.
- Repository: tools/present.py, tools/dev.py and presentation/demo-runbook.md. Rehearsal evidence and checksums: presentation/evidence/manifest.json.
