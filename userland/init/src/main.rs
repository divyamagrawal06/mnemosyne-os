#![no_std]
#![no_main]
extern crate alloc;
mod shell;
mod sys;
use core::fmt::Write;
use linked_list_allocator::LockedHeap;
use mnemosyne_abi::*;
#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();
struct Console;
impl Write for Console {
    fn write_str(&mut self, mut text: &str) -> core::fmt::Result {
        while !text.is_empty() {
            let mut n = text.len().min(4096);
            while !text.is_char_boundary(n) {
                n -= 1;
            }
            if sys::call(SYS_WRITE, text.as_ptr() as u64, n as u64, 0, 0).0 < 0 {
                return Err(core::fmt::Error);
            }
            text = &text[n..];
        }
        Ok(())
    }
}
macro_rules! println { ($($args:tt)*) => {{ let _ = core::fmt::Write::write_fmt(&mut crate::Console, format_args!("{}\n", format_args!($($args)*))); }}; }
pub(crate) use println;

#[unsafe(no_mangle)]
extern "C" fn user_start(pid: u64) -> ! {
    unsafe {
        HEAP.lock().init(
            HEAP_START as *mut u8,
            if pid == 1 {
                INIT_HEAP_BYTES
            } else {
                WORKER_HEAP_BYTES
            },
        );
    }
    match pid {
        1 => shell::run(),
        2 => echo_service(),
        3 => loop {
            core::hint::spin_loop();
        },
        4 => fault_service(),
        _ => panic!("unknown role"),
    }
}
fn echo_service() -> ! {
    let mut counter = 0u64;
    loop {
        let message = sys::receive();
        counter = counter.wrapping_add(message.value);
        assert_eq!(sys::send(1, counter, message.tag), 0);
    }
}
fn fault_service() -> ! {
    assert_eq!(sys::call(SYS_READ, 0, 0, 0, 0).0, ERR_DENIED);
    assert_eq!(sys::call(SYS_DISK_FLUSH, 0, 0, 0, 0).0, ERR_DENIED);
    loop {
        let message = sys::receive();
        unsafe {
            match message.value {
                1 => {
                    core::arch::asm!("mov al, byte ptr [rdi]", in("rdi") 0xffffffff80000000u64, out("al") _);
                }
                2 => core::arch::asm!("ud2"),
                3 => core::arch::asm!("cli"),
                _ => {
                    core::arch::asm!("xor rax, rax", "mov al, byte ptr [rax]", out("rax") _);
                }
            }
        }
    }
}
pub(crate) fn read_line() -> alloc::string::String {
    let mut line = alloc::string::String::new();
    loop {
        let byte = sys::call(SYS_READ, 0, 0, 0, 0).0;
        if byte < 0 {
            sys::sleep(1);
            continue;
        }
        match byte as u8 {
            b'\r' | b'\n' => {
                println!("");
                return line;
            }
            8 | 127 => {
                if line.pop().is_some() {
                    let _ = write!(Console, "\x08 \x08");
                }
            }
            b if (32..127).contains(&b) && line.len() < 512 => {
                line.push(b as char);
                let _ = write!(Console, "{}", b as char);
            }
            _ => (),
        }
    }
}
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    println!("user-panic: {info}");
    sys::call(SYS_EXIT, 1, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}
