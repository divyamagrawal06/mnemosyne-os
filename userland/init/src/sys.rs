use core::arch::asm;
use mnemosyne_abi::*;
pub fn call(no: u64, a: u64, b: u64, c: u64, d: u64) -> (i64, u64) {
    let result: u64;
    let extra: u64;
    // The trap ABI preserves all registers except RAX (status/value) and RDX
    // (extra return value). The kernel never borrows user pointers past a call.
    unsafe {
        asm!("int 0x80", inlateout("rax") no => result,
        in("rdi") a, in("rsi") b, inlateout("rdx") c => extra, in("r10") d);
    }
    (result as i64, extra)
}
pub fn ticks() -> u64 {
    call(SYS_TICKS, 0, 0, 0, 0).0 as u64
}
pub fn sleep(ticks: u64) {
    call(SYS_SLEEP, ticks, 0, 0, 0);
}
pub fn cap(pid: u64) -> (u64, u64) {
    let (slot, generation) = call(SYS_CAP, pid, 0, 0, 0);
    assert!(slot >= 0);
    (slot as u64, generation)
}
pub fn send(pid: u64, value: u64, tag: u64) -> i64 {
    let (slot, generation) = cap(pid);
    call(SYS_SEND, slot, generation, value, tag).0
}
pub fn receive() -> Message {
    let mut message = Message::default();
    assert_eq!(
        call(SYS_RECEIVE, (&mut message as *mut Message) as u64, 0, 0, 0).0,
        0
    );
    message
}
pub fn stats() -> [TaskInfo; TASK_COUNT] {
    let mut stats = [TaskInfo::default(); TASK_COUNT];
    assert_eq!(call(SYS_TASKS, stats.as_mut_ptr() as u64, 0, 0, 0).0, 0);
    stats
}
pub fn shutdown(status: u64) -> ! {
    call(SYS_SHUTDOWN, status, 0, 0, 0);
    loop {
        sleep(100);
    }
}
