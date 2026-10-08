# Working on MnemosyneOS

- Never use Azure, including inspection, authentication, cleanup, or VM management.
- Heavy compute requires an explicitly approved non-Azure arrangement. Ordinary development builds and short QEMU smoke tests should use modest resources.
- Make small, coherent local commits as working increments are completed. Do not push or publish unless asked.
- Reuse existing libraries and project code where practical. Pin dependencies and record source, revision, license, and local modifications for copied code in THIRD_PARTY.md. Preserve upstream notices.
- Keep the custom kernel boundary explicit. Host experiments do not establish that a feature runs in the kernel.
- Keep shared core crates usable without the Rust standard library where practical.
- Validate the behavior changed. Security and recovery tests must exercise failures, stale identities, and interrupted operations, not just success cases.
- Distinguish planned features, tested implementation, and completed formal proofs in documentation.

