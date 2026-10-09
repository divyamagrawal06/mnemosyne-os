//! Per-process lower-half page tables; the supervisor-only upper half is shared.
use crate::memory::BootFrames;
use alloc::vec::Vec;
use mnemosyne_abi::*;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageTable, PageTableFlags as Flags, PhysFrame, Size4KiB,
    },
};

pub const COW: Flags = Flags::BIT_9;
#[derive(Clone, Copy)]
pub struct UserPage {
    pub address: u64,
    pub physical: u64,
    pub flags: Flags,
}
pub struct AddressSpace {
    pub root: u64,
    pub pages: Vec<UserPage>,
    pub entry: u64,
    pub cow_faults: u64,
}

fn read64(data: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(data[at..at + 8].try_into().unwrap())
}
fn read32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}
fn read16(data: &[u8], at: usize) -> usize {
    u16::from_le_bytes(data[at..at + 2].try_into().unwrap()) as usize
}

impl AddressSpace {
    // All methods run on the sole CPU with interrupts disabled. Mapping routines
    // only modify the uniquely owned lower half, never shared supervisor tables.
    pub fn load(image: &[u8], pid: usize, offset: u64, frames: &mut BootFrames<'_>) -> Self {
        assert!(image.len() >= 64 && &image[..7] == b"\x7fELF\x02\x01\x01");
        assert_eq!(read16(image, 18), 62);
        let root = frames.allocate().expect("process PML4");
        unsafe {
            let dst = (offset + root) as *mut PageTable;
            dst.write(PageTable::new());
            let current = (offset + Cr3::read().0.start_address().as_u64()) as *const PageTable;
            for i in 256..512 {
                (&mut *dst)[i] = (&*current)[i].clone();
            }
        }
        let mut space = Self {
            root,
            pages: Vec::new(),
            entry: read64(image, 24),
            cow_faults: 0,
        };
        let phoff = usize::try_from(read64(image, 32)).unwrap();
        let phsize = read16(image, 54);
        let count = read16(image, 56);
        assert!(phsize == 56 && count <= 16 && phoff + phsize * count <= image.len());
        for i in 0..count {
            let at = phoff + i * phsize;
            if read32(image, at) != 1 {
                continue;
            }
            let flags = read32(image, at + 4);
            let file = usize::try_from(read64(image, at + 8)).unwrap();
            let address = read64(image, at + 16);
            let file_len = usize::try_from(read64(image, at + 32)).unwrap();
            let mem_len = read64(image, at + 40);
            assert!(
                file_len as u64 <= mem_len && file.checked_add(file_len).unwrap() <= image.len()
            );
            assert!(address >= 0x400000 && address.checked_add(mem_len).unwrap() < 0x1000_0000);
            assert_eq!(address % 4096, 0);
            assert_ne!(flags & 3, 3, "ELF writable executable segment");
            let mut perms = Flags::PRESENT | Flags::USER_ACCESSIBLE;
            if flags & 2 != 0 {
                perms |= Flags::WRITABLE;
            }
            if flags & 1 == 0 {
                perms |= Flags::NO_EXECUTE;
            }
            for page in (address..address + mem_len).step_by(4096) {
                space.allocate_page(page, perms, offset, frames);
            }
            // Populate through the supervisor HHDM while preserving user W^X.
            let mut cursor = 0;
            while cursor < file_len {
                let page = space.page(address + cursor as u64).unwrap();
                let n = (file_len - cursor).min(4096);
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        image[file + cursor..].as_ptr(),
                        (offset + page.physical) as *mut u8,
                        n,
                    );
                }
                cursor += n;
            }
        }
        let heap = if pid == 1 {
            INIT_HEAP_BYTES
        } else {
            WORKER_HEAP_BYTES
        };
        let rw = Flags::PRESENT | Flags::USER_ACCESSIBLE | Flags::WRITABLE | Flags::NO_EXECUTE;
        for address in (HEAP_START as u64..(HEAP_START + heap) as u64).step_by(4096) {
            space.allocate_page(address, rw, offset, frames);
        }
        // Pages below and above the stack remain unmapped guard pages.
        let stack_bytes = if pid == 1 {
            INIT_STACK_BYTES
        } else {
            STACK_BYTES
        };
        for address in (STACK_TOP - stack_bytes..STACK_TOP).step_by(4096) {
            space.allocate_page(address, rw, offset, frames);
        }
        assert!(
            space
                .page(space.entry)
                .is_some_and(|p| !p.flags.contains(Flags::NO_EXECUTE))
        );
        space
    }

    fn mapper<'a>(&mut self, offset: u64) -> OffsetPageTable<'a> {
        unsafe {
            OffsetPageTable::new(
                &mut *((offset + self.root) as *mut PageTable),
                VirtAddr::new(offset),
            )
        }
    }
    fn allocate_page(
        &mut self,
        address: u64,
        flags: Flags,
        offset: u64,
        frames: &mut BootFrames<'_>,
    ) {
        assert!(self.page(address).is_none(), "overlapping user segments");
        let physical = frames.allocate().expect("user page allocation");
        unsafe {
            ((offset + physical) as *mut u8).write_bytes(0, 4096);
        }
        let mut mapper = self.mapper(offset);
        unsafe {
            mapper
                .map_to(
                    Page::<Size4KiB>::from_start_address(VirtAddr::new(address)).unwrap(),
                    PhysFrame::from_start_address(PhysAddr::new(physical)).unwrap(),
                    flags,
                    frames,
                )
                .expect("map user page")
                .ignore();
        }
        self.pages.push(UserPage {
            address,
            physical,
            flags,
        });
    }
    pub fn page(&self, address: u64) -> Option<&UserPage> {
        self.pages
            .binary_search_by_key(&(address & !4095), |p| p.address)
            .ok()
            .map(|i| &self.pages[i])
    }
    pub fn valid(&self, address: u64, length: usize, writable: bool) -> bool {
        if length == 0 {
            return true;
        }
        let Some(end) = address.checked_add(length as u64 - 1) else {
            return false;
        };
        if end >= 0x0000_8000_0000_0000 {
            return false;
        }
        (address & !4095..=end & !4095).step_by(4096).all(|a| {
            self.page(a).is_some_and(|p| {
                p.flags.contains(Flags::USER_ACCESSIBLE)
                    && (!writable || p.flags.intersects(Flags::WRITABLE | COW))
            })
        })
    }
    pub fn read(&self, address: u64, out: &mut [u8], offset: u64) -> Result<(), ()> {
        if !self.valid(address, out.len(), false) {
            return Err(());
        }
        let mut done = 0;
        while done < out.len() {
            let at = address + done as u64;
            let page = self.page(at).unwrap();
            let off = (at & 4095) as usize;
            let n = (4096 - off).min(out.len() - done);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    (offset + page.physical + off as u64) as *const u8,
                    out[done..].as_mut_ptr(),
                    n,
                );
            }
            done += n;
        }
        Ok(())
    }
    pub fn write(
        &mut self,
        address: u64,
        bytes: &[u8],
        offset: u64,
        frames: &mut BootFrames<'_>,
    ) -> Result<(), ()> {
        if !self.valid(address, bytes.len(), true) {
            return Err(());
        }
        let mut done = 0;
        while done < bytes.len() {
            let at = address + done as u64;
            if self.page(at).unwrap().flags.contains(COW) {
                self.copy_on_write(at, offset, frames)?;
            }
            let page = self.page(at).unwrap();
            let off = (at & 4095) as usize;
            let n = (4096 - off).min(bytes.len() - done);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    bytes[done..].as_ptr(),
                    (offset + page.physical + off as u64) as *mut u8,
                    n,
                );
            }
            done += n;
        }
        Ok(())
    }
    fn replace(
        &mut self,
        index: usize,
        physical: u64,
        flags: Flags,
        offset: u64,
        frames: &mut BootFrames<'_>,
    ) {
        let address = self.pages[index].address;
        let mut mapper = self.mapper(offset);
        let page = Page::<Size4KiB>::from_start_address(VirtAddr::new(address)).unwrap();
        mapper.unmap(page).unwrap().1.flush();
        unsafe {
            mapper
                .map_to(
                    page,
                    PhysFrame::from_start_address(PhysAddr::new(physical)).unwrap(),
                    flags,
                    frames,
                )
                .unwrap()
                .flush();
        }
        self.pages[index].physical = physical;
        self.pages[index].flags = flags;
    }
    pub fn copy_on_write(
        &mut self,
        address: u64,
        offset: u64,
        frames: &mut BootFrames<'_>,
    ) -> Result<(), ()> {
        let index = self
            .pages
            .iter()
            .position(|p| p.address == address & !4095)
            .ok_or(())?;
        let old = self.pages[index];
        if !old.flags.contains(COW) {
            return Err(());
        }
        let physical = frames.allocate().ok_or(())?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                (offset + old.physical) as *const u8,
                (offset + physical) as *mut u8,
                4096,
            );
        }
        self.replace(
            index,
            physical,
            (old.flags | Flags::WRITABLE) & !COW,
            offset,
            frames,
        );
        self.cow_faults += 1;
        Ok(())
    }
    pub fn checkpoint(&mut self, offset: u64, frames: &mut BootFrames<'_>) -> Vec<UserPage> {
        let snapshot = self.pages.clone();
        for i in 0..self.pages.len() {
            let p = self.pages[i];
            if p.flags.contains(Flags::WRITABLE) {
                self.replace(
                    i,
                    p.physical,
                    (p.flags & !Flags::WRITABLE) | COW,
                    offset,
                    frames,
                );
            }
        }
        snapshot
    }
    pub fn restore(&mut self, snapshot: &[UserPage], offset: u64, frames: &mut BootFrames<'_>) {
        assert_eq!(self.pages.len(), snapshot.len());
        for (i, saved) in snapshot.iter().enumerate() {
            let old = self.pages[i];
            assert_eq!(old.address, saved.address);
            let flags = if saved.flags.contains(Flags::WRITABLE) {
                (saved.flags & !Flags::WRITABLE) | COW
            } else {
                saved.flags
            };
            self.replace(i, saved.physical, flags, offset, frames);
            if old.physical != saved.physical {
                unsafe {
                    frames.release(old.physical);
                }
            }
        }
    }
}
