# MnemosyneOS Review I speaker notes

Slides 1–22 are the main presentation; 23–24 are backup. Team names and the final completion denominator remain open. The full demo route is in docs/review-one-guide.md.

## 1. MnemosyneOS

Introduce the research OS and state that the demonstration now boots its own kernel with protected userspace tasks and persistent storage. The original proposal title ends with Verified AI-Assisted Recovery. In this review, verified refers to implemented deterministic policy checks; no formal proof is claimed. The cover is conceptual artwork, not experimental evidence.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.
- Concept artwork generated previously with image_gen. Source asset: presentation/assets/cover-memory.png. Full prompt provenance: presentation/assets/provenance.json. This is not a photograph or program output.

## 2. Problem statement

Use a counter or key-value service as the example. Restoration must account for both state and authority. The current prototype reconstructs a restricted worker and compares its effects in memory. It does not implement an external recipient protocol for exactly-once effects.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf

## 3. Objectives

The kernel is the experimental environment for recovery. Explain acceptance as observable behavior rather than a feature list: a CPU-hog is preempted, a sibling survives a fault, a file survives reboot, stale handles fail, and a recovered worker accepts a new request.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.

## 4. Background: kernel structure and state

seL4 is a reference for carefully defined kernel authority and verification obligations. Theseus explores a different Rust OS structure with strong component state boundaries. Our implementation does not reuse their proofs, structures wholesale, or reported results.

- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Boos, Liyanage, Ijaz and Zhong. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.

## 5. Background: recovery and verification

RIFL explains why replay equality by itself cannot solve distributed duplicate effects. CrashMonkey motivates bounded crash testing, though its tool is not ported here. LLM-Modulo motivates separating generation from checking. Soundness of the checker and correctness of execution still require their own argument.

- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
- Mohan, Martinez, Ponnapalli, Raju and Chidambaram. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan
- Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484v1, 2024. https://arxiv.org/html/2411.14484v1

## 6. Reuse and project contribution

Distinguish using a library from claiming to have written an interpreter or bootloader. The project contribution is the integrated OS and its constrained recovery protocol. THIRD_PARTY.md and Cargo.lock record the reuse boundary and exact resolved dependencies.

- Repository: THIRD_PARTY.md, Cargo.lock and third-party/licenses. Limine bootloader v10.8.5-binary commit 7a9013f305de0bea1f6310e76e7baba30499fef0; Rust OSDev x86_64 0.15.4; linked_list_allocator 0.10.6; Wasmi 2.0.0; wat 1.228.0. https://github.com/limine-bootloader/limine https://github.com/rust-osdev/x86_64 https://github.com/wasmi-labs/wasmi

## 7. Current architecture

Walk down the diagram. Linux is the development or cloud host and runs QEMU. QEMU supplies virtual hardware to the separate MnemosyneOS kernel. The guest console, filesystem, interpreter manager and recovery executor are inside privileged init. Counter, hog and fault services are separate ring-3 tasks. ATA is currently a kernel driver. The optional browser dashboard shows older host experiment suites, and an optional host AI adapter supplies candidate JSON through the operator.

- Repository: kernel/src/main.rs, arch.rs, memory.rs, paging.rs, tasks.rs, user-memory.rs and task-entry.S. The user ELF is compiled into the kernel; no arbitrary application loader is claimed.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.
- Repository: docs/cloud-demo.md, tools/run-image.py and tools/dev.py. The prebuilt bundle requires Python 3 and QEMU on a non-Azure x86-64 Linux host. Run-image uses a 256-MiB, one-CPU guest without a NIC. The self-test uses fresh disks and two bounded boots.

## 8. System design contracts

These are intentional implementation limits. No FP/SIMD sharing or arbitrary native ELF loader is supported. The fixed Wasm profile excludes threads, tables, floating point and a start function. Filesystem checksums detect corruption; they do not authenticate an adversarial rewrite.

- Repository: kernel/src/main.rs, arch.rs, memory.rs, paging.rs, tasks.rs, user-memory.rs and task-entry.S. The user ELF is compiled into the kernel; no arbitrary application loader is claimed.
- Repository: crates/chronicle-fs/src/lib.rs, kernel/src/ata.rs and userland/init/src/shell.rs. Eight shared filesystem tests. ATA is in the kernel; filesystem policy runs in trusted userspace init. Flush ordering is an assumption.
- Repository: crates/service-runtime/src/lib.rs and tests.rs, workloads/kv/service.wat, userland/init/src/shell.rs. Eighteen runtime tests. The fixed profile has full-copy Wasm snapshots, synthetic clock/random inputs and in-memory effects.

