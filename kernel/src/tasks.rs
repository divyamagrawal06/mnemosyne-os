//! Single-core round-robin scheduler with ring-3 traps and capability-checked IPC.
use crate::{
    arch,
    memory::BootFrames,
    serialln,
    user_memory::{AddressSpace, UserPage},
};
use alloc::vec::Vec;
use core::{arch::global_asm, mem::size_of};
use mnemosyne_abi::*;
use mnemosyne_capability::{Handle, ObjectKind, Rights, Table};
use spin::{Mutex, Once};
use x86_64::{
    PhysAddr,
    registers::control::{Cr2, Cr3, Cr3Flags},
    structures::paging::PhysFrame,
};

global_asm!(include_str!("task-entry.S"));
unsafe extern "C" {
    pub fn irq_entry();
    pub fn syscall_entry();
    pub fn page_fault_entry();
    pub fn protection_entry();
    pub fn invalid_entry();
    pub fn divide_entry();
    fn enter_user(frame: *const TrapFrame) -> !;
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TrapFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    vector: u64,
    error: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}
const _: () = assert!(size_of::<TrapFrame>() == 176);
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Receiving,
    Sleeping(u64),
    Dead,
}
struct Mailbox {
    messages: [Message; 16],
    len: usize,
}
impl Mailbox {
    fn new() -> Self {
        Self {
            messages: [Message {
                sender: 0,
                value: 0,
                tag: 0,
            }; 16],
            len: 0,
        }
    }
    fn push(&mut self, msg: Message) -> Result<(), ()> {
        if self.len == 16 {
            return Err(());
        }
        self.messages[self.len] = msg;
        self.len += 1;
        Ok(())
    }
    fn pop(&mut self) -> Option<Message> {
        if self.len == 0 {
            return None;
        }
        let msg = self.messages[0];
        self.messages.copy_within(1..self.len, 0);
        self.len -= 1;
        Some(msg)
    }
    fn remove_sender(&mut self, pid: u64) {
        let mut keep = 0;
        for i in 0..self.len {
            if self.messages[i].sender != pid {
                self.messages[keep] = self.messages[i];
                keep += 1;
            }
        }
        self.len = keep;
    }
}
struct Checkpoint {
    pages: Vec<UserPage>,
    frame: TrapFrame,
}
struct Task {
    memory: AddressSpace,
    frame: TrapFrame,
    state: State,
    caps: Table<16>,
    receive_cap: Handle,
    send_caps: [Option<Handle>; TASK_COUNT],
    mailbox: Mailbox,
    switches: u64,
    preemptions: u64,
    faults: u64,
    epoch: u64,
    checkpoint: Option<Checkpoint>,
}
struct Kernel {
    tasks: Vec<Task>,
    current: usize,
    frames: BootFrames<'static>,
    offset: u64,
}
static KERNEL: Once<Mutex<Kernel>> = Once::new();
const USER_IMAGE: &[u8] = include_bytes!(env!("MNEMOSYNE_USER_IMAGE"));

pub fn launch(offset: u64, mut frames: BootFrames<'static>) -> ! {
    let (code, data) = arch::user_selectors();
    let mut tasks = Vec::new();
    for pid in 1..=TASK_COUNT {
        let mut memory = AddressSpace::load(USER_IMAGE, pid, offset, &mut frames);
        let frame = TrapFrame {
            rip: memory.entry,
            rsp: STACK_TOP - 8,
            cs: code,
            ss: data,
            rflags: 0x202,
            rdi: pid as u64,
            ..TrapFrame::default()
        };
        let checkpoint = if pid == 4 {
            Some(Checkpoint {
                pages: memory.checkpoint(offset, &mut frames),
                frame,
            })
        } else {
            None
        };
        let mut caps = Table::new();
        caps.mint_root(
            1,
            ObjectKind::Device,
            if pid == 1 {
                Rights::READ | Rights::WRITE
            } else {
                Rights::WRITE
            },
        )
        .unwrap();
        let receive_cap = caps
            .mint_root(100 + pid as u64, ObjectKind::Endpoint, Rights::RECEIVE)
            .unwrap();
        tasks.push(Task {
            memory,
            frame,
            state: State::Ready,
            caps,
            receive_cap,
            send_caps: [None; TASK_COUNT],
            mailbox: Mailbox::new(),
            switches: 0,
            preemptions: 0,
            faults: 0,
            epoch: 1,
            checkpoint,
        });
    }
    // Static initial service policy: init may contact echo and fault services;
    // workers may reply to init, but no worker can read the console or disk.
    for (index, task) in tasks.iter_mut().enumerate() {
        for target in 0..TASK_COUNT {
            if (index == 0 && (target == 1 || target == 3)) || (index != 0 && target == 0) {
                task.send_caps[target] = Some(
                    task.caps
                        .mint_root(101 + target as u64, ObjectKind::Endpoint, Rights::SEND)
                        .unwrap(),
                );
            }
        }
        if index == 0 {
            task.caps
                .mint_root(
                    2,
                    ObjectKind::Device,
                    Rights::READ | Rights::WRITE | Rights::CONTROL,
                )
                .unwrap();
            for pid in [2, 4] {
                task.caps
                    .mint_root(pid, ObjectKind::Process, Rights::CONTROL)
                    .unwrap();
            }
        }
    }
    serialln!(
        "{{\"event\":\"userspace-ready\",\"tasks\":4,\"privilege\":3,\"scheduler\":\"round-robin-100hz\"}}"
    );
    KERNEL.call_once(|| {
        Mutex::new(Kernel {
            tasks,
            current: 0,
            frames,
            offset,
        })
    });
    arch::install_user_handlers();
    let ptr = {
        let kernel = KERNEL.get().unwrap().lock();
        unsafe {
            Cr3::write(
                PhysFrame::from_start_address(PhysAddr::new(kernel.tasks[0].memory.root)).unwrap(),
                Cr3Flags::empty(),
            );
        }
        &kernel.tasks[0].frame as *const TrapFrame
    };
    unsafe { enter_user(ptr) }
}

