# MnemosyneOS presentation and demo guide

The current demo can use the local browser dashboard or PowerShell, a headless QEMU guest, and a host Wasmi experiment. It shows working code and real correctness checks. The kernel has no graphical desktop or interactive shell yet. Statistical benchmark tooling, ChronicleFS, isolated userspace services, and the AI recovery verifier are future milestones.

## Browser demo route

```powershell
Set-Location 'C:\Users\ludic\Desktop\mnemosyne'
python tools/dashboard.py --open
```

The browser opens `http://127.0.0.1:8765`. At slide 15, switch to the dashboard, select **Full demonstration**, and click **Run demo**. The source switches to **Latest run**. Follow the three stages on Overview, then inspect **Kernel**, **Test suite**, and **Replay inspector**. In Replay, compare the matching state hashes, explain the rejected histories, filter the actual trace by event type, and select an event to inspect its payload. **Raw console** exposes the original command output. **Export evidence** downloads the selected run as JSON.

Use **Saved demonstration** for the original recorded fallback. Its capture date and code revision remain visible; it is not a fresh execution. That original report has aggregate results only. The trace inspector becomes available on a current live replay run. **Host checks only** skips QEMU; **Stop after step** finishes the active command and skips subsequent stages.

The original deck describes its stated `44b739d` baseline. Explain that this local browser UI was added afterward and is a host control surface, not a desktop running inside the OS. The [UI guide](../ui/README.md) covers setup, persistence, and troubleshooting. The three terminal acts below remain useful for narration and as an alternative route.

## Fast start on this computer

Open PowerShell or Windows Terminal, increase the font size to about 20 points, and run:

```powershell
Set-Location 'C:\Users\ludic\Desktop\mnemosyne'
python tools/present.py
```

Press Enter before each of the three steps. The script runs the existing build/test commands and stops on failure. It writes a fresh run record to `build/presentation-demo.json` and a transcript to `build/presentation-demo.log`.

For a rehearsal without pauses:

```powershell
python tools/present.py --auto
```

The presentation files are in `C:\Users\ludic\Desktop\mnemosyne\presentation\output`. Open the PPTX in PowerPoint and start Slide Show with F5. Speaker notes contain narration, source links, and the boundaries of each claim. Use Alt+Tab to switch to the terminal for the demo. The PDF is a viewing fallback and does not contain the speaker notes.

## Presentation route

Allow roughly 15-20 minutes for the main presentation and 5-7 minutes for the narrated demo. These are speaking-time estimates, not performance measurements. Slides 1-22 form the main pitch. Slides 23-30 provide technical backup, exact commands, the formal proposal, and references.

Explain the problem and intended contribution first. Walk through the proposed architecture, then state which parts run today. Switch to the terminal when you reach the live-demo slide. Return to the deck for evaluation, milestones, and discussion. Use the backup slides when the teacher asks about trust, deployment, formal verification, or the proposal abstract.

## Before the presentation

The current machine already has native Rust, Git, Python, Ubuntu WSL, QEMU, and xorriso. Rehearse before going offline so Cargo packages and the pinned Limine bootloader are cached.

```powershell
Set-Location 'C:\Users\ludic\Desktop\mnemosyne'
git status --short
python --version
cargo --version
rustup show active-toolchain
wsl -l -v
wsl -d Ubuntu -- qemu-system-x86_64 --version
wsl -d Ubuntu -- xorriso -version
python tools/dev.py check
python tools/present.py --auto
```

