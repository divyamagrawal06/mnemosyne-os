use crate::{Console, println, sys};
use alloc::{
    format,
    string::{String, ToString},
};
use core::fmt::Write;
use mnemosyne_abi::*;
use mnemosyne_chronicle_fs::{BlockDevice, Error as FsError, FileSystem, Result as FsResult};
use mnemosyne_recovery_policy::{self as policy, Action, Context, Plan};
use mnemosyne_service_runtime::{self as runtime, RecoveryBundle, Request, Worker};

struct Disk;
impl BlockDevice for Disk {
    fn read(&mut self, lba: u64, out: &mut [u8; 512]) -> FsResult<()> {
        if sys::call(SYS_DISK_READ, lba, out.as_mut_ptr() as u64, 0, 0).0 == 0 {
            Ok(())
        } else {
            Err(FsError::Io)
        }
    }
    fn write(&mut self, lba: u64, bytes: &[u8; 512]) -> FsResult<()> {
        if sys::call(SYS_DISK_WRITE, lba, bytes.as_ptr() as u64, 0, 0).0 == 0 {
            Ok(())
        } else {
            Err(FsError::Io)
        }
    }
    fn flush(&mut self) -> FsResult<()> {
        if sys::call(SYS_DISK_FLUSH, 0, 0, 0, 0).0 == 0 {
            Ok(())
        } else {
            Err(FsError::Io)
        }
    }
}
type Result<T> = core::result::Result<T, String>;
fn fs_error(error: FsError) -> String {
    format!("chronicle-fs: {error:?}")
}
fn require(test: bool, message: &str) -> Result<()> {
    if test {
        Ok(())
    } else {
        Err(String::from(message))
    }
}
fn number(value: Option<&str>) -> Result<u64> {
    value
        .ok_or_else(|| String::from("missing number"))?
        .parse()
        .map_err(|_| String::from("invalid number"))
}

struct Shell {
    fs: Option<FileSystem<Disk>>,
    worker: Option<Worker>,
}
pub fn run() -> ! {
    println!("\nmnemosyne-os 0.1 / x86-64 / ring-3 console");
    println!("4 protected tasks | preemptive scheduler | capability IPC | chronicle-fs");
    let fs = match FileSystem::mount(Disk) {
        Ok(fs) => Some(fs),
        Err(FsError::Blank) => FileSystem::create_blank(Disk).ok(),
        Err(error) => {
            println!("disk mount failed: {error:?}");
            None
        }
    };
    let mut shell = Shell { fs, worker: None };
    if let Some(fs) = &shell.fs {
        println!(
            "chronicle-fs mounted: generation={} retained={}",
            fs.generation(),
            fs.history().len()
        );
    }
    println!("Type help for commands, demo for the full demonstration, shutdown to exit.");
    if let Some(desktop) = crate::desktop::Desktop::new() {
        shell.run_desktop(desktop);
    }
    loop {
        let _ = write!(Console, "mnemosyne-os> ");
        let line = crate::read_line();
        if let Err(error) = shell.command(line.trim()) {
            println!("error: {error}");
        }
    }
}

impl Shell {
    fn desktop_model(&self, desktop: &mut crate::desktop::Desktop) {
        let files = self
            .fs
            .as_ref()
            .map(|fs| {
                fs.files()
                    .map(|(name, size)| (name.to_string(), size))
                    .collect()
            })
            .unwrap_or_default();
        desktop.update_model(files, sys::stats(), sys::ticks());
    }

