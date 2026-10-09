# Bootable research OS scope

The target is a small x86-64 research operating system demonstrated through its
own serial console inside QEMU on a non-Azure Linux VM. The browser dashboard is
optional host tooling. This is not a Linux distribution or a general desktop OS.

Acceptance gates for this integration:

1. Load independent ring-3 ELF tasks into separate page tables. Preempt a task
   that never voluntarily yields, keep the console responsive, and contain CPU
   faults to the offending task.
2. Validate syscall pointers and capability authority. Exchange bounded messages
   between user services. Reject forged/stale handles and unauthorized operations.
3. Mount a dedicated virtual disk, create/read/delete files, retain immutable
   generations, restore a snapshot, and verify persistence across QEMU boots.
   Test interrupted writes and corrupt commits against the shared filesystem code.
4. Run the existing Wasmi workload inside the guest, checkpoint it, record inputs,
   inject a failure, and recover with equal state and effects. Keep runtime limits.
5. Check a typed recovery plan deterministically before execution, including
   authority, target identity, memory/fuel/event limits, and rollback availability.
6. Exercise all of these through an automated guest-console script and publish
   a reproducible cloud-VM runbook, clear limitations, and local test evidence.

New project paths, commands, repository names, and document names use kebab-case.
Language-required Rust/Python identifiers and conventional files such as
Cargo.toml, README.md, and AGENTS.md retain their standard names.

No formal proof, general-purpose driver support, SMP, POSIX compatibility, arbitrary
Wasm checkpointing, or production security claim is implied by these gates.
Cloud infrastructure is user-provided; no provider provisioning is needed to build.
