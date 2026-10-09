use linked_list_allocator::LockedHeap;
#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();
#[repr(C, align(4096))]
struct HeapBytes([u8; 8 * 1024 * 1024]);
static mut BYTES: HeapBytes = HeapBytes([0; 8 * 1024 * 1024]);

pub fn initialize() {
    // SAFETY: this statically mapped region is handed to the allocator exactly
    // once before any allocation; kernel interrupts remain disabled.
    unsafe {
        HEAP.lock()
            .init((&raw mut BYTES).cast::<u8>(), 8 * 1024 * 1024);
    }
}