impl Kernel {
    fn authority(&self, kind: ObjectKind, object: u64, rights: Rights) -> bool {
        // Tables are fixed at 16 entries; only live handles with exact authority
        // count. Privileged syscalls never accept a caller-supplied process ID.
        (0..16).any(|slot| {
            self.tasks[self.current]
                .caps
                .resolve(
                    Handle {
                        slot,
                        generation: 1,
                    },
                    kind,
                    rights,
                )
                .is_ok_and(|c| c.object == object)
        })
    }
    fn write_user(&mut self, address: u64, data: &[u8]) -> i64 {
        match self.tasks[self.current]
            .memory
            .write(address, data, self.offset, &mut self.frames)
        {
            Ok(()) => 0,
            Err(()) => ERR_INVALID,
        }
    }
    fn control(&self, pid: u64) -> bool {
        self.authority(ObjectKind::Process, pid, Rights::CONTROL)
    }
    fn restart(&mut self, pid: usize) -> i64 {
        if pid != 2 && pid != 4 {
            return ERR_INVALID;
        }
        if !self.control(pid as u64) {
            return ERR_DENIED;
        }
        let task = &mut self.tasks[pid - 1];
        let Some(saved) = &task.checkpoint else {
            return ERR_INVALID;
        };
        task.memory
            .restore(&saved.pages, self.offset, &mut self.frames);
        task.frame = saved.frame;
        task.state = State::Ready;
        task.epoch += 1;
        task.mailbox.len = 0;
        // Revoke old endpoint handles before reissuing policy-approved authority.
        for other in &mut self.tasks {
            other.mailbox.remove_sender(pid as u64);
            other.caps.revoke_object(100 + pid as u64);
            if other.send_caps[pid - 1].is_some() {
                other.send_caps[pid - 1] = Some(
                    other
                        .caps
                        .mint_root(100 + pid as u64, ObjectKind::Endpoint, Rights::SEND)
                        .unwrap(),
                );
            }
        }
        // The receiver's original slot is reused with a new generation. Receive
        // resolves a handle stored by the kernel, never a fabricated generation.
        self.tasks[pid - 1].receive_cap = self.tasks[pid - 1]
            .caps
            .mint_root(100 + pid as u64, ObjectKind::Endpoint, Rights::RECEIVE)
            .unwrap();
        0
    }
    fn syscall(&mut self, f: &mut TrapFrame) -> bool {
        let mut schedule = false;
        let result = match f.rax {
            SYS_WRITE => {
                if !self.authority(ObjectKind::Device, 1, Rights::WRITE) {
                    ERR_DENIED
                } else if f.rsi > 4096 {
                    ERR_INVALID
                } else {
                    let mut bytes = [0; 4096];
                    let len = f.rsi as usize;
                    if self.tasks[self.current]
                        .memory
                        .read(f.rdi, &mut bytes[..len], self.offset)
                        .is_err()
                    {
                        ERR_INVALID
                    } else if let Ok(text) = core::str::from_utf8(&bytes[..len]) {
                        crate::serial::write(format_args!("{text}"));
                        len as i64
                    } else {
                        ERR_INVALID
                    }
                }
            }
            SYS_READ => {
                if self.authority(ObjectKind::Device, 1, Rights::READ) {
                    crate::serial::read_byte()
                        .map(i64::from)
                        .unwrap_or(ERR_AGAIN)
                } else {
                    ERR_DENIED
                }
            }
            SYS_YIELD => {
                schedule = true;
                0
            }
            SYS_SLEEP => {
                if f.rdi > 6000 {
                    ERR_INVALID
                } else {
                    self.tasks[self.current].state = State::Sleeping(arch::ticks() + f.rdi.max(1));
                    schedule = true;
                    0
                }
            }
            SYS_TICKS => arch::ticks() as i64,
            SYS_CAP => {
                if !(1..=TASK_COUNT as u64).contains(&f.rdi) {
                    ERR_INVALID
                } else if let Some(cap) = self.tasks[self.current].send_caps[f.rdi as usize - 1] {
                    f.rdx = cap.generation;
                    cap.slot as i64
                } else {
                    ERR_DENIED
                }
            }
            SYS_SEND => {
                let handle = Handle {
                    slot: f.rdi as u32,
                    generation: f.rsi,
                };
                if f.rdi > u32::MAX as u64 {
                    ERR_DENIED
                } else if let Ok(cap) = self.tasks[self.current].caps.resolve(
                    handle,
                    ObjectKind::Endpoint,
                    Rights::SEND,
                ) {
                    let index = (cap.object - 101) as usize;
                    let message = Message {
                        sender: self.current as u64 + 1,
                        value: f.rdx,
                        tag: f.r10,
                    };
                    if self.tasks[index].state == State::Dead {
                        ERR_INVALID
                    } else if self.tasks[index].mailbox.push(message).is_err() {
                        ERR_FULL
                    } else {
                        if self.tasks[index].state == State::Receiving {
                            self.tasks[index].state = State::Ready;
                        }
                        0
                    }
                } else {
                    ERR_DENIED
                }
            }
            SYS_RECEIVE => {
                // Each task has its own receive endpoint; it cannot select another task's queue.
                let task = &self.tasks[self.current];
                if task
                    .caps
                    .resolve(task.receive_cap, ObjectKind::Endpoint, Rights::RECEIVE)
                    .is_err()
                {
                    ERR_DENIED
                } else if !self.tasks[self.current]
                    .memory
                    .valid(f.rdi, size_of::<Message>(), true)
                {
                    ERR_INVALID
                } else if let Some(message) = self.tasks[self.current].mailbox.pop() {
                    let bytes = unsafe {
                        core::slice::from_raw_parts(
                            (&message as *const Message).cast::<u8>(),
                            size_of::<Message>(),
                        )
                    };
                    self.write_user(f.rdi, bytes)
                } else {
                    self.tasks[self.current].state = State::Receiving;
                    f.rip -= 2;
                    self.tasks[self.current].frame = *f;
                    return true; // Retry the same int 0x80 when a sender wakes us.
                }
            }
            SYS_TASKS => {
                if !self.authority(ObjectKind::Device, 1, Rights::READ) {
                    ERR_DENIED
                } else {
                    let stats: [TaskInfo; TASK_COUNT] = core::array::from_fn(|i| {
                        let t = &self.tasks[i];
                        TaskInfo {
                            pid: i as u64 + 1,
                            state: match t.state {
                                State::Ready => 0,
                                State::Receiving => 1,
                                State::Sleeping(_) => 2,
                                State::Dead => 3,
                            },
                            switches: t.switches,
                            preemptions: t.preemptions,
                            faults: t.faults,
                            epoch: t.epoch,
                        }
                    });
                    let bytes = unsafe {
                        core::slice::from_raw_parts(
                            stats.as_ptr().cast::<u8>(),
                            size_of_val(&stats),
                        )
                    };
                    self.write_user(f.rdi, bytes)
                }
            }
            SYS_DISK_READ | SYS_DISK_WRITE | SYS_DISK_FLUSH => {
                let rights = if f.rax == SYS_DISK_READ {
                    Rights::READ
                } else {
                    Rights::WRITE
                };
                if !self.authority(ObjectKind::Device, 2, rights) {
                    ERR_DENIED
                } else if f.rax == SYS_DISK_FLUSH {
                    crate::ata::flush().map(|_| 0).unwrap_or(ERR_IO)
                } else {
                    let mut sector = [0; 512];
                    if f.rax == SYS_DISK_READ {
                        if !self.tasks[self.current].memory.valid(f.rsi, 512, true) {
                            ERR_INVALID
                        } else if crate::ata::read(f.rdi, &mut sector).is_err() {
                            ERR_IO
                        } else {
                            self.write_user(f.rsi, &sector)
                        }
                    } else if self.tasks[self.current]
                        .memory
                        .read(f.rsi, &mut sector, self.offset)
                        .is_err()
                    {
                        ERR_INVALID
                    } else {
                        crate::ata::write(f.rdi, &sector)
                            .map(|_| 0)
                            .unwrap_or(ERR_IO)
                    }
                }
            }
            SYS_CHECKPOINT => {
                if f.rdi != 2 || !self.control(2) {
                    ERR_DENIED
                } else {
                    let t = &mut self.tasks[1];
                    if t.state != State::Receiving {
                        ERR_AGAIN
                    } else if t.checkpoint.is_some() {
                        ERR_FULL
                    } else {
                        t.checkpoint = Some(Checkpoint {
                            pages: t.memory.checkpoint(self.offset, &mut self.frames),
                            frame: t.frame,
                        });
                        0
                    }
                }
            }
            SYS_RESTART | SYS_RESTORE => self.restart(f.rdi as usize),
            SYS_COW_FAULTS => {
                if f.rdi == 2 && self.control(2) {
                    self.tasks[1].memory.cow_faults as i64
                } else {
                    ERR_DENIED
                }
            }
            SYS_SHUTDOWN => {
                if self.authority(ObjectKind::Device, 2, Rights::CONTROL) {
                    serialln!("{{\"event\":\"guest-shutdown\",\"status\":{}}}", f.rdi);
                    crate::exit_qemu(if f.rdi == 0 { 0x10 } else { 0x11 });
                }
                ERR_DENIED
            }
            SYS_EXIT => {
                if self.current == 0 {
                    crate::exit_qemu(0x11);
                }
                self.tasks[self.current].state = State::Dead;
                schedule = true;
                0
            }
            _ => ERR_INVALID,
        };
        f.rax = result as u64;
        self.tasks[self.current].frame = *f;
        schedule
    }
    fn next(&mut self) -> bool {
        let now = arch::ticks();
        for task in &mut self.tasks {
            if matches!(task.state, State::Sleeping(until) if until <= now) {
                task.state = State::Ready;
            }
        }
        for delta in 1..=self.tasks.len() {
            let index = (self.current + delta) % self.tasks.len();
            if self.tasks[index].state == State::Ready {
                self.current = index;
                self.tasks[index].switches += 1;
                return true;
            }
        }
        false
    }
}

