# mnemosyne-os

A bootable Rust research OS for capability-controlled services, deterministic
replay and policy-checked recovery. It has its own kernel and interactive serial
console. Run it inside QEMU locally or on a non-Azure Linux cloud VM.

**Working now:** ring-3 tasks with private page tables, preemptive scheduling,
checked syscalls, capability IPC, fault containment, native service copy-on-write
checkpoints, a persistent snapshotting filesystem, guest Wasm replay, and a
deterministic recovery-plan verifier.

The prototype uses one CPU, four compiled-in native tasks and one admitted Wasm
fixture. It is not a desktop OS or a formally verified kernel. See the
[implementation status](docs/implementation-status.md),
[professor brief](docs/professor-brief.md), and [full demo path](docs/cloud-demo.md).

## Run the OS

With the prebuilt demo bundle, only Python 3 and QEMU are required:

```sh
python3 tools/run-image.py
```

At the guest prompt:

```text
help
ps
demo
ls
snapshots
shutdown
```

`demo` runs scheduling/IPC, capability rejection, CPU fault containment, COW
checkpoint/replay, filesystem restore, and durable Wasm recovery **inside the
guest OS**. Files persist in `build/chronicle-disk.img` across boots.

## Build from source

Install Git, Python 3 and rustup. `rust-toolchain.toml` pins Rust and the target.
On Ubuntu, install the external image/emulator tools:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends build-essential qemu-system-x86 xorriso
python3 tools/dev.py guest-test --release
python3 tools/dev.py run --release
```

On Windows use native Rust/Python and Ubuntu WSL for QEMU/xorriso; invoke `python`
instead of `python3`. `MNEMOSYNE_WSL_DISTRO` selects a configured distribution.
The runner fetches pinned Limine boot files and builds its installer. The guest
needs no Linux kernel, network connection, browser, or AI model.

| Command | Result |
| --- | --- |
| `python tools/dev.py build` | Userspace ELF and freestanding kernel |
| `python tools/dev.py os-image --release` | `build/mnemosyne-os-release.iso` |
| `python tools/dev.py run --release` | Interactive guest with persistent disk |
| `python tools/dev.py guest-test --release` | Six-part demo plus reboot/recovery tests |
| `python tools/dev.py boot` | Short kernel hardware checks, then exit |
| `python tools/dev.py test` | Shared Rust correctness/failure tests |
| `python tools/dev.py check` | Formatting and host/kernel/userspace Clippy |
| `python tools/dev.py demo` | Original host-only Wasmi experiment |
| `python tools/package-demo.py` | Prebuilt cloud-demo archive and licenses |

## Architecture

```text
QEMU hardware: one x86-64 CPU, serial port, dedicated IDE disk
  Rust kernel: page tables, interrupts, scheduler, capabilities, IPC, drivers
    ring 3: init-console + ChronicleFS + Wasmi + recovery verifier
    ring 3: counter-service  [native COW/IPC replay]
    ring 3: cpu-hog          [non-yielding preemption probe]
    ring 3: fault-service    [CPU fault containment probe]
```

The privileged init process currently co-locates several services. The
[status document](docs/implementation-status.md) explains this boundary and
remaining decomposition work. Neither full-copy Wasm checkpoints nor native COW
snapshots imply exactly-once external effects.

## Optional AI and browser tools

`tools/ai-plan.py` requests an untrusted typed plan from OpenAI or accepts an
offline candidate. It never executes commands. The guest verifies the plan before
recovery. Online mode requires the user's API key and explicit model; see
[the AI walkthrough](docs/cloud-demo.md#optional-ai-proposal).

`python tools/dashboard.py --open` launches the older host evidence dashboard.
It runs hardware smoke tests and the host replay experiment, separately from
the guest shell. See the [UI guide](ui/README.md).

## Presentation and research

- [Professor brief](docs/professor-brief.md) and [cloud demo runbook](docs/cloud-demo.md)
- [Original PowerPoint](presentation/output/mnemosyne-os-pitch.pptx),
  [PDF](presentation/output/mnemosyne-os-pitch.pdf), and
  [speaker notes](presentation/output/mnemosyne-os-speaker-notes.md)
- [Literature review](docs/deep-research.md), [research plan](docs/research-plan.md),
  [repository source guide](docs/source-reading-guide.md)
- [Third-party versions and licenses](THIRD_PARTY.md)

The original deck preserves its proposal-stage `44b739d` baseline. Current
capabilities are described in the brief and runbook. New paths and commands use
kebab-case; language identifiers and conventional filenames retain their forms.

Original code is MIT licensed. Reused dependencies retain their own licenses.
