use limine::memory_map::{Entry, EntryType};

pub const PAGE_SIZE: u64 = 4096;

/// Boot-only frame allocator. Never reuses frames or touches reserved regions.
/// Replaced by a reclaiming allocator before dynamic process teardown is added.
pub struct BootFrames<'a> {
    entries: &'a [&'a Entry],
    region: usize,
    next: u64,
}

impl<'a> BootFrames<'a> {
    pub fn new(entries: &'a [&'a Entry]) -> Self {
        Self {
            entries,
            region: 0,
            next: 0,
        }
    }

    pub fn allocate(&mut self) -> Option<u64> {
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
}