## 9. Algorithm: scheduling and IPC

The timer or a blocking syscall saves the current task context. The scheduler wakes eligible sleepers and selects the next ready task, changes CR3 and restores its frame. IPC first checks the handle and user buffer, then queues a bounded message stamped with the sender identity. A receive can block; a full queue produces an error. The idle path is implemented, but the standard demo keeps cpu-hog runnable.

- Repository: kernel/src/main.rs, arch.rs, memory.rs, paging.rs, tasks.rs, user-memory.rs and task-entry.S. The user ELF is compiled into the kernel; no arbitrary application loader is claimed.
- Repository: crates/capability/src/lib.rs and kernel/src/tasks.rs. Eight capability tests, typed handles, generation checks, attenuation/subtree revocation in the table, checked syscalls and endpoint invalidation on restore. No cross-process delegation syscall.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.

## 10. Algorithm: filesystem commit

ChronicleFS commits a complete immutable root in an unused slot. Payload and header have separate flushes. Mount accepts only structurally valid checksummed roots and chooses the newest generation. A failed final flush can be ambiguous, so remount is required. The current volume has 64 retained generations, no GC, 32 files and a 192-KiB file bound. Guest reboot success is narrower evidence than physical power-loss correctness.

- Repository: crates/chronicle-fs/src/lib.rs, kernel/src/ata.rs and userland/init/src/shell.rs. Eight shared filesystem tests. ATA is in the kernel; filesystem policy runs in trusted userspace init. Flush ordering is an assumption.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.
- Mohan, Martinez, Ponnapalli, Raju and Chidambaram. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan

## 11. Two checkpoint paths

Do not describe Wasm recovery as COW. The native path retains a checkpoint and copies modified pages on write. The saved native run reports two page copies. The Wasm path uses a durable recovery bundle and a new interpreter instance. Neither is a universal multi-service snapshot or an external effect delivery protocol.

- Repository: kernel/src/main.rs, arch.rs, memory.rs, paging.rs, tasks.rs, user-memory.rs and task-entry.S. The user ELF is compiled into the kernel; no arbitrary application loader is claimed.
- Repository: crates/service-runtime/src/lib.rs and tests.rs, workloads/kv/service.wat, userland/init/src/shell.rs. Eighteen runtime tests. The fixed profile has full-copy Wasm snapshots, synthetic clock/random inputs and in-memory effects.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.

## 12. Algorithm: policy-checked recovery

The failed interpreter instance is discarded. Parse a bounded plan, verify identity, authority, requested rights, resource budgets and rollback. The private token is consumed only after revalidating execution-time context. Replay uses a fresh private instance and compares every event, state digest and effect. Commit the result before promoting the recovered live worker. Any check or commit failure rejects publication. The flowchart summarizes the implemented restricted Wasm path.

- Repository: crates/recovery-policy/src/lib.rs, userland/init/src/shell.rs, tools/ai-plan.py and tools/test-ai-plan.py. Six policy and four adapter tests. The optional online model path has not been evaluated by the offline acceptance run.
- Repository: crates/service-runtime/src/lib.rs and tests.rs, workloads/kv/service.wat, userland/init/src/shell.rs. Eighteen runtime tests. The fixed profile has full-copy Wasm snapshots, synthetic clock/random inputs and in-memory effects.
- Repository: crates/chronicle-fs/src/lib.rs, kernel/src/ata.rs and userland/init/src/shell.rs. Eight shared filesystem tests. ATA is in the kernel; filesystem policy runs in trusted userspace init. Flush ordering is an assumption.

## 13. Typed recovery proposal

This is a shortened display of the recorded candidate, not a paste-ready plan. plan-show supplies the full digest and current epoch. The DSL has one action and denies unknown fields. An optional host model may propose this JSON, but cannot execute it. The demo uses an offline deterministic candidate; online model reliability and benefit remain unevaluated.

- Repository: crates/recovery-policy/src/lib.rs, userland/init/src/shell.rs, tools/ai-plan.py and tools/test-ai-plan.py. Six policy and four adapter tests. The optional online model path has not been evaluated by the offline acceptance run.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.

