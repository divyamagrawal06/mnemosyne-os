use core::sync::atomic::{AtomicU64, Ordering};
use pic8259::ChainedPics;
use spin::{LazyLock as Lazy, Mutex};
use x86_64::VirtAddr;
use x86_64::instructions::{
    port::Port,
    segmentation::{CS, DS, ES, SS, Segment},
    tables::load_tss,
};
use x86_64::structures::{
    gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
    idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
    tss::TaskStateSegment,
};

const IRQ_BASE: u8 = 32;
const DOUBLE_FAULT_IST: u16 = 0;
static TICKS: AtomicU64 = AtomicU64::new(0);
static BREAKPOINTS: AtomicU64 = AtomicU64::new(0);
static mut DOUBLE_FAULT_STACK: [u8; 20 * 1024] = [0; 20 * 1024];
static mut SYSCALL_STACK: [u8; 64 * 1024] = [0; 64 * 1024];

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    let start = VirtAddr::from_ptr(&raw const DOUBLE_FAULT_STACK);
    tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = start + 20 * 1024u64;
    tss.privilege_stack_table[0] = VirtAddr::from_ptr(&raw const SYSCALL_STACK) + 64 * 1024u64;
    tss
});

struct Selectors {
    code: SegmentSelector,
    data: SegmentSelector,
    tss: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
}
static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut table = GlobalDescriptorTable::new();
    let code = table.append(Descriptor::kernel_code_segment());
    let data = table.append(Descriptor::kernel_data_segment());
    let user_code = table.append(Descriptor::user_code_segment());
    let user_data = table.append(Descriptor::user_data_segment());
    let tss = table.append(Descriptor::tss_segment(&TSS));
    (
        table,
        Selectors {
            code,
            data,
            tss,
            user_code,
            user_data,
        },
    )
});

// SAFETY: PIC I/O ports are owned exclusively by this module.
static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(IRQ_BASE, IRQ_BASE + 8) });
static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut table = InterruptDescriptorTable::new();
    table.breakpoint.set_handler_fn(breakpoint);
    table.page_fault.set_handler_fn(page_fault);
    table
        .general_protection_fault
        .set_handler_fn(protection_fault);
    table.invalid_opcode.set_handler_fn(invalid_opcode);
    // SAFETY: GDT loads this TSS before IDT installation; IST 0 has a dedicated stack.
    unsafe {
        table
            .double_fault
            .set_handler_fn(double_fault)
            .set_stack_index(DOUBLE_FAULT_IST);
    }
    table[IRQ_BASE].set_handler_fn(timer);
    table
});

pub fn initialize() {
    x86_64::instructions::interrupts::disable();
    GDT.0.load();
    // SAFETY: selectors refer to live descriptors in our static loaded GDT.
    unsafe {
        CS::set_reg(GDT.1.code);
        DS::set_reg(GDT.1.data);
        ES::set_reg(GDT.1.data);
        SS::set_reg(GDT.1.data);
        load_tss(GDT.1.tss);
    }
    IDT.load();
    // SAFETY: interrupts are disabled and this module owns the legacy PIC/PIT.
    unsafe {
        PICS.lock().initialize();
        PICS.lock().write_masks(0xfe, 0xff); // Only IRQ0 (timer).
        let divisor: u16 = 11_932; // Approximately 100 Hz.
        Port::<u8>::new(0x43).write(0x36);
        Port::<u8>::new(0x40).write(divisor as u8);
        Port::<u8>::new(0x40).write((divisor >> 8) as u8);
    }
}

pub fn breakpoint_smoke() {
    x86_64::instructions::interrupts::int3();
    assert_eq!(BREAKPOINTS.load(Ordering::Relaxed), 1);
}

pub fn timer_smoke() -> u64 {
    while TICKS.load(Ordering::Relaxed) < 5 {
        x86_64::instructions::interrupts::enable_and_hlt();
    }
    x86_64::instructions::interrupts::disable();
    TICKS.load(Ordering::Relaxed)
}

extern "x86-interrupt" fn breakpoint(_frame: InterruptStackFrame) {
    BREAKPOINTS.fetch_add(1, Ordering::Relaxed);
}

extern "x86-interrupt" fn timer(_frame: InterruptStackFrame) {
    timer_tick();
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
pub fn user_selectors() -> (u64, u64) {
    (GDT.1.user_code.0 as u64, GDT.1.user_data.0 as u64)
}
pub fn timer_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: this is the handler for IRQ0; the matching PIC receives its EOI.
    unsafe {
        PICS.lock().notify_end_of_interrupt(IRQ_BASE);
    }
}

static USER_IDT: spin::Once<InterruptDescriptorTable> = spin::Once::new();
pub fn install_user_handlers() {
    use crate::tasks::{
        divide_entry, invalid_entry, irq_entry, page_fault_entry, protection_entry, syscall_entry,
    };
    let table = USER_IDT.call_once(|| {
        let mut table = InterruptDescriptorTable::new();
        unsafe {
            table[32].set_handler_addr(VirtAddr::from_ptr(irq_entry as *const ()));
            table[128]
                .set_handler_addr(VirtAddr::from_ptr(syscall_entry as *const ()))
                .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
            table
                .page_fault
                .set_handler_addr(VirtAddr::from_ptr(page_fault_entry as *const ()));
            table
                .general_protection_fault
                .set_handler_addr(VirtAddr::from_ptr(protection_entry as *const ()));
            table
                .invalid_opcode
                .set_handler_addr(VirtAddr::from_ptr(invalid_entry as *const ()));
            table
                .divide_error
                .set_handler_addr(VirtAddr::from_ptr(divide_entry as *const ()));
            table
                .double_fault
                .set_handler_fn(double_fault)
                .set_stack_index(DOUBLE_FAULT_IST);
        }
        table
    });
    table.load();
}

extern "x86-interrupt" fn page_fault(mut frame: InterruptStackFrame, error: PageFaultErrorCode) {
    if crate::paging::recover_probe(&mut frame, error) {
        return;
    }
    panic!("page fault: {error:?} {frame:?}");
}

extern "x86-interrupt" fn protection_fault(frame: InterruptStackFrame, error: u64) {
    panic!("general protection fault: {error} {frame:?}");
}

extern "x86-interrupt" fn invalid_opcode(frame: InterruptStackFrame) {
    panic!("invalid opcode: {frame:?}");
}

extern "x86-interrupt" fn double_fault(frame: InterruptStackFrame, error: u64) -> ! {
    panic!("double fault: {error} {frame:?}");
}
