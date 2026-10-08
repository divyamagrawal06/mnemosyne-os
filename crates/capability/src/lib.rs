//! Bounded, allocation-free capability tables for trusted kernel code.
//!
//! Handles are local to a table: possession of their integer representation in
//! another process conveys no authority. The kernel must select the caller's
//! table; `mint_root` and `revoke_object` are privileged object-manager operations.
//! This implements table semantics, not a syscall boundary or object registry.
#![no_std]
#![forbid(unsafe_code)]

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rights: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const SEND = 1 << 2;
        const RECEIVE = 1 << 3;
        const DELEGATE = 1 << 4;
        const REVOKE = 1 << 5;
        const CONTROL = 1 << 6;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Memory,
    Endpoint,
    Process,
}

impl ObjectKind {
    fn allowed_rights(self) -> Rights {
        let common = Rights::DELEGATE | Rights::REVOKE;
        common
            | match self {
                Self::Memory => Rights::READ | Rights::WRITE,
                Self::Endpoint => Rights::SEND | Rights::RECEIVE,
                Self::Process => Rights::CONTROL,
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    pub slot: u32,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub object: u64,
    pub kind: ObjectKind,
    pub rights: Rights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidHandle,
    InvalidRights,
    WrongType,
    Denied,
    Full,
}

#[derive(Clone, Copy)]
struct Record {
    capability: Capability,
    parent: Option<Handle>,
}

#[derive(Clone, Copy)]
struct Slot {
    generation: u64,
    retired: bool,
    record: Option<Record>,
}

const EMPTY: Slot = Slot {
    generation: 1,
    retired: false,
    record: None,
};

pub struct Table<const N: usize> {
    slots: [Slot; N],
}

impl<const N: usize> Default for Table<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Table<N> {
    pub const fn new() -> Self {
        assert!(N <= u32::MAX as usize);
        Self { slots: [EMPTY; N] }
    }

    /// Trusted object-manager operation. Object IDs must not be recycled while
    /// references remain; revoke every referencing table before object reuse.
    pub fn mint_root(
        &mut self,
        object: u64,
        kind: ObjectKind,
        rights: Rights,
    ) -> Result<Handle, Error> {
        if !kind.allowed_rights().contains(rights) {
            return Err(Error::InvalidRights);
        }
        self.insert(Record {
            capability: Capability {
                object,
                kind,
                rights,
            },
            parent: None,
        })
    }

    pub fn resolve(
        &self,
        handle: Handle,
        kind: ObjectKind,
        required: Rights,
    ) -> Result<Capability, Error> {
        if !kind.allowed_rights().contains(required) {
            return Err(Error::InvalidRights);
        }
        let capability = self.record(handle)?.capability;
        if capability.kind != kind {
            return Err(Error::WrongType);
        }
        if !capability.rights.contains(required) {
            return Err(Error::Denied);
        }
        Ok(capability)
    }

    pub fn delegate(&mut self, parent: Handle, rights: Rights) -> Result<Handle, Error> {
        let capability = self.record(parent)?.capability;
        if !capability.rights.contains(Rights::DELEGATE) || !capability.rights.contains(rights) {
            return Err(Error::Denied);
        }
        self.insert(Record {
            capability: Capability {
                rights,
                ..capability
            },
            parent: Some(parent),
        })
    }

    /// Keep the invoking capability but remove its entire derivation subtree.
    /// Runs synchronously in O(N^2); callers must bound N and serialize access.
    pub fn revoke_descendants(&mut self, handle: Handle) -> Result<usize, Error> {
        if !self
            .record(handle)?
            .capability
            .rights
            .contains(Rights::REVOKE)
        {
            return Err(Error::Denied);
        }
        Ok(self.remove_tree(handle, false))
    }

    /// Delete an owned capability and its descendants. No dangling parent links.
    pub fn delete_tree(&mut self, handle: Handle) -> Result<usize, Error> {
        self.record(handle)?;
        Ok(self.remove_tree(handle, true))
    }

    /// Trusted object teardown. Call this on every table that may reference it.
    pub fn revoke_object(&mut self, object: u64) -> usize {
        let mut count = 0;
        for slot in &mut self.slots {
            if slot.record.is_some_and(|r| r.capability.object == object) {
                Self::invalidate(slot);
                count += 1;
            }
        }
        count
    }

    fn insert(&mut self, record: Record) -> Result<Handle, Error> {
        let (index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.record.is_none() && !s.retired)
            .ok_or(Error::Full)?;
        slot.record = Some(record);
        Ok(Handle {
            slot: index as u32,
            generation: slot.generation,
        })
    }

    fn record(&self, handle: Handle) -> Result<Record, Error> {
        let slot = self
            .slots
            .get(handle.slot as usize)
            .ok_or(Error::InvalidHandle)?;
        if slot.generation != handle.generation {
            return Err(Error::InvalidHandle);
        }
        slot.record.ok_or(Error::InvalidHandle)
    }

    fn descendant(&self, index: usize, ancestor: Handle) -> bool {
        let mut cursor = self.slots[index].record.and_then(|r| r.parent);
        for _ in 0..N {
            match cursor {
                Some(parent) if parent == ancestor => return true,
                Some(parent) => cursor = self.record(parent).ok().and_then(|r| r.parent),
                None => return false,
            }
        }
        false
    }

    fn remove_tree(&mut self, handle: Handle, include_root: bool) -> usize {
        // Compute membership before deleting parents, so grandchildren are found.
        let selected: [bool; N] = core::array::from_fn(|i| {
            (include_root && i == handle.slot as usize) || self.descendant(i, handle)
        });
        let mut count = 0;
        for (slot, selected) in self.slots.iter_mut().zip(selected) {
            if selected {
                Self::invalidate(slot);
                count += 1;
            }
        }
        count
    }

    fn invalidate(slot: &mut Slot) {
        slot.record = None;
        match slot.generation.checked_add(1) {
            Some(next) => slot.generation = next,
            None => slot.retired = true, // Exhausted identity is never reused.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MEMORY: ObjectKind = ObjectKind::Memory;
    fn memory_rights() -> Rights {
        Rights::READ | Rights::WRITE | Rights::DELEGATE | Rights::REVOKE
    }

    #[test]
    fn delegation_attenuates_and_rejects_amplification() {
        let mut table = Table::<4>::new();
        let parent = table
            .mint_root(7, MEMORY, Rights::READ | Rights::DELEGATE)
            .unwrap();
        let child = table.delegate(parent, Rights::READ).unwrap();
        assert_eq!(
            table.resolve(child, MEMORY, Rights::READ).unwrap().object,
            7
        );
        assert_eq!(
            table.resolve(child, MEMORY, Rights::WRITE),
            Err(Error::Denied)
        );
        assert_eq!(table.delegate(parent, Rights::WRITE), Err(Error::Denied));
        assert_eq!(table.delegate(child, Rights::READ), Err(Error::Denied));
    }

    #[test]
    fn subtree_revocation_preserves_parent_and_sibling() {
        let mut table = Table::<8>::new();
        let root = table.mint_root(7, MEMORY, memory_rights()).unwrap();
        let a = table.delegate(root, memory_rights()).unwrap();
        let b = table.delegate(root, Rights::READ).unwrap();
        let c = table.delegate(a, memory_rights()).unwrap();
        let d = table.delegate(c, Rights::READ).unwrap();
        assert_eq!(table.revoke_descendants(a), Ok(2));
        for handle in [root, a, b] {
            assert!(table.resolve(handle, MEMORY, Rights::READ).is_ok());
        }
        for handle in [c, d] {
            assert_eq!(
                table.resolve(handle, MEMORY, Rights::READ),
                Err(Error::InvalidHandle)
            );
        }
        assert_eq!(table.revoke_descendants(b), Err(Error::Denied));
    }

    #[test]
    fn deletion_removes_descendants_and_stale_handles_never_alias_reuse() {
        let mut table = Table::<3>::new();
        let root = table.mint_root(1, MEMORY, memory_rights()).unwrap();
        let child = table.delegate(root, Rights::READ).unwrap();
        assert_eq!(table.delete_tree(root), Ok(2));
        let replacement = table.mint_root(2, MEMORY, Rights::READ).unwrap();
        assert_eq!(replacement.slot, root.slot);
        assert_ne!(replacement.generation, root.generation);
        for handle in [root, child] {
            assert_eq!(
                table.resolve(handle, MEMORY, Rights::READ),
                Err(Error::InvalidHandle)
            );
        }
    }

    #[test]
    fn exhausted_generation_retires_the_slot() {
        let mut table = Table::<1>::new();
        table.slots[0].generation = u64::MAX;
        let handle = table.mint_root(1, MEMORY, Rights::READ).unwrap();
        table.delete_tree(handle).unwrap();
        assert_eq!(table.mint_root(2, MEMORY, Rights::READ), Err(Error::Full));
        assert_eq!(
            table.resolve(handle, MEMORY, Rights::READ),
            Err(Error::InvalidHandle)
        );
    }

    #[test]
    fn forged_handle_wrong_type_and_unknown_rights_are_rejected() {
        let mut table = Table::<2>::new();
        let handle = table.mint_root(1, MEMORY, Rights::READ).unwrap();
        assert_eq!(
            table.resolve(
                Handle {
                    slot: u32::MAX,
                    generation: 1
                },
                MEMORY,
                Rights::READ
            ),
            Err(Error::InvalidHandle)
        );
        assert_eq!(
            table.resolve(
                Handle {
                    generation: 0,
                    ..handle
                },
                MEMORY,
                Rights::READ
            ),
            Err(Error::InvalidHandle)
        );
        assert_eq!(
            table.resolve(handle, ObjectKind::Endpoint, Rights::SEND),
            Err(Error::WrongType)
        );
        assert_eq!(
            table.mint_root(2, MEMORY, Rights::SEND),
            Err(Error::InvalidRights)
        );
        assert_eq!(
            table.resolve(handle, MEMORY, Rights::from_bits_retain(1 << 31)),
            Err(Error::InvalidRights)
        );
    }

    #[test]
    fn capacity_failure_does_not_change_existing_authority() {
        let mut table = Table::<1>::new();
        let handle = table.mint_root(1, MEMORY, memory_rights()).unwrap();
        assert_eq!(table.delegate(handle, Rights::READ), Err(Error::Full));
        assert!(table.resolve(handle, MEMORY, Rights::WRITE).is_ok());
        assert_eq!(
            Table::<0>::new().mint_root(1, MEMORY, Rights::READ),
            Err(Error::Full)
        );
    }

    #[test]
    fn object_teardown_invalidates_independent_roots() {
        let mut table = Table::<4>::new();
        let a = table.mint_root(1, MEMORY, memory_rights()).unwrap();
        let b = table.mint_root(1, MEMORY, Rights::READ).unwrap();
        let c = table.mint_root(2, MEMORY, Rights::READ).unwrap();
        assert_eq!(table.revoke_object(1), 2);
        for handle in [a, b] {
            assert_eq!(
                table.resolve(handle, MEMORY, Rights::READ),
                Err(Error::InvalidHandle)
            );
        }
        assert!(table.resolve(c, MEMORY, Rights::READ).is_ok());
    }

    #[test]
    fn exhaustive_rights_subsets_never_amplify() {
        for parent_bits in 0..=Rights::all().bits() {
            let parent_rights = Rights::from_bits_retain(parent_bits);
            if !MEMORY.allowed_rights().contains(parent_rights) {
                continue;
            }
            for child_bits in 0..=Rights::all().bits() {
                let mut table = Table::<2>::new();
                let parent = table.mint_root(1, MEMORY, parent_rights).unwrap();
                let child_rights = Rights::from_bits_retain(child_bits);
                let result = table.delegate(parent, child_rights);
                let allowed = parent_rights.contains(Rights::DELEGATE)
                    && parent_rights.contains(child_rights);
                assert_eq!(result.is_ok(), allowed);
            }
        }
    }
}
