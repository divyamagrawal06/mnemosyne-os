//! Bounded append-only filesystem. A generation commits with one checksummed
//! sector, after its payload is flushed. Prior generations are never overwritten.
//! This deliberately trades capacity for an auditable crash-recovery protocol.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};
use sha2::{Digest, Sha256};

pub const SECTOR_BYTES: usize = 512;
pub const SLOT_SECTORS: u64 = 512;
pub const SLOT_COUNT: usize = 64;
pub const DISK_SECTORS: u64 = SLOT_SECTORS * SLOT_COUNT as u64;
pub const MAX_FILE_BYTES: usize = 192 * 1024;
pub const MAX_FILES: usize = 32;
const MAX_PAYLOAD: usize = (SLOT_SECTORS as usize - 1) * SECTOR_BYTES;
const MAGIC: &[u8; 8] = b"CHRONFS1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Io,
    Blank,
    Corrupt,
    Full,
    TooLarge,
    InvalidName,
    NotFound,
    NotBlank,
}
pub type Result<T> = core::result::Result<T, Error>;

pub trait BlockDevice {
    fn read(&mut self, sector: u64, data: &mut [u8; SECTOR_BYTES]) -> Result<()>;
    fn write(&mut self, sector: u64, data: &[u8; SECTOR_BYTES]) -> Result<()>;
    /// Success must mean all prior writes are durable and ordered before later writes.
    fn flush(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub id: u64,
    pub label: String,
    pub bytes: usize,
    slot: usize,
}

pub struct FileSystem<D> {
    disk: D,
    files: BTreeMap<String, Vec<u8>>,
    history: Vec<Generation>,
    used: [bool; SLOT_COUNT],
    generation: u64,
    // A failed flush has an ambiguous durability outcome. Require a remount
    // before another operation can assume which root is committed.
    poisoned: bool,
}

fn name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-._".contains(&b))
        && name != "."
        && name != ".."
}

fn encode(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    if files.len() > MAX_FILES {
        return Err(Error::Full);
    }
    let size = 2 + files
        .iter()
        .map(|(name, data)| 5 + name.len() + data.len())
        .sum::<usize>();
    if size > MAX_PAYLOAD {
        return Err(Error::TooLarge);
    }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    for (name, data) in files {
        if !name_valid(name) {
            return Err(Error::InvalidName);
        }
        if data.len() > MAX_FILE_BYTES {
            return Err(Error::TooLarge);
        }
        out.push(name.len() as u8);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
    }
    Ok(out)
}

fn decode(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    if bytes.len() < 2 {
        return Err(Error::Corrupt);
    }
    let count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    if count > MAX_FILES {
        return Err(Error::Corrupt);
    }
    let mut rest = &bytes[2..];
    let mut files = BTreeMap::new();
    for _ in 0..count {
        if rest.len() < 5 {
            return Err(Error::Corrupt);
        }
        let n = rest[0] as usize;
        let len = u32::from_le_bytes(rest[1..5].try_into().unwrap()) as usize;
        rest = &rest[5..];
        if len > MAX_FILE_BYTES || n > 48 || rest.len() < n + len {
            return Err(Error::Corrupt);
        }
        let name = core::str::from_utf8(&rest[..n]).map_err(|_| Error::Corrupt)?;
        if !name_valid(name) || files.contains_key(name) {
            return Err(Error::Corrupt);
        }
        files.insert(String::from(name), rest[n..n + len].to_vec());
        rest = &rest[n + len..];
    }
    if !rest.is_empty() {
        return Err(Error::Corrupt);
    }
    Ok(files)
}

