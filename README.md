# MnemosyneOS

A research Rust microkernel investigating capability isolation, deterministic service replay, and policy-checked recovery.

Development begins with a bootable x86-64 kernel and host-testable core libraries. The research proposal describes the intended system; it is not a claim that all features are implemented.

## Research

- [Architecture and implementation plan](docs/research-plan.md)
- [Deep literature and protocol review](docs/deep-research.md)
- [Pinned repository reading guide](docs/source-reading-guide.md)
- [Research collection notes](research/README.md)

## Development principles

Use existing bootloader, CPU support, Wasm, serialization, and testing libraries where practical. Keep original integration and recovery semantics small and testable. Record third-party attribution and pin dependency versions.

The intended development VM is a short-lived local QEMU instance with one virtual CPU and 256 MiB RAM. It boots a disposable image; no cloud account is required. Full performance campaigns and model training are outside ordinary local development.

Original project code is MIT licensed. Third-party dependencies retain their own licenses.
