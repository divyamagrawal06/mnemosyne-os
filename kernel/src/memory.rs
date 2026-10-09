use limine::memory_map::{Entry, EntryType};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

pub const PAGE_SIZE: u64 = 4096;

/// Boot-only frame allocator. Never reuses frames or touches reserved regions.
/// Replaced by a reclaiming allocator before dynamic process teardown is added.
pub struct BootFrames<'a> {
    entries: &'a [&'a Entry],
    region: usize,
    next: u64,
    free: alloc::vec::Vec<u64>,
}

impl<'a> BootFrames<'a> {
    /// # Safety
    /// Usable entries must be disjoint, unused RAM. This must be the only allocator
    /// for those entries; creating a second allocator would hand out aliased frames.
    pub unsafe fn new(entries: &'a [&'a Entry]) -> Self {
        Self {
            entries,
            region: 0,
            next: 0,
            free: alloc::vec::Vec::new(),
        }
    }

    pub fn allocate(&mut self) -> Option<u64> {
        if let Some(frame) = self.free.pop() {
            return Some(frame);
        }
        while let Some(entry) = self.entries.get(self.region) {
            if entry.entry_type == EntryType::USABLE {
                let start = entry.base.checked_add(PAGE_SIZE - 1)? & !(PAGE_SIZE - 1);
                let end = entry.base.checked_add(entry.length)? & !(PAGE_SIZE - 1);
                self.next = self.next.max(start).max(PAGE_SIZE);
                if self.next < end {
                    let frame = self.next;
                    self.next = self.next.checked_add(PAGE_SIZE)?;
                    return Some(frame);
                }
            }
            self.region += 1;
            self.next = 0;
        }
        None
    }

    /// # Safety
    /// The frame must have come from this allocator and have no remaining
    /// mappings or checkpoint references. Each frame may be returned only once.
    pub unsafe fn release(&mut self, frame: u64) {
        self.free.push(frame);
    }
}

// SAFETY: construction requires unique ownership of disjoint usable regions;
// allocation advances monotonically within each region and never reuses a frame.
unsafe impl FrameAllocator<Size4KiB> for BootFrames<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate().map(|address| {
            PhysFrame::from_start_address(PhysAddr::new(address)).expect("aligned boot frame")
        })
    }
}
