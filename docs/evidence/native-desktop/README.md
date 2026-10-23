# Native desktop acceptance, 23 October 2026

Tested source: `c609f43be488eb1c85f3432ce8dd6056f043721c`, clean release build.
These are recorded QEMU results, not EC2 or physical Raspberry Pi results.
The screenshots are direct QMP framebuffer captures, converted from PPM to PNG.

The extracted `mnemosyne-desktop-demo.tar.gz` passed all 17 steps in
`tools/test-desktop.py`. See [the report](acceptance.json), [first boot](boot-1.log)
and [second boot](boot-2.log). Checks include real PS/2 input, a loopback VNC
handshake, mouse navigation, task 3 stop/start, file editing, 11 invalid display/
capability requests, three contained faults, and persistence across clean boots.
Exactly 430 mixed characters survived writes at measured 24.9998 and 49.9995
characters per second. The oversized file preview rejected Save and retained
all original bytes.

An additional live native-window run verified counter task 2 stop/start followed
by IPC, rejected stopping init, and wrote 15 files to exercise Next/Prev and read
the correct contents on both pages. Its [report](additional-controls.json) and
[serial log](additional-controls.log) are retained. Temporary test files were
removed through the guest shell after the check.

- [Task manager](tasks.png)
- [Native terminal](terminal.png)
- [Files, second page](file-pagination.png)
- [Read-only preview](read-only-preview.png)
- [Saved file after reboot](reboot-file-read.png)

Bundle SHA-256:
`83bc31c357c1e8846791fc7ac9bd3f0502120d845b4c3bedc841068c381f6bdf`.
All 100 manifest file hashes and 52 dependency notice groups were checked.
The bundle contains the ISO, Python launcher/test, EC2 runbook and licenses;
it contains no browser frontend or web server. The source revision above remains
the build target; this evidence is committed separately after the tests.

Reproduce on Linux/WSL with `python3 tools/test-desktop.py --timeout 120`.
This uses one emulated CPU and 256 MiB RAM with isolated disks. The live VNC
viewer connection over EC2 SSH still needs rehearsal on the deployment host.