    fn run_desktop(&mut self, mut desktop: crate::desktop::Desktop) -> ! {
        // First-boot content lives on the actual guest disk, not in a UI fixture.
        if let Some(fs) = &mut self.fs
            && fs.generation() == 1
            && fs.files().next().is_none()
        {
            let _ = fs.write("welcome.txt", b"Welcome to MnemosyneOS. This file lives on your guest disk. Edit it in Files or use: write note.txt hello");
        }
        self.desktop_model(&mut desktop);
        desktop.present();
        println!("{{\"event\":\"native-desktop-ready\"}}");
        let mut refreshed = sys::ticks();
        loop {
            if let Some(line) = desktop.poll() {
                let command = line.trim();
                if !command.is_empty() {
                    // This echo and every command result also appear on serial,
                    // allowing independent QMP/keyboard integration tests.
                    println!("mnemosyne-os> {command}");
                    desktop.present();
                    crate::begin_capture();
                    let result = self.command(command);
                    if let Err(error) = &result {
                        println!("error: {error}");
                    }
                    let output = crate::end_capture();
                    desktop.write_console(&output);
                    desktop.command_complete(result.is_ok(), &output);
                    if result.is_ok()
                        && let Some(name) = command.strip_prefix("cat ")
                        && let Some(fs) = &self.fs
                        && let Ok(bytes) = fs.read(name.trim())
                        && let Ok(contents) = core::str::from_utf8(bytes)
                    {
                        desktop.set_file_contents(name.trim(), contents);
                    }
                    self.desktop_model(&mut desktop);
                    desktop.present();
                    println!(
                        "{{\"event\":\"native-command-complete\",\"ok\":{}}}",
                        result.is_ok()
                    );
                    refreshed = sys::ticks();
                }
            }
            if sys::ticks().saturating_sub(refreshed) >= 100 {
                self.desktop_model(&mut desktop);
                refreshed = sys::ticks();
            }
            sys::sleep(1);
        }
    }

