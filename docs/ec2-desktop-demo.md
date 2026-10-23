# Native MnemosyneOS desktop on an EC2 Linux host

The desktop pixels are drawn by MnemosyneOS into its bootloader-provided
framebuffer. QEMU supplies an emulated VGA display and PS/2 keyboard/mouse.
A native QEMU window or VNC viewer displays those pixels. This route uses no
web dashboard, browser rendering, remote fonts, npm, or runtime Rust toolchain.
The custom OS runs inside QEMU; the EC2 host still runs a supported Linux OS.
No deployment to an actual EC2 instance has been verified by this runbook.

AWS VM Import/Export lists supported operating systems and image formats.
MnemosyneOS is not a supported imported operating system. This runbook uses
an existing Linux EC2 host running QEMU with TCG instead of claiming an ISO
can become a supported EC2 AMI. It needs no nested virtualization or KVM.
See [AWS VM Import/Export requirements](https://docs.aws.amazon.com/vm-import/latest/userguide/prerequisites.html)
and [QEMU display/VNC options](https://www.qemu.org/docs/master/system/invocation.html).
The configuration uses one emulated x86-64 CPU, 256 MiB guest RAM and no guest
network card. Linux/QEMU need additional host RAM. Prefer an existing host
with at least 1 GiB available RAM for rehearsal; this is a recommendation,
not a measured EC2 minimum or performance claim. No resources are provisioned
by these scripts.

## Build and transfer

On the development checkout, after tests and commits:

```sh
python tools/dev.py desktop-image --release
python tools/package-desktop.py
scp -i your-key.pem build/mnemosyne-desktop-demo.tar.gz ubuntu@your-ec2-host:~/
```

Packaging requires the development Rust dependency cache to collect upstream
license notices, plus a clean checkout and matching committed ISO provenance.
It retains the ISO, provenance, native launcher, acceptance tool and notices.
The resulting runtime bundle includes no web frontend or dashboard backend.

On the existing Ubuntu host:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends python3 qemu-system-x86
tar -xzf mnemosyne-desktop-demo.tar.gz
cd mnemosyne-os-demo
python3 tools/run-desktop.py --check
python3 tools/run-desktop.py --vnc 127.0.0.1:1
```

The launcher verifies the release desktop ISO against its provenance before
starting. The guest disk is `build/desktop/chronicle-disk.img`, a dedicated
16-MiB file retained across boots. Serial diagnostics go to
`build/desktop/serial.log`. Existing disks are never truncated; QEMU rejects
simultaneous writers. Keep this terminal open while presenting.

## Connect to the actual framebuffer

From your laptop, use the same SSH hostname and key:

```sh
ssh -i your-key.pem -N -L 5901:127.0.0.1:5901 ubuntu@your-ec2-host
```

Connect a native VNC viewer to `127.0.0.1` port `5901` (some viewers spell this
`127.0.0.1:1`, others use `127.0.0.1::5901`). VNC carries the guest display
and input; SSH authenticates and encrypts the connection. Leave EC2 security
group port 5901 closed; allow SSH only through your existing access policy.
The launcher rejects non-loopback VNC addresses and creates no public listener.
The VNC endpoint itself has no password and is available to other processes
on the host, so use a trusted host/user account.

On a local machine with QEMU graphical window support, run
`python tools/run-desktop.py` without `--vnc` for a native window. Windows uses
native QEMU when found on PATH, otherwise Ubuntu WSL; a WSL graphical window
requires WSLg. On Ubuntu/WSL, install the display backend with
`sudo apt-get install --no-install-recommends qemu-system-gui` if QEMU lists only
`none` and `curses` under `qemu-system-x86_64 -display help`.
VNC over loopback works without WSLg. A missing graphical
backend or DISPLAY is a host configuration issue, not evidence that the OS
rendered successfully.

For a core-OS presentation:

1. Boot to the native desktop and move the pointer. Use F1 through F4 or the
   launcher to switch between Terminal, Files, Tasks and System.
2. In Terminal, type `help`, `ps` and `uptime`. Explain that the shell calls the
   custom kernel, and the task table reports its real scheduler counters.
3. Open Tasks. Stop the CPU worker (PID 3), observe its stopped state, then Start
   it. Init remains protected. These controls operate on the fixed native tasks;
   they do not load arbitrary executables.
4. In Files, type a short note and press Save file or Enter. Click its filename
   to read the actual bytes from disk. The editor supports single-line ASCII up
   to 420 bytes; larger/multiline files are read-only previews.
5. In System, run the fault-isolation check. Task 4 faults while init and the
   counter service continue. Return to Tasks and restart the fault service.
6. Shut down through System, boot again and open the saved note. Replay and AI
   experiments remain optional terminal commands; they are not needed here.

Keyboard and pointer input are handled by the guest's PS/2 path. The system
is a prototype desktop, not a Linux/X11/Wayland application environment.

Use the guest shutdown command for a clean exit. Ctrl+C in the launcher is an
emergency stop and may interrupt an in-flight operation. Stop QEMU before
copying the disk as a backup. To intentionally reset, move the disk aside
while stopped, then boot again; never pass a host block device as `--disk`.

## Repeatable acceptance

```sh
python3 tools/test-desktop.py --timeout 300
```

This uses real QEMU, a fresh isolated persistent disk, local QMP control and
framebuffer screenshots. It injects keyboard and mouse input and checks the
guest's actual response. It leaves screenshots, serial logs and a JSON report
under `build/desktop-test-*`. It does not validate an EC2 network connection
or a physical machine. Inspect those artifacts and rehearse the VNC viewer
on the actual host before presenting.

If the display is blank, inspect the serial log first and verify this is the
desktop ISO, not the serial-console ISO. If port 5901 is occupied, use
`--vnc 127.0.0.1:2` and forward/connect port 5902. If input is absent, confirm
the viewer has keyboard focus and uses a US keyboard mapping. The emulated
mouse is relative PS/2; a viewer may capture/release the pointer. Do not add
a USB tablet unless the guest gains and tests a USB input driver.