On a fresh Windows computer, install Git, Python 3, Rust/rustup, and an Ubuntu WSL distribution before rehearsing. The repository pins the Rust toolchain and target. Install the Linux image tools once inside Ubuntu:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends build-essential qemu-system-x86 xorriso
```

Builds use native Windows Cargo. WSL handles image creation and QEMU. The VM uses one vCPU, 256 MiB RAM, software emulation, no network device, and a 25-second guest timeout. No cloud VM is required. On Linux, run the Python commands from the repository root without the PowerShell `Set-Location` command.

## Act 1: boot and CPU protection

Manual command, also run by the guided demo:

```powershell
python tools/dev.py boot
```

Say: “This command builds our Rust kernel, packages it with Limine, and boots it in QEMU. The guest sends structured events over its serial port. It exits automatically when its checks complete.”

Look for these events in the output:

```json
{"event":"frames_ok","count":2}
{"event":"capabilities_ok"}
{"event":"paging_ok","fault_probes":3}
{"event":"exception_ok","vector":3}
{"event":"timer_ok","ticks":5}
{"event":"boot_ok"}
```

The memory-region and usable-byte counts can vary with the build. The timer must deliver at least five ticks. The runner requires successful completion events and the expected QEMU exit code.

Explain the three `paging_ok` probes: execute a non-executable page, write a read-only page, and read a page after unmapping. Each must produce the expected CPU fault. A narrowly matched test handler resumes only those deliberate probes. Other kernel faults remain fatal.

Say: “These are real CPU protection checks in the boot address space. Separate userspace processes and a preemptive scheduler are the next milestone.” Timer delivery alone does not demonstrate preemption.

Optional optimized-build demonstration:

```powershell
python tools/dev.py boot --release
```

Evidence: `build/boot.log`, `build/boot-release.log`, `build/mnemosyne.iso`, and `build/mnemosyne-release.iso`. A standalone QEMU process returns code 33 on this successful smoke path because the guest uses the debug-exit device. The Python runner translates the expected result into success.

## Act 2: capability and failure tests

```powershell
python tools/dev.py test
```

Say: “The allocation-free capability table checks object types, allowed operations, delegation, and revocation. A child capability can only carry a subset of its parent's rights. Generation numbers prevent stale handles from becoming valid when slots are reused.”

Expected baseline: 8 capability tests and 16 Wasmi/replay tests, all passing. Additional empty binary/doc-test groups are normal. Capability enforcement across processes still requires the future syscall boundary and object registry.

For a focused question, run one of these exact tests:

```powershell
cargo test --locked -p mnemosyne-capability delegation_attenuates_and_rejects_amplification -- --nocapture
cargo test --locked -p mnemosyne-capability deletion_removes_descendants_and_stale_handles_never_alias_reuse -- --nocapture
cargo test --locked -p mnemosyne-replay-lab wasm_trap_after_emit_discards_pending_effect_and_mutated_state -- --nocapture
cargo test --locked -p mnemosyne-replay-lab infinite_loop_terminates_with_fuel_trap -- --nocapture
```

Passing negative tests means the expected rejection or trap occurred. It does not mean that the test silently skipped the failure. Do not edit code during the presentation to manufacture a failure.

## Act 3: checkpoint and replay

```powershell
python tools/dev.py demo
```

Say: “This experiment runs Wasmi on the host. It executes a small integer key-value workload, takes a checkpoint after four requests, and records a suffix of twelve requests. We then create a fresh instance with different live input seeds, restore the checkpoint, and replay the recorded suffix.”

Explain the result fields:

| Field | Expected baseline | Meaning |
| --- | --- | --- |
| `passed` | `true` | All demo checks succeeded |
| `replayed_requests` | `12` | Requests after the checkpoint |
| `trace_events` | `60` | Five events per request |
| `snapshot_mutable_state_bytes` | `65544` | 65,536 memory bytes plus one 8-byte global, excluding metadata |
| `state_equal`, `effects_equal` | Both `true` | Exact snapshot comparison and identical effect observations |
| `final_state_sha256`, `replay_state_sha256` | Identical | Compact labels for the equal states |
| Four `*_rejected` fields | All `true` | Missing, reordered, extra events and a corrupted checkpoint fail recovery |

The baseline final-state digest is:

```text
30097fe2565ba3716a1aa8c0699c8614caa27b9ca0edcc493a4d60b790a2da11
```

The fixture log sequence is request, clock, random, effect, reply. Replay injects recorded clock/random values and compares the other events. A mismatch or trap poisons the instance. A successful replay must consume the complete trace. A failed handler cannot produce a new checkpoint or publish its pending in-memory effects.

Say: “This proves the fixture's replay behavior under its explicit state schema. It uses a full copy at handler boundaries. Durable logging, copy-on-write checkpoints, external exactly-once effects, and recovery inside the OS remain later work.”

Open the result without rerunning:

```powershell
Get-Content build/replay-demo.json
```

## Failure fallback

If WSL/QEMU is unavailable, continue with a fresh host-only run:

```powershell
python tools/present.py --host-only
```

Say: “The kernel boot is skipped on this machine. The tests and Wasm replay are running live.”

If live execution cannot run, show the committed rehearsal evidence:

```powershell
python tools/present.py --recorded
```

This mode prints its original capture time and code commit and labels every result as recorded evidence. It does not build, boot, or test anything. Say that explicitly. The saved files are in `presentation/evidence`, with checksums and provenance in `manifest.json`. The deck's evidence slides are a second fallback.

## Troubleshooting

| Symptom | Action |
| --- | --- |
| `python` or `cargo` is not found | Open a fresh terminal after installing the required tool. Verify versions before the talk. |
| Ubuntu distribution not found | Use `wsl -l -v`. Set `$env:MNEMOSYNE_WSL_DISTRO = 'YourDistributionName'` to an existing Linux distribution with the tools installed. |
| QEMU, xorriso, or make missing | Install the Linux prerequisites in that distribution, then rehearse again. |
| Toolchain or dependency download starts during the talk | Use the recorded fallback. Complete the online rehearsal beforehand. |
| QEMU timeout, panic, or missing `boot_ok` | Read `build/boot.log`. Do not describe the boot as passing. Use host-only mode or saved evidence. |
| Replay result is false or command exits with an error | Keep the error visible. Use the pinned recorded baseline and explain that this live result failed. |
| PowerPoint is unavailable | Present the PDF and keep the demo guide open separately. |

## What to say about AI and verification

The current implementation makes no model calls. The proposed AI adapter emits a bounded typed plan. The deterministic verifier will check current capability authority, resource reservations, execution bounds, privilege amplification, and rollback availability before a trusted executor acts.

An ordinary parser, checksum, test suite, or deterministic checker is not a whole-system formal proof. The project's use of “verified recovery” is a proposed acceptance boundary. It must become an explicit specification and checked implementation before being presented as a completed guarantee.

The teacher-facing completion demonstration will require isolated services, persistent ChronicleFS state, durable replay and output handling, rejection of an unauthorized recovery plan, and continued operation of a healthy sibling service. Today's demo establishes the first kernel and replay foundations for that system.