    fn fs(&mut self) -> Result<&mut FileSystem<Disk>> {
        self.fs
            .as_mut()
            .ok_or_else(|| String::from("no mounted filesystem"))
    }
    fn command(&mut self, command: &str) -> Result<()> {
        let mut words = command.split_whitespace();
        match words.next().unwrap_or("") {
            "" => (),
            "help" => {
                println!("system: help, about, ps, uptime, stop <pid>, start <pid>, shutdown");
                println!("files:  ls, cat <name>, write <name> <text>, rm <name>, fs-info");
                println!("        snapshot <label>, snapshots, restore <generation>");
                println!(
                    "tests:  ipc-demo, cap-demo, fault-demo, cow-demo, wasm-demo, demo, self-test"
                );
                println!(
                    "replay: recover, plan-show, verify-plan <name>, recover-plan <name>, kv-add <key> <delta>"
                );
                println!("bench:  bench (100 IPC round trips; timer resolution 10 ms)");
                println!(
                    "        bench-suite (5 samples of IPC, capability, Wasm and fault checks)"
                );
            }
            "about" => {
                println!(
                    "Rust kernel + Limine + rust-osdev; userspace Wasmi, ChronicleFS, policy verifier."
                );
                println!(
                    "One CPU, four ring-3 tasks, private page tables, 100-Hz timer, bounded IPC."
                );
                println!(
                    "Fixed Wasm fixture; synthetic clock/random; in-memory effects; no formal proof."
                );
                println!(
                    "Filesystem: flat names, 32 files, 64 retained generations, 255.5-KiB root limit."
                );
            }
            "uptime" => println!("uptime-ticks={} tick-ms=10", sys::ticks()),
            "stop" | "start" => {
                let operation = command.split_whitespace().next().unwrap();
                let pid = number(words.next())?;
                require(
                    (2..=4).contains(&pid),
                    "only service PIDs 2..4 may be controlled; init is protected",
                )?;
                require(words.next().is_none(), "provide one service PID")?;
                let syscall = if operation == "stop" {
                    SYS_TASK_STOP
                } else {
                    SYS_RESTART
                };
                require(
                    sys::call(syscall, pid, 0, 0, 0).0 == 0,
                    "service operation failed",
                )?;
                println!("task-{operation}: pid={pid}");
            }
            "ps" => {
                println!("pid  service          state       switches  preemptions  faults  epoch");
                for task in sys::stats() {
                    let name = [
                        "init-console",
                        "counter-service",
                        "cpu-hog",
                        "fault-service",
                    ][(task.pid - 1) as usize];
                    let state = ["ready", "receive", "sleep", "dead"][task.state as usize];
                    println!(
                        "{}    {:16} {:8}    {}          {}         {}       {}",
                        task.pid,
                        name,
                        state,
                        task.switches,
                        task.preemptions,
                        task.faults,
                        task.epoch
                    );
                }
            }
            "ls" => {
                for (name, bytes) in self.fs()?.files() {
                    println!("{name}  {bytes} bytes");
                }
            }
            "fs-info" => {
                let fs = self.fs()?;
                println!(
                    "generation={} retained={} free-generations={} max-file-bytes=196608",
                    fs.generation(),
                    fs.history().len(),
                    64 - fs.history().len()
                );
            }
            "cat" => {
                let name = words
                    .next()
                    .ok_or_else(|| String::from("usage: cat <name>"))?;
                let data = self.fs()?.read(name).map_err(fs_error)?;
                println!(
                    "{}",
                    core::str::from_utf8(data).map_err(|_| String::from("file is not UTF-8"))?
                );
            }
            "write" => {
                let rest = command.strip_prefix("write").unwrap().trim_start();
                let (name, text) = rest
                    .split_once(' ')
                    .ok_or_else(|| String::from("usage: write <name> <text>"))?;
                let generation = self.fs()?.write(name, text.as_bytes()).map_err(fs_error)?;
                println!("committed generation={generation}");
            }
            "rm" => {
                let name = words
                    .next()
                    .ok_or_else(|| String::from("usage: rm <name>"))?;
                println!(
                    "committed generation={}",
                    self.fs()?.remove(name).map_err(fs_error)?
                );
            }
            "snapshot" => {
                let label = words
                    .next()
                    .ok_or_else(|| String::from("usage: snapshot <label>"))?;
                println!(
                    "snapshot generation={}",
                    self.fs()?.snapshot(label).map_err(fs_error)?
                );
            }
            "snapshots" => {
                for g in self.fs()?.history() {
                    println!("generation={} label={} bytes={}", g.id, g.label, g.bytes);
                }
            }
            "restore" => {
                let id = number(words.next())?;
                println!(
                    "restored generation={id} as generation={}",
                    self.fs()?.restore(id).map_err(fs_error)?
                );
                self.worker = None;
            }
            "ipc-demo" => ipc_demo()?,
            "cap-demo" => cap_demo()?,
            "fault-demo" => fault_demo()?,
            "cow-demo" => cow_demo()?,
            "wasm-demo" => self.wasm_demo()?,
            "recover" => self.recover("recovery-plan.json", false)?,
            "plan-show" => {
                let bytes = self.fs()?.read("recovery-plan.json").map_err(fs_error)?;
                println!(
                    "{}",
                    core::str::from_utf8(bytes).unwrap_or("invalid plan text")
                );
            }
            "verify-plan" | "recover-plan" => {
                let name = words
                    .next()
                    .ok_or_else(|| String::from("provide a plan file name"))?;
                self.recover(name, command.starts_with("verify-plan"))?;
            }
            "kv-add" => {
                let key = number(words.next())?;
                require(key < 16, "key must be 0..15")?;
                let delta: i64 = words
                    .next()
                    .ok_or_else(|| String::from("missing delta"))?
                    .parse()
                    .map_err(|_| String::from("invalid delta"))?;
                let worker = self
                    .worker
                    .as_mut()
                    .ok_or_else(|| String::from("run wasm-demo or recover first"))?;
                let value = worker
                    .apply(Request {
                        key: key as i32,
                        delta,
                    })
                    .map_err(|e| e.to_string())?;
                println!(
                    "kv-service key={key} value={value} (live; run recover to restore durable bundle)"
                );
            }
            "demo" | "self-test" => self.demo()?,
            "bench-suite" => benchmark_suite()?,
            "bench" => {
                let start = sys::ticks();
                for i in 0..100 {
                    rpc(0, i)?;
                }
                println!(
                    "ipc-round-trips=100 elapsed-ticks={} tick-ms=10 environment=qemu-tcg",
                    sys::ticks() - start
                );
            }
            "shutdown" => sys::shutdown(0),
            _ => return Err(String::from("unknown command; type help")),
        }
        Ok(())
    }
    fn wasm_demo(&mut self) -> Result<()> {
        println!("wasm: recording 12 requests and 60 events; injecting a trap...");
        let bundle = runtime::capture_fault_demo().map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&bundle).map_err(|e| e.to_string())?;
        let epoch = self.fs()?.generation() + 1;
        let plan = Plan {
            version: 1,
            action: Action::RestoreReplay,
            service: String::from("kv-service"),
            epoch,
            checkpoint_sha256: bundle.checkpoint_sha256(),
            max_events: bundle.event_count(),
            fuel_per_request: 10000,
            memory_pages: 1,
            requested_rights: 3,
            require_rollback: true,
        };
        let plan = serde_json::to_vec(&plan).map_err(|e| e.to_string())?;
        let epoch = epoch.to_string();
        self.fs()?
            .write_batch(
                &[
                    ("recovery-bundle.json", &bytes),
                    ("recovery-plan.json", &plan),
                    ("service-epoch.txt", epoch.as_bytes()),
                ],
                "wasm-checkpoint",
            )
            .map_err(fs_error)?;
        self.worker = None;
        println!(
            "wasm: failed instance discarded; checkpoint, trace, and candidate plan committed to disk."
        );
        self.recover("recovery-plan.json", false)
    }
    fn recover(&mut self, name: &str, verify_only: bool) -> Result<()> {
        let fs = self.fs()?;
        let bundle: RecoveryBundle =
            serde_json::from_slice(fs.read("recovery-bundle.json").map_err(fs_error)?)
                .map_err(|e| e.to_string())?;
        bundle.validate().map_err(|e| e.to_string())?;
        let digest = bundle.checkpoint_sha256();
        let epoch: u64 = core::str::from_utf8(fs.read("service-epoch.txt").map_err(fs_error)?)
            .map_err(|e| e.to_string())?
            .parse()
            .map_err(|_| String::from("invalid service epoch"))?;
        let source = fs.read(name).map_err(fs_error)?;
        let context = Context {
            service: "kv-service",
            epoch,
            checkpoint_sha256: &digest,
            authority_to_recover: true,
            granted_rights: 3,
            rollback_available: true,
            event_count: bundle.event_count(),
            max_events: 4096,
            fuel_per_request: 10000,
            memory_pages: 1,
        };
        let verified =
            policy::verify(source, &context).map_err(|e| format!("plan rejected: {e:?}"))?;
        if verify_only {
            println!("plan-verified=true execution=false");
            return Ok(());
        }
        verified
            .revalidate(&context)
            .map_err(|e| format!("plan became stale: {e:?}"))?;
        let start = sys::ticks();
        let (worker, report) = runtime::recover_live(&bundle).map_err(|e| e.to_string())?;
        let elapsed = sys::ticks() - start;
        let report_json = serde_json::to_vec(&report).map_err(|e| e.to_string())?;
        fs.write("recovery-result.json", &report_json)
            .map_err(fs_error)?;
        self.worker = Some(worker);
        println!(
            "{{\"event\":\"wasm-recovery\",\"passed\":true,\"requests\":{},\"events\":{},\"state-equal\":{},\"effects-equal\":{},\"elapsed-ticks\":{elapsed}}}",
            report.requests, report.events, report.state_equal, report.effects_equal
        );
        println!("recovered-sha256={}", report.final_sha256);
        Ok(())
    }
    fn demo(&mut self) -> Result<()> {
        println!("[1/6] scheduling and isolated IPC");
        ipc_demo()?;
        println!("[2/6] syscall authority and bad pointers");
        cap_demo()?;
        println!("[3/6] CPU fault containment");
        fault_demo()?;
        println!("[4/6] native service copy-on-write checkpoint and IPC replay");
        cow_demo()?;
        println!("[5/6] persistent filesystem and snapshot restore");
        let fs = self.fs()?;
        fs.write("demo-note.txt", b"before-change")
            .map_err(fs_error)?;
        let generation = fs.snapshot("before-demo-edit").map_err(fs_error)?;
        fs.write("demo-note.txt", b"after-change")
            .map_err(fs_error)?;
        fs.restore(generation).map_err(fs_error)?;
        require(
            fs.read("demo-note.txt").map_err(fs_error)? == b"before-change",
            "filesystem restore mismatch",
        )?;
        println!(
            "{{\"event\":\"filesystem-demo\",\"passed\":true,\"restored-generation\":{generation}}}"
        );
        println!("[6/6] Wasm fault, durable checkpoint, verified plan, replay and resume");
        self.wasm_demo()?;
        println!("{{\"event\":\"guest-demo\",\"passed\":true}}");
        Ok(())
    }
}