impl<D: BlockDevice> FileSystem<D> {
    pub fn mount(mut disk: D) -> Result<Self> {
        let mut files = BTreeMap::new();
        let mut history = Vec::new();
        let mut used = [false; SLOT_COUNT];
        let mut generation = 0;
        for (slot, occupied) in used.iter_mut().enumerate() {
            let mut header = [0; SECTOR_BYTES];
            disk.read(slot as u64 * SLOT_SECTORS, &mut header)?;
            if header.iter().all(|b| *b == 0) {
                continue;
            }
            *occupied = true;
            let Some((id, label, len)) = Self::header(&header) else {
                continue;
            };
            let payload = Self::payload(&mut disk, slot, len)?;
            if Sha256::digest(&payload)[..] != header[64..96] {
                continue;
            }
            let Ok(decoded) = decode(&payload) else {
                continue;
            };
            // Equal committed IDs cannot be produced by the single-writer protocol.
            if history.iter().any(|g: &Generation| g.id == id) {
                return Err(Error::Corrupt);
            }
            if id > generation {
                generation = id;
                files = decoded;
            }
            history.push(Generation {
                id,
                label,
                bytes: len,
                slot,
            });
        }
        if generation == 0 {
            return Err(if used.iter().any(|u| *u) {
                Error::Corrupt
            } else {
                Error::Blank
            });
        }
        history.sort_by_key(|g| g.id);
        Ok(Self {
            disk,
            files,
            history,
            used,
            generation,
            poisoned: false,
        })
    }

    /// Creates a volume only if every commit-header sector is zero. It does not
    /// erase or reformat an existing volume, even one with corrupt metadata.
    pub fn create_blank(mut disk: D) -> Result<Self> {
        for slot in 0..SLOT_COUNT {
            let mut sector = [0; SECTOR_BYTES];
            disk.read(slot as u64 * SLOT_SECTORS, &mut sector)?;
            if sector.iter().any(|b| *b != 0) {
                return Err(Error::NotBlank);
            }
        }
        let mut fs = Self {
            disk,
            files: BTreeMap::new(),
            history: Vec::new(),
            used: [false; SLOT_COUNT],
            generation: 0,
            poisoned: false,
        };
        fs.commit(BTreeMap::new(), "initial")?;
        Ok(fs)
    }

    fn header(header: &[u8; SECTOR_BYTES]) -> Option<(u64, String, usize)> {
        if &header[..8] != MAGIC
            || header[8..16] != 1u64.to_le_bytes()
            || Sha256::digest(&header[..96])[..] != header[96..128]
            || header[128..].iter().any(|b| *b != 0)
        {
            return None;
        }
        let id = u64::from_le_bytes(header[16..24].try_into().ok()?);
        let len = u32::from_le_bytes(header[24..28].try_into().ok()?) as usize;
        if id == 0 || !(2..=MAX_PAYLOAD).contains(&len) {
            return None;
        }
        let end = header[28..60].iter().position(|b| *b == 0).unwrap_or(32);
        let label = core::str::from_utf8(&header[28..28 + end]).ok()?;
        if !name_valid(label) {
            return None;
        }
        Some((id, String::from(label), len))
    }

    fn payload(disk: &mut D, slot: usize, len: usize) -> Result<Vec<u8>> {
        let mut bytes = vec![0; len];
        for (index, chunk) in bytes.chunks_mut(SECTOR_BYTES).enumerate() {
            let mut sector = [0; SECTOR_BYTES];
            disk.read(slot as u64 * SLOT_SECTORS + 1 + index as u64, &mut sector)?;
            chunk.copy_from_slice(&sector[..chunk.len()]);
        }
        Ok(bytes)
    }