## 14. Implemented kernel functionality

These behaviors now execute in the guest under the custom kernel. Earlier host-only experiments are not being substituted for these results. The negative syscall demo rejects forged or inappropriate authority and invalid user pointers. Native containment targets the separate fault-service, not the trusted init process.

- Repository: kernel/src/main.rs, arch.rs, memory.rs, paging.rs, tasks.rs, user-memory.rs and task-entry.S. The user ELF is compiled into the kernel; no arbitrary application loader is claimed.
- Repository: crates/capability/src/lib.rs and kernel/src/tasks.rs. Eight capability tests, typed handles, generation checks, attenuation/subtree revocation in the table, checked syscalls and endpoint invalidation on restore. No cross-process delegation syscall.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.

## 15. Implemented storage and recovery

The filesystem, Wasmi embedding and verifier execute inside the OS but are not yet independent service processes. The disk is real virtual block storage and persists across QEMU boots. A Wasm trap discards the failed interpreter; a native fault in trusted init is fatal. Keep these different containment boundaries explicit.

- Repository: crates/chronicle-fs/src/lib.rs, kernel/src/ata.rs and userland/init/src/shell.rs. Eight shared filesystem tests. ATA is in the kernel; filesystem policy runs in trusted userspace init. Flush ordering is an assumption.
- Repository: crates/service-runtime/src/lib.rs and tests.rs, workloads/kv/service.wat, userland/init/src/shell.rs. Eighteen runtime tests. The fixed profile has full-copy Wasm snapshots, synthetic clock/random inputs and in-memory effects.
- Repository: crates/recovery-policy/src/lib.rs, userland/init/src/shell.rs, tools/ai-plan.py and tools/test-ai-plan.py. Six policy and four adapter tests. The optional online model path has not been evaluated by the offline acceptance run.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.

## 16. Executable prototype evidence

These are excerpts and counts from the saved two-boot acceptance, not a screenshot of a new live run. All six guest stages pass. The next boot reads the persistence marker, rejects a plan requesting excessive rights, replays the durable bundle and accepts a new live request. The integrated increment also passed 40 Rust and 17 Python tests; those counts do not measure implementation completeness. Zero elapsed ticks in one sample means below timer resolution, not zero work.

- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.
- Validation completed for the integrated OS increment: 40 Rust unit tests (8 capability, 8 filesystem, 6 policy, 18 runtime); 17 Python tests (13 dashboard, 4 AI adapter); debug/release kernel boots and two-boot guest acceptance. The presentation update does not rerun all software checks. Reproduce via tools/dev.py test, check, boot and guest-test; Python test files run directly.

## 17. Implementation progress and Review I

Be direct: six of six bounded prototype acceptance gates have evidence. That number cannot establish the percentage of the original proposal. The final denominator must be an agreed weighted set of implementation acceptance criteria. Split partial work into concrete deliverables, attach evidence and keep slide preparation out of the numerator. The project is executable; its full-project percentage is currently unquantified.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.

## 18. Proposed module division

This is an editable allocation proposal, not evidence of individual work already performed. Assign real names and agree responsibilities before submission. Combine workstreams if there are fewer than four team members. Every member should understand the architecture and be able to run the core demo. Use actual commits and reviewed work to support individual attribution.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.

## 19. Live demo: boot and execute the core

Run the two-boot self-test before the presentation. In the live guest, narrate each of the six demo stages and require the final passed:true event. The run-image commands need the prebuilt ISO in the extracted bundle; a source checkout can instead run tools/dev.py guest-test --release and run --release. Use a fresh disk filename for a fresh rehearsal because ChronicleFS retains a finite number of generations.

- Repository: docs/cloud-demo.md, tools/run-image.py and tools/dev.py. The prebuilt bundle requires Python 3 and QEMU on a non-Azure x86-64 Linux host. Run-image uses a 256-MiB, one-CPU guest without a NIC. The self-test uses fresh disks and two bounded boots.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.

## 20. Live demo: persistence and recovery

A full QEMU exit and another boot demonstrates persistence beyond a running process. Immediately after recovering the saved fixture, the specified KV request returns 180. Type shutdown when finished; the intentional CPU-hog otherwise continues to run. The self-test also exercises a privilege-amplifying plan rejection. Do not paste a plan from a different disk or epoch.