fn benchmark_sample(name: &str, sample: u64, operations: u64, start: u64) {
    let elapsed = sys::ticks() - start;
    println!(
        "{{\"event\":\"benchmark-sample\",\"benchmark\":\"{name}\",\"sample\":{sample},\"operations\":{operations},\"elapsed-ticks\":{elapsed},\"tick-ms\":10}}"
    );
}

/// Bounded guest measurements. Timer ticks include scheduling and emulator
/// overhead; zero ticks mean below resolution, never zero-cost execution.
/// Fresh workers keep traces bounded and leave the durable demo bundle intact.
fn benchmark_suite() -> Result<()> {
    println!(
        "bench-suite: QEMU guest ticks include scheduling; resolution=10ms; zero means below resolution."
    );
    let bundle = runtime::capture_fault_demo().map_err(|e| e.to_string())?;
    for sample in 1..=5 {
        let start = sys::ticks();
        for i in 0..100 {
            rpc(0, 1000 + i)?;
        }
        benchmark_sample("ipc-round-trip", sample, 100, start);

        let start = sys::ticks();
        for _ in 0..1000 {
            core::hint::black_box(sys::cap(2));
        }
        benchmark_sample("capability-query-syscall", sample, 1000, start);
        let (slot, generation) = sys::cap(2);
        let start = sys::ticks();
        for _ in 0..1000 {
            require(
                sys::call(SYS_SEND, slot, generation.wrapping_add(999), 0, 0).0 == ERR_DENIED,
                "benchmark accepted invalid authority",
            )?;
        }
        benchmark_sample("capability-rejection-syscall", sample, 1000, start);

        let mut worker = Worker::record(42 + sample).map_err(|e| e.to_string())?;
        let start = sys::ticks();
        for i in 0..100 {
            worker
                .apply(Request {
                    key: i % 16,
                    delta: i64::from(i) - 50,
                })
                .map_err(|e| e.to_string())?;
        }
        benchmark_sample("wasm-recorded-request", sample, 100, start);
        let expected = worker.snapshot().map_err(|e| e.to_string())?.digest_hex();
        let start = sys::ticks();
        for _ in 0..20 {
            core::hint::black_box(worker.snapshot().map_err(|e| e.to_string())?);
        }
        benchmark_sample("wasm-checkpoint-with-digest", sample, 20, start);
        require(
            worker.snapshot().map_err(|e| e.to_string())?.digest_hex() == expected,
            "checkpoint changed worker state",
        )?;

        let start = sys::ticks();
        for _ in 0..5 {
            let report = runtime::recover_bundle(&bundle).map_err(|e| e.to_string())?;
            require(
                report.passed && report.state_equal && report.effects_equal,
                "benchmark replay mismatch",
            )?;
        }
        benchmark_sample("wasm-restore-replay", sample, 5, start);
        let start = sys::ticks();
        fault_demo()?;
        benchmark_sample("native-fault-containment", sample, 3, start);
    }
    println!("{{\"event\":\"benchmark-suite\",\"passed\":true,\"samples\":35,\"tick-ms\":10}}");
    Ok(())
}