    fn commit(&mut self, next: BTreeMap<String, Vec<u8>>, label: &str) -> Result<u64> {
        if self.poisoned {
            return Err(Error::Io);
        }
        if !name_valid(label) || label.len() > 32 {
            return Err(Error::InvalidName);
        }
        let slot = self.used.iter().position(|used| !used).ok_or(Error::Full)?;
        let id = self.generation.checked_add(1).ok_or(Error::Full)?;
        let payload = encode(&next)?;
        self.used[slot] = true;
        self.poisoned = true;
        for (index, chunk) in payload.chunks(SECTOR_BYTES).enumerate() {
            let mut sector = [0; SECTOR_BYTES];
            sector[..chunk.len()].copy_from_slice(chunk);
            self.disk
                .write(slot as u64 * SLOT_SECTORS + 1 + index as u64, &sector)?;
        }
        self.disk.flush()?;
        let mut header = [0; SECTOR_BYTES];
        header[..8].copy_from_slice(MAGIC);
        header[8..16].copy_from_slice(&1u64.to_le_bytes());
        header[16..24].copy_from_slice(&id.to_le_bytes());
        header[24..28].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        header[28..28 + label.len()].copy_from_slice(label.as_bytes());
        header[64..96].copy_from_slice(&Sha256::digest(&payload));
        let digest = Sha256::digest(&header[..96]);
        header[96..128].copy_from_slice(&digest);
        self.disk.write(slot as u64 * SLOT_SECTORS, &header)?;
        self.disk.flush()?;
        self.files = next;
        self.generation = id;
        self.history.push(Generation {
            id,
            label: String::from(label),
            bytes: payload.len(),
            slot,
        });
        self.poisoned = false;
        Ok(id)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn history(&self) -> &[Generation] {
        &self.history
    }
    pub fn files(&self) -> impl Iterator<Item = (&str, usize)> {
        self.files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.len()))
    }
    pub fn read(&self, name: &str) -> Result<&[u8]> {
        if self.poisoned {
            return Err(Error::Io);
        }
        self.files
            .get(name)
            .map(Vec::as_slice)
            .ok_or(Error::NotFound)
    }
    pub fn write(&mut self, name: &str, bytes: &[u8]) -> Result<u64> {
        if !name_valid(name) {
            return Err(Error::InvalidName);
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err(Error::TooLarge);
        }
        let mut next = self.files.clone();
        next.insert(String::from(name), bytes.to_vec());
        self.commit(next, "write")
    }
    /// Several file updates publish as one generation, used for checkpoint+log bundles.
    pub fn write_batch(&mut self, updates: &[(&str, &[u8])], label: &str) -> Result<u64> {
        let mut next = self.files.clone();
        for (name, bytes) in updates {
            if !name_valid(name) {
                return Err(Error::InvalidName);
            }
            if bytes.len() > MAX_FILE_BYTES {
                return Err(Error::TooLarge);
            }
            next.insert(String::from(*name), bytes.to_vec());
        }
        self.commit(next, label)
    }
    pub fn remove(&mut self, name: &str) -> Result<u64> {
        let mut next = self.files.clone();
        next.remove(name).ok_or(Error::NotFound)?;
        self.commit(next, "remove")
    }
    pub fn snapshot(&mut self, label: &str) -> Result<u64> {
        self.commit(self.files.clone(), label)
    }
    pub fn restore(&mut self, id: u64) -> Result<u64> {
        if self.poisoned {
            return Err(Error::Io);
        }
        let entry = self
            .history
            .iter()
            .find(|g| g.id == id)
            .ok_or(Error::NotFound)?
            .clone();
        // Revalidate both metadata and data; a mounted history entry alone is not authority.
        let mut header = [0; SECTOR_BYTES];
        self.disk
            .read(entry.slot as u64 * SLOT_SECTORS, &mut header)?;
        let (actual, _, len) = Self::header(&header).ok_or(Error::Corrupt)?;
        if actual != id {
            return Err(Error::Corrupt);
        }
        let payload = Self::payload(&mut self.disk, entry.slot, len)?;
        if Sha256::digest(&payload)[..] != header[64..96] {
            return Err(Error::Corrupt);
        }
        self.commit(decode(&payload)?, "restore")
    }
    pub fn into_device(self) -> D {
        self.disk
    }
}

#[cfg(test)]
mod tests;
