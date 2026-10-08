//! Active-address-space smoke test using upstream x86_64 page-table machinery.
//! Process address spaces, frame reclamation and COW are subsequent work.

use crate::memory::{BootFrames, PAGE_SIZE};
use core::{
    arch::global_asm,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use x86_64::{
    VirtAddr,
    instructions::interrupts,
    registers::{
        control::{Cr0, Cr0Flags, Cr2, Cr3, Cr4, Cr4Flags},
        model_specific::{Efer, EferFlags},
    },
    structures::{
        idt::{InterruptStackFrame, PageFaultErrorCode},
        paging::{
            FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, Size4KiB,
            Translate, mapper::TranslateResult,
        },
    },
};

const TEST_ADDRESS: u64 = 0x0000_4000_0000_0000;

// These probes have explicit fault sites and a common return instruction. The
// page-fault handler only fixes up an armed probe with the exact RIP, CR2 and error.
// Unexpected faults still panic. This is test machinery, not general fault recovery.
global_asm!(
    ".global mnemosyne_probe_write",
    "mnemosyne_probe_write:",
    "mov byte ptr [rdi], 0xff",
    "ret",
    ".global mnemosyne_probe_read",
    "mnemosyne_probe_read:",
    "mov al, byte ptr [rdi]",
    "ret",
    ".global mnemosyne_probe_exec",
    "mnemosyne_probe_exec:",
    "sub rsp, 8",
    "call rdi",
    "add rsp, 8",
    "ret",
    ".global mnemosyne_probe_resume",
    "mnemosyne_probe_resume:",
    "ret",
);

unsafe extern "sysv64" {
    fn mnemosyne_probe_write(address: *const u8);
    fn mnemosyne_probe_read(address: *const u8);
    fn mnemosyne_probe_exec(address: *const u8);
    fn mnemosyne_probe_resume();
}

static ARMED: AtomicBool = AtomicBool::new(false);
static FAULT_RIP: AtomicU64 = AtomicU64::new(0);
static FAULT_ADDRESS: AtomicU64 = AtomicU64::new(0);
static FAULT_FLAGS: AtomicU64 = AtomicU64::new(0);
static FAULT_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn recover_probe(frame: &mut InterruptStackFrame, error: PageFaultErrorCode) -> bool {
    if !ARMED.load(Ordering::Acquire)
        || frame.instruction_pointer.as_u64() != FAULT_RIP.load(Ordering::Relaxed)
        || Cr2::read_raw() != FAULT_ADDRESS.load(Ordering::Relaxed)
        || error.bits() != FAULT_FLAGS.load(Ordering::Relaxed)
    {
        return false;
    }
    ARMED.store(false, Ordering::Release);
    // SAFETY: exact probe matching above identifies a controlled assembly call.
    // The resume instruction returns from that call with its stack intact. For
    // execute probes it returns to the instruction after `call rdi` first.
    unsafe {
        frame.as_mut().update(|saved| {
            saved.instruction_pointer = VirtAddr::from_ptr(mnemosyne_probe_resume as *const ());
        });
    }
    FAULT_COUNT.fetch_add(1, Ordering::Relaxed);
    true
}

unsafe fn expect_fault(
    probe: unsafe extern "sysv64" fn(*const u8),
    fault_rip: u64,
    address: VirtAddr,
    flags: PageFaultErrorCode,
) {
    assert!(
        !interrupts::are_enabled(),
        "fault probes require exclusive CPU access"
    );
    assert!(!ARMED.load(Ordering::Acquire));
    let previous = FAULT_COUNT.load(Ordering::Relaxed);
    FAULT_RIP.store(fault_rip, Ordering::Relaxed);
    FAULT_ADDRESS.store(address.as_u64(), Ordering::Relaxed);
    FAULT_FLAGS.store(flags.bits(), Ordering::Relaxed);
    ARMED.store(true, Ordering::Release);
    // SAFETY: caller selects one of the assembly probes for our reserved test page.
    unsafe {
        probe(address.as_ptr());
    }
    ARMED.store(false, Ordering::Release);
    assert_eq!(
        FAULT_COUNT.load(Ordering::Relaxed),
        previous + 1,
        "CPU did not enforce page permissions"
    );
}

/// # Safety
/// HHDM must map all RAM and active page tables. There must be one CPU and no
/// concurrent page-table users. `frames` must uniquely own its unused frames.
pub unsafe fn smoke(offset: u64, frames: &mut BootFrames<'_>) {
    assert!(!interrupts::are_enabled());
    assert!(
        !Cr4::read().contains(Cr4Flags::L5_PAGING),
        "only four-level paging is supported"
    );
    // SAFETY: CPUID is available in the x86-64 execution environment.
    let features = unsafe { core::arch::x86_64::__cpuid(0x8000_0001) };
    assert_ne!(features.edx & (1 << 20), 0, "NX support is required");
    // SAFETY: preserve all control bits and enable NX and supervisor write protection.
    unsafe {
        Efer::update(|flags| flags.insert(EferFlags::NO_EXECUTE_ENABLE));
        Cr0::update(|flags| flags.insert(Cr0Flags::WRITE_PROTECT));
    }

    let root_address = VirtAddr::new(offset + Cr3::read().0.start_address().as_u64());
    // SAFETY: unique access to the live PML4 and correct direct-map offset are
    // required by the caller. This mutable reference never escapes this function.
    let mut mapper = unsafe {
        let root = &mut *root_address.as_mut_ptr::<PageTable>();
        OffsetPageTable::new(root, VirtAddr::new(offset))
    };
    let address = VirtAddr::new(TEST_ADDRESS);
    let page = Page::<Size4KiB>::from_start_address(address).unwrap();
    assert!(matches!(
        mapper.translate(address),
        TranslateResult::NotMapped
    ));
    let frame = frames.allocate_frame().expect("paging test data frame");
    let direct = VirtAddr::new(offset + frame.start_address().as_u64()).as_mut_ptr::<u8>();
    // SAFETY: this new, exclusively owned RAM frame is mapped by HHDM.
    unsafe {
        direct.write_bytes(0, PAGE_SIZE as usize);
    }
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
    // SAFETY: the destination is unused, frame ownership is exclusive, and the
    // direct-map alias is only accessed through raw pointers by this test.
    unsafe {
        mapper
            .map_to(page, frame, flags, frames)
            .expect("map test page")
            .flush();
    }
    assert_eq!(mapper.translate_addr(address), Some(frame.start_address()));
    // SAFETY: the page is present and writable; first and last bytes lie within it.
    unsafe {
        address
            .as_mut_ptr::<u64>()
            .write_volatile(0x4d4e454d4f53594e);
        (address + PAGE_SIZE - 1u64)
            .as_mut_ptr::<u8>()
            .write_volatile(0x5a);
        assert_eq!(direct.cast::<u64>().read_volatile(), 0x4d4e454d4f53594e);
        assert_eq!(direct.add(PAGE_SIZE as usize - 1).read_volatile(), 0x5a);
        // If NX fails, this is a harmless `ret` and the missing fault fails the test.
        address.as_mut_ptr::<u8>().write_volatile(0xc3);
        expect_fault(
            mnemosyne_probe_exec,
            address.as_u64(),
            address,
            PageFaultErrorCode::PROTECTION_VIOLATION | PageFaultErrorCode::INSTRUCTION_FETCH,
        );
        mapper
            .update_flags(page, PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE)
            .expect("make page read only")
            .flush();
        expect_fault(
            mnemosyne_probe_write,
            mnemosyne_probe_write as *const () as u64,
            address,
            PageFaultErrorCode::PROTECTION_VIOLATION | PageFaultErrorCode::CAUSED_BY_WRITE,
        );
        assert_eq!(address.as_ptr::<u8>().read_volatile(), 0xc3);
    }
    let (removed, flush) = mapper.unmap(page).expect("unmap test page");
    flush.flush();
    assert_eq!(removed, frame);
    assert!(matches!(
        mapper.translate(address),
        TranslateResult::NotMapped
    ));
    // SAFETY: this is the controlled read probe for the page just unmapped. It
    // returns only through the exact fault fixup, or the test fails on missing fault.
    unsafe {
        expect_fault(
            mnemosyne_probe_read,
            mnemosyne_probe_read as *const () as u64,
            address,
            PageFaultErrorCode::empty(),
        );
    }
    // Boot allocator retains the data and page-table frames; no reclamation yet.
}
