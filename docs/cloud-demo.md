# Cloud VM and guest demo

Use an existing **non-Azure** x86-64 Linux VM. Linux hosts QEMU, which boots
MnemosyneOS as a separate guest OS. TCG emulation works without nested
virtualization/KVM. These scripts provision no cloud resources.

## Fast path: prebuilt bundle

Copy `build/mnemosyne-os-demo.tar.gz` to the VM using your SSH/SCP setup:

```powershell
scp .\build\mnemosyne-os-demo.tar.gz user@your-vm:~/
```

On Ubuntu:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends python3 qemu-system-x86
tar -xzf mnemosyne-os-demo.tar.gz
cd mnemosyne-os-demo
python3 tools/run-image.py --self-test
python3 tools/run-image.py
```

A modest 1-vCPU/1-GiB VM is sufficient for the prebuilt demo. QEMU uses one CPU,
256 MiB and no NIC. The CPU-hog intentionally stays busy while the guest runs;
type `shutdown` when finished. Self-test uses fresh test disks and bounded boots.
Interactive mode retains `build/chronicle-disk.img`; existing images are never
truncated. `manifest.json` records the Git revision, file hashes and licenses.
The prebuilt route requires no Rust toolchain.

## Build from source

Use about 2 vCPUs, 4 GiB RAM and 20 GiB disk for source builds. Authenticate
GitHub normally: the repository is private. Install Rust with the official rustup
installer if needed, then:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends git python3 build-essential qemu-system-x86 xorriso
git clone https://github.com/divyamagrawal06/mnemosyne-os.git
cd mnemosyne-os
rustup show
python3 tools/dev.py guest-test --release
python3 tools/dev.py run --release
```

The checkout pins Rust, the target, Cargo dependencies and Limine. Cargo uses two
jobs. No GitHub Actions/cloud build is started.

## Five-minute classroom route

At the **guest** prompt:

```text
about
ps
demo
ls
snapshots
```

Explain the six demo stages:

1. **Scheduling/IPC:** the CPU-hog never yields; timer preemptions increase while
   the console exchanges requests with the counter service.
2. **Authority:** forged generations, wrong capability types and invalid pointers
   are rejected at the syscall boundary.
3. **Containment:** kernel-memory access, invalid and privileged instructions kill
   only the fault service. The healthy service still replies.
4. **Native COW:** checkpoint the counter at blocked receive, copy changed pages,
   restore registers/pages and replay requests. Replies match; old handles fail.
5. **Filesystem:** create a file, snapshot it, edit it and restore prior contents.
6. **Wasm:** record 12 requests/60 events, inject a trap, discard the failed
   instance, verify a plan, restore/replay, compare state/effects and resume.

Success includes `"event":"guest-demo","passed":true` and:

```text
recovered-sha256=30097fe2565ba3716a1aa8c0699c8614caa27b9ca0edcc493a4d60b790a2da11
```

Show persistence:

```text
write professor-note.txt survives-a-complete-vm-reboot
cat professor-note.txt
shutdown
```

Run `python3 tools/run-image.py` again on the host, then:

```text
cat professor-note.txt
recover
kv-add 1 2
ps
shutdown
```

The recovered KV service accepts a new request. Subsequent live KV changes are
volatile; `recover` restores the saved demonstration bundle.

## Filesystem commands

```text
write notes.txt original-content
snapshot before-edit
snapshots
write notes.txt edited-content
cat notes.txt
restore 3
cat notes.txt
rm notes.txt
fs-info
```

Replace `3` with the generation printed by **your** snapshot. Restore also restores
the file set, including recovery artifacts present at that time. The volume has
32 files, a 192-KiB file limit, a 255.5-KiB root limit, and 64 retained generations.
It stops accepting writes when full; no GC is implemented. For a fresh session,
keep the old image and select a new name:

```sh
python3 tools/run-image.py --disk build/classroom-session-2.img
```

## Optional AI proposal

The offline demo uses a deterministic candidate plan, not an LLM call. For the AI
path, run guest `wasm-demo` and `plan-show`; copy its plan JSON to a host file such
as `build/recovery-context.json`. Acceptance tests also export this context from
their test disk. Use context from the same disk/session you intend to recover.

Offline mode:

```sh
python3 tools/ai-plan.py --context build/recovery-context.json \
  --candidate build/recovery-context.json --out build/offline-plan.json
```

Online mode: set `OPENAI_API_KEY` in the **host** environment, and select a Responses
model available to your account that supports structured outputs:

```sh
python3 tools/ai-plan.py --context build/recovery-context.json \
  --model YOUR-MODEL-ID --out build/model-plan.json
```

This explicit command sends the selected context/optional trace to OpenAI and
makes one paid API request. No API key enters the guest. Use `--prompt-only` to
inspect the request without a call. The adapter uses
[Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs);
schema validity does not grant authority. Offline response parsing is tested;
a live provider call was not required for the OS acceptance run.

The host prints three lines to paste into the guest:

```text
write ai-plan.json {the-actual-json-printed-by-the-tool}
verify-plan ai-plan.json
recover-plan ai-plan.json
```

The guest checks its actual checkpoint identity and epoch. Change
`requested-rights` to `255` in a candidate to show `PrivilegeAmplification`
rejection. A rejected plan does not replace the live worker. No model-generated
shell command is executed.

## Evidence and troubleshooting

Self-test verifies both boots, every required success event, unsafe-plan rejection
and a new live KV request. Logs: `build/guest-demo.log`, `build/guest-reboot.log`,
`build/guest-last.log`. The original browser dashboard runs the older host
experiments; the guest console is the current OS demonstration.

- For `chronicle-fs: Full`, use a new named disk; retain the old disk if needed.
- For corruption, keep the image for investigation. The guest will not erase it.
- For plan identity rejection, obtain the current `plan-show`; do not bypass
  verification or alter the trusted service-epoch file.
- `python tools/dev.py boot` isolates hardware checks; `test` isolates host tests.
- Timing uses 10-ms ticks and QEMU scheduling. It is development evidence, not a
  statistically established hardware performance result.

QEMU's terminal/disk options follow its
[official invocation documentation](https://www.qemu.org/docs/master/system/qemu-manpage.html).
