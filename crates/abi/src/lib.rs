#![no_std]

pub const SYS_WRITE: u64 = 0;
pub const SYS_READ: u64 = 1;
pub const SYS_YIELD: u64 = 2;
pub const SYS_SLEEP: u64 = 3;
pub const SYS_TICKS: u64 = 4;
pub const SYS_SEND: u64 = 5;
pub const SYS_RECEIVE: u64 = 6;
pub const SYS_TASKS: u64 = 7;
pub const SYS_RESTART: u64 = 8;
pub const SYS_DISK_READ: u64 = 9;
pub const SYS_DISK_WRITE: u64 = 10;
pub const SYS_DISK_FLUSH: u64 = 11;
pub const SYS_SHUTDOWN: u64 = 12;
pub const SYS_CAP: u64 = 13;
pub const SYS_CHECKPOINT: u64 = 14;
pub const SYS_RESTORE: u64 = 15;
pub const SYS_EXIT: u64 = 16;
pub const SYS_COW_FAULTS: u64 = 17;
pub const ERR_DENIED: i64 = -1;
pub const ERR_INVALID: i64 = -2;
pub const ERR_FULL: i64 = -3;
pub const ERR_AGAIN: i64 = -4;
pub const ERR_IO: i64 = -5;
pub const TASK_COUNT: usize = 4;
pub const HEAP_START: usize = 0x4000_0000;
pub const INIT_HEAP_BYTES: usize = 24 * 1024 * 1024;
pub const WORKER_HEAP_BYTES: usize = 64 * 1024;
pub const STACK_TOP: u64 = 0x7000_0000;
pub const STACK_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Message { pub sender: u64, pub value: u64, pub tag: u64 }
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TaskInfo {
    pub pid: u64, pub state: u64, pub switches: u64, pub preemptions: u64,
    pub faults: u64, pub epoch: u64,
}