fn rpc(value: u64, tag: u64) -> Result<u64> {
    require(sys::send(2, value, tag) == 0, "IPC send rejected")?;
    let reply = sys::receive();
    require(
        reply.sender == 2 && reply.tag == tag,
        "IPC reply identity mismatch",
    )?;
    Ok(reply.value)
}
fn ipc_demo() -> Result<()> {
    let before = sys::stats()[2].preemptions;
    let a = rpc(1, 10)?;
    let b = rpc(5, 11)?;
    sys::sleep(4);
    require(b == a + 5, "counter-service reply mismatch")?;
    let after = sys::stats()[2].preemptions;
    require(after > before, "non-yielding task was not preempted")?;
    println!(
        "{{\"event\":\"ipc-demo\",\"passed\":true,\"counter\":{b},\"hog-preemptions\":{after}}}"
    );
    Ok(())
}
fn cap_demo() -> Result<()> {
    let (slot, generation) = sys::cap(2);
    require(
        sys::call(SYS_SEND, slot, generation + 999, 1, 0).0 == ERR_DENIED,
        "forged generation accepted",
    )?;
    require(
        sys::call(SYS_SEND, 0, 1, 1, 0).0 == ERR_DENIED,
        "wrong capability type accepted",
    )?;
    require(
        sys::call(SYS_WRITE, u64::MAX - 1, 8, 0, 0).0 == ERR_INVALID,
        "overflow pointer accepted",
    )?;
    require(
        sys::call(SYS_TASKS, 0x400000, 0, 0, 0).0 == ERR_INVALID,
        "read-only output pointer accepted",
    )?;
    require(
        sys::call(SYS_WRITE, 0xffffffff80000000, 8, 0, 0).0 == ERR_INVALID,
        "kernel pointer accepted",
    )?;
    let mut rejections = 5;
    let mut display = DisplayInfo::default();
    if sys::call(SYS_DISPLAY_INFO, &mut display as *mut _ as u64, 0, 0, 0).0 == 0 {
        require(
            sys::call(SYS_DISPLAY_INFO, 0, 0, 0, 0).0 == ERR_INVALID,
            "null display-info output accepted",
        )?;
        require(
            sys::call(SYS_INPUT, 0, 0, 0, 0).0 == ERR_INVALID,
            "null input output accepted",
        )?;
        let pixel = 0u32;
        let one = (1u64 << 32) | 1;
        require(
            sys::call(SYS_DISPLAY_BLIT, 0, 0, one, 0).0 == ERR_INVALID,
            "null blit input accepted",
        )?;
        for (xy, wh) in [
            (((display.width - 1) as u64) << 32, (2u64 << 32) | 1),
            (u64::MAX, one),
            (0, (1024u64 << 32) | 17),
        ] {
            require(
                sys::call(SYS_DISPLAY_BLIT, &pixel as *const _ as u64, xy, wh, 0).0 == ERR_INVALID,
                "invalid display rectangle accepted",
            )?;
        }
        rejections += 6;
    }
    println!("{{\"event\":\"capability-demo\",\"passed\":true,\"rejections\":{rejections}}}");
    Ok(())
}
fn fault_demo() -> Result<()> {
    let before = sys::stats()[3].faults;
    for mode in 1..=3 {
        require(
            sys::call(SYS_RESTART, 4, 0, 0, 0).0 == 0,
            "fault-service restart failed",
        )?;
        require(sys::send(4, mode, 0) == 0, "fault injection rejected")?;
        sys::sleep(5);
        require(sys::stats()[3].state == 3, "fault was not contained")?;
        rpc(0, 99)?;
    }
    require(sys::stats()[3].faults == before + 3, "missing CPU fault")?;
    println!("{{\"event\":\"fault-demo\",\"passed\":true,\"contained\":3}}");
    Ok(())
}
fn cow_demo() -> Result<()> {
    rpc(0, 100)?;
    sys::sleep(2);
    let status = sys::call(SYS_CHECKPOINT, 2, 0, 0, 0).0;
    require(
        status == 0 || status == ERR_FULL,
        "service is not at a checkpoint boundary",
    )?;
    require(
        sys::call(SYS_RESTORE, 2, 0, 0, 0).0 == 0,
        "COW restore failed",
    )?;
    let base = rpc(0, 101)?;
    let expected = [rpc(11, 102)?, rpc(7, 103)?];
    let old = sys::cap(2);
    require(
        sys::call(SYS_RESTORE, 2, 0, 0, 0).0 == 0,
        "COW rollback failed",
    )?;
    require(
        sys::call(SYS_SEND, old.0, old.1, 1, 0).0 == ERR_DENIED,
        "stale endpoint survived restore",
    )?;
    let replay = [rpc(11, 102)?, rpc(7, 103)?];
    let copies = sys::call(SYS_COW_FAULTS, 2, 0, 0, 0).0;
    require(
        expected == replay && replay[1] == base + 18 && copies > 0,
        "COW replay diverged",
    )?;
    println!(
        "{{\"event\":\"cow-replay\",\"passed\":true,\"messages\":2,\"page-copies\":{copies},\"stale-capability-rejected\":true}}"
    );
    Ok(())
}
