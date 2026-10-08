# MnemosyneOS

A research Rust microkernel investigating capability isolation, deterministic service replay, and policy-checked recovery.

The x86-64 kernel boots through Limine in QEMU and checks serial output, usable physical frames, capability attenuation/revocation, breakpoint handling, and PIT timer interrupts. The allocation-free capability library also has host tests for stale handles, subtree revocation, rights, capacity, object teardown, and generation exhaustion. This is a tested table implementation; a process/syscall security boundary is still pending.

The research proposal describes the intended system; it is not a claim that all features are implemented. Userspace, process scheduling, IPC, persistent storage, and recovery are subsequent milestones.

## Build and boot

Install Rust/rustup, Python 3, and Git on the build host. The pinned Rust toolchain and `x86_64-unknown-none` target are declared in `rust-toolchain.toml`.

The image tools and QEMU run on Linux, or in the existing Ubuntu WSL distribution on Windows:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends build-essential qemu-system-x86 xorriso
```

From the repository root:

```sh
python tools/dev.py build
python tools/dev.py boot
cargo test --locked
```

`boot` builds the kernel and an ISO, runs a headless 1-vCPU/256-MiB QEMU instance, verifies serial events and the exit status, and terminates the test VM. A timeout fails the test. Output is retained in `build/boot.log`; the image is `build/mnemosyne.iso`. Windows builds use native Cargo and WSL only for image construction and emulation. Set `MNEMOSYNE_WSL_DISTRO` to select another configured distribution.

The runner downloads a pinned Limine binary distribution into `.tools/limine` on first use. See [third-party attribution](THIRD_PARTY.md). BIOS boot is tested; UEFI files are included but UEFI boot is not yet tested.

## Research

- [Architecture and implementation plan](docs/research-plan.md)
- [Deep literature and protocol review](docs/deep-research.md)
- [Pinned repository reading guide](docs/source-reading-guide.md)
- [Research collection notes](research/README.md)

## Development principles

Use existing bootloader, CPU support, Wasm, serialization, and testing libraries where practical. Keep original integration and recovery semantics small and testable. Record third-party attribution and pin dependency versions.

The intended development VM is a short-lived local QEMU instance with one virtual CPU and 256 MiB RAM. It boots a disposable image; no cloud account is required. Full performance campaigns and model training are outside ordinary local development.

Original project code is MIT licensed. Third-party dependencies retain their own licenses.

Capability revocation currently traverses a fixed-capacity table synchronously in O(N²). Deleting a capability also deletes its descendants. Object teardown must invalidate references in every table before an object ID is reused. These are explicit prototype contracts, not a completed formal security proof.
