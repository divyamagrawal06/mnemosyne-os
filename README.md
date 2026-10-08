# MnemosyneOS

A research Rust microkernel investigating capability isolation, deterministic service replay, and policy-checked recovery.

The x86-64 kernel boots through Limine in QEMU and checks serial output, usable physical frames, capability attenuation/revocation, breakpoint handling, and PIT timer interrupts. The allocation-free capability library also has host tests for stale handles, subtree revocation, rights, capacity, object teardown, and generation exhaustion. This is a tested table implementation; a process/syscall security boundary is still pending.

The host replay lab embeds Wasmi and exercises a fixed integer KV workload. It records requests, clock/random inputs, effects, and replies; restores memory and a mutable global into a fresh instance; and checks replay against the log. It tests corrupted snapshots, altered traces, fuel exhaustion, memory limits, and traps. This experiment runs on the host, separately from the kernel.

The research proposal describes the intended system; it is not a claim that all features are implemented. Userspace, process scheduling, IPC, persistent storage, and recovery inside the OS are subsequent milestones. See [implementation status](docs/implementation-status.md) for the next acceptance gates.

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
python tools/dev.py test
python tools/dev.py check
python tools/dev.py demo
```

`boot` builds the kernel and an ISO, runs a headless 1-vCPU/256-MiB QEMU instance, verifies serial events and the exit status, and terminates the test VM. A timeout fails the test. Output is retained in `build/boot.log`; the image is `build/mnemosyne.iso`. Windows builds use native Cargo and WSL only for image construction and emulation. Set `MNEMOSYNE_WSL_DISTRO` to select another configured distribution.

The runner downloads a pinned Limine binary distribution into `.tools/limine` on first use. See [third-party attribution](THIRD_PARTY.md). BIOS boot is tested; UEFI files are included but UEFI boot is not yet tested.

`test`, `check`, and `demo` use native Cargo on Windows and require no VM. `demo` writes `build/replay-demo.json`. Host tests are the workspace default; `cargo test --workspace` would incorrectly include the freestanding kernel on a host target. The runner selects the kernel target explicitly for its build and lint checks.

## Replay experiment contract

The bundled workload has one 64-KiB memory and one mutable `i64` global, no tables, no threads, no floating point, and no start function. Wasmi validates it, meters each request with 10,000 fuel, and restricts memory to one page. A checkpoint copies all state in this fixture's schema between completed handlers. It is not a copy-on-write or mid-execution checkpoint.

The bounded typed log is bound to the checkpoint digest, module hash, and execution profile. Replay injects recorded inputs and compares request arguments, effect payloads, and replies. Any trap or mismatch poisons the instance. Recovery constructs a fresh instance; it never continues with partially mutated state. An unread trace suffix also fails completion.

Clock and random providers are controlled synthetic inputs. Effects are observations held in memory; there is no network or filesystem delivery. The lab does not yet establish durable commits, crash consistency, exactly-once delivery, authenticated logs, general Wasm snapshotting, or replay across runtime upgrades. The snapshot checksum detects corruption; it does not establish that a checkpoint is semantically safe or authorize a recovery action.

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
