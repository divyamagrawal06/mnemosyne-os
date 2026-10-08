// Boot request layout adapted from the Limine Rust template (0BSD).
// See THIRD_PARTY.md for the pinned source and retained notice.
#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod arch;
mod memory;
mod serial;

use limine::{
    BaseRevision,
    memory_map::EntryType,
    request::{HhdmRequest, MemoryMapRequest, RequestsEndMarker, RequestsStartMarker},
};

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static START: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests")]
static BASE: BaseRevision = BaseRevision::new();
#[used]
#[unsafe(link_section = ".requests")]
static MEMORY: MemoryMapRequest = MemoryMapRequest::new();
#[used]
#[unsafe(link_section = ".requests")]
static HHDM: HhdmRequest = HhdmRequest::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static END: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    serialln!("{{\"event\":\"boot\",\"kernel\":\"mnemosyne\",\"abi\":1}}");
    assert!(BASE.is_supported(), "unsupported Limine base revision");
    let entries = MEMORY
        .get_response()
        .expect("memory map required")
        .entries();
    let offset = HHDM.get_response().expect("direct map required").offset();
    let usable: u64 = entries
        .iter()
        .filter(|e| e.entry_type == EntryType::USABLE)
        .map(|e| e.length)
        .sum();
    serialln!(
        "{{\"event\":\"memory\",\"regions\":{},\"usable_bytes\":{}}}",
        entries.len(),
        usable
    );
    let mut frames = memory::BootFrames::new(entries);
    let first = frames.allocate().expect("first usable frame");
    let second = frames.allocate().expect("second usable frame");
    assert_ne!(first, second);
    assert_eq!(first % memory::PAGE_SIZE, 0);
    // SAFETY: the allocator exclusively hands out USABLE frames; Limine maps them
    // at HHDM. These two frames are retained for this boot and never handed out again.
    unsafe {
        let a = (offset + first) as *mut u64;
        let b = (offset + second) as *mut u64;
        a.write_volatile(0x4d4e454d4f53594e);
        b.write_volatile(0x4348524f4e49434c);
        assert_eq!(a.read_volatile(), 0x4d4e454d4f53594e);
        assert_eq!(b.read_volatile(), 0x4348524f4e49434c);
    }
    serialln!("{{\"event\":\"frames_ok\",\"count\":2}}");
    arch::initialize();
    arch::breakpoint_smoke();
    serialln!("{{\"event\":\"exception_ok\",\"vector\":3}}");
    let ticks = arch::timer_smoke();
    serialln!("{{\"event\":\"timer_ok\",\"ticks\":{ticks}}}");
    serialln!("{{\"event\":\"boot_ok\"}}");
    exit_qemu(0x10)
}

fn exit_qemu(code: u32) -> ! {
    x86_64::instructions::interrupts::disable();
    // SAFETY: the smoke-test machine reserves this port for isa-debug-exit.
    unsafe {
        x86_64::instructions::port::Port::<u32>::new(0xf4).write(code);
    }
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    x86_64::instructions::interrupts::disable();
    serial::panic_write(format_args!("PANIC: {info}\n"));
    exit_qemu(0x11)
}