- Repository: docs/cloud-demo.md, tools/run-image.py and tools/dev.py. The prebuilt bundle requires Python 3 and QEMU on a non-Azure x86-64 Linux host. Run-image uses a 256-MiB, one-CPU guest without a NIC. The self-test uses fresh disks and two bounded boots.
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.

## 21. Remaining implementation and evaluation

Prioritize measurable acceptance instead of adding a desktop or unrelated features. Use seeded fault cases, raw samples, a stated measurement environment and baselines such as restart-only versus checkpoint/replay. Define an external-effect protocol only if retained in the final agreed scope. A live AI call alone would not show an improvement over a deterministic proposer.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.
- Repository: crates/recovery-policy/src/lib.rs, userland/init/src/shell.rs, tools/ai-plan.py and tools/test-ai-plan.py. Six policy and four adapter tests. The optional online model path has not been evaluated by the offline acceptance run.

## 22. Research contribution to evaluate

End the main presentation with what is working and what must be evaluated next. Do not claim a novel protocol or a percentage merely because every module has some code. The following slides are backup for questions and source references.

- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.

## 23. Review questions

If asked about microkernel purity, explain that ATA is still in the kernel and several userspace services are co-located in init. If asked why not seL4, say a seL4-based recovery study is a credible alternative, but the agreed project includes kernel construction. The OS does not need a GUI to demonstrate its kernel, filesystem or scheduler.

- Repository: docs/implementation-status.md, docs/build-scope.md and docs/professor-brief.md. This is a single-core research prototype, not a formally verified kernel or a complete general-purpose OS.
- Review I requirements supplied by the project author. Rubric, algorithms, ownership placeholders and completion method: docs/review-one-guide.md. Implementation baseline f09a43143e36db9d0b38b7fc7d69b6cb51d37059, 9 October 2026.
- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf

## 24. References and reproducibility

Primary paper links are provided below. Extended background is in docs/deep-research.md and pinned repository source pointers are in docs/source-reading-guide.md. Current executable evidence is under docs/evidence, separate from the older presentation/evidence baseline. The private GitHub repository is https://github.com/divyamagrawal06/mnemosyne-os and may require access for the professor.

- Klein et al. seL4: Formal Verification of an OS Kernel. SOSP 2009. https://sel4.systems/Research/pdfs/sel4-formal-verification-os-kernel.pdf
- Boos, Liyanage, Ijaz and Zhong. Theseus: an Experiment in Operating System Structure and State Management. OSDI 2020. https://www.usenix.org/conference/osdi20/presentation/boos
- Lee et al. Implementing Linearizability at Large Scale and Low Latency. SOSP 2015. https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
- Mohan, Martinez, Ponnapalli, Raju and Chidambaram. Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing. OSDI 2018. https://www.usenix.org/conference/osdi18/presentation/mohan
- Gundawar, Valmeekam, Verma and Kambhampati. Robust Planning with Compound LLM Architectures: An LLM-Modulo Approach. arXiv:2411.14484v1, 2024. https://arxiv.org/html/2411.14484v1
- Repository: THIRD_PARTY.md, Cargo.lock and third-party/licenses. Limine bootloader v10.8.5-binary commit 7a9013f305de0bea1f6310e76e7baba30499fef0; Rust OSDev x86_64 0.15.4; linked_list_allocator 0.10.6; Wasmi 2.0.0; wat 1.228.0. https://github.com/limine-bootloader/limine https://github.com/rust-osdev/x86_64 https://github.com/wasmi-labs/wasmi
- Recorded guest evidence: docs/evidence/guest-demo.log and guest-reboot.log, capture 2026-10-09T07:18:25.765818+00:00, source 1bc81f15606e7ce8465111ad7858d49d1f20455e, ISO SHA-256 849a71cce8b08ce174fc130d943d7c171b078d6ea6b7c14fcf6136e59967fbe5. The later f09a431 change concerns packaging notices/docs, and its release kernel including embedded userspace was compared byte-for-byte with the Linux-tested binary. These are saved results, not a new live run.
- Repository: docs/cloud-demo.md, tools/run-image.py and tools/dev.py. The prebuilt bundle requires Python 3 and QEMU on a non-Azure x86-64 Linux host. Run-image uses a 256-MiB, one-CPU guest without a NIC. The self-test uses fresh disks and two bounded boots.