#[unsafe(no_mangle)]
extern "C" fn task_trap(frame: &mut TrapFrame) -> *const TrapFrame {
    if frame.cs & 3 == 0 && frame.vector == 32 {
        // Only the explicit idle loop enables interrupts in kernel mode. It
        // holds no scheduler/allocator locks and resumes after this tick.
        arch::timer_tick();
        return frame;
    }
    assert_eq!(frame.cs & 3, 3, "unexpected kernel trap: {frame:?}");
    let mut k = KERNEL.get().unwrap().lock();
    let current = k.current;
    k.tasks[current].frame = *frame;
    let schedule = match frame.vector {
        32 => {
            arch::timer_tick();
            k.tasks[current].preemptions += 1;
            true
        }
        128 => k.syscall(frame),
        _ => {
            let Kernel {
                tasks,
                offset,
                frames,
                ..
            } = &mut *k;
            let task = &mut tasks[current];
            if frame.vector == 14
                && frame.error == 7
                && task
                    .memory
                    .copy_on_write(Cr2::read_raw(), *offset, frames)
                    .is_ok()
            {
                false
            } else {
                task.faults += 1;
                task.state = State::Dead;
                serialln!(
                    "{{\"event\":\"task-fault\",\"pid\":{},\"vector\":{},\"error\":{},\"rip\":{},\"address\":{}}}",
                    current + 1,
                    frame.vector,
                    frame.error,
                    frame.rip,
                    Cr2::read_raw()
                );
                assert_ne!(current, 0, "init service fault");
                true
            }
        }
    };
    if schedule {
        while !k.next() {
            drop(k);
            x86_64::instructions::interrupts::enable_and_hlt();
            x86_64::instructions::interrupts::disable();
            k = KERNEL.get().unwrap().lock();
        }
    }
    let task = &k.tasks[k.current];
    unsafe {
        Cr3::write(
            PhysFrame::from_start_address(PhysAddr::new(task.memory.root)).unwrap(),
            Cr3Flags::empty(),
        );
    }
    &task.frame
}
