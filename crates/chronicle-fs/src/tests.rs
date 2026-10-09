use super::*;

#[derive(Clone)]
struct Disk {
    sectors: BTreeMap<u64, [u8; 512]>,
    pending: BTreeMap<u64, [u8; 512]>,
    fail_at: Option<usize>,
    operations: usize,
}
impl Disk {
    fn new() -> Self {
        Self {
            sectors: BTreeMap::new(),
            pending: BTreeMap::new(),
            fail_at: None,
            operations: 0,
        }
    }
    fn operation(&mut self) -> Result<()> {
        let at = self.operations;
        self.operations += 1;
        if self.fail_at == Some(at) {
            Err(Error::Io)
        } else {
            Ok(())
        }
    }
    fn crash(mut self) -> Self {
        self.pending.clear();
        self.fail_at = None;
        self.operations = 0;
        self
    }
}
impl BlockDevice for Disk {
    fn read(&mut self, lba: u64, out: &mut [u8; 512]) -> Result<()> {
        *out = self
            .pending
            .get(&lba)
            .or_else(|| self.sectors.get(&lba))
            .copied()
            .unwrap_or([0; 512]);
        Ok(())
    }
    fn write(&mut self, lba: u64, data: &[u8; 512]) -> Result<()> {
        self.operation()?;
        self.pending.insert(lba, *data);
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        self.operation()?;
        self.sectors.append(&mut self.pending);
        Ok(())
    }
}
fn initial() -> FileSystem<Disk> {
    FileSystem::create_blank(Disk::new()).unwrap()
}

#[test]
fn files_snapshots_and_deletions_survive_remount() {
    let mut fs = initial();
    fs.write("notes.txt", b"before").unwrap();
    let snapshot = fs.snapshot("before-change").unwrap();
    fs.write("notes.txt", b"after").unwrap();
    fs.remove("notes.txt").unwrap();
    assert_eq!(fs.read("notes.txt"), Err(Error::NotFound));
    fs.restore(snapshot).unwrap();
    let remount = FileSystem::mount(fs.into_device().crash()).unwrap();
    assert_eq!(remount.read("notes.txt").unwrap(), b"before");
    assert_eq!(remount.history().len(), 6);
}

#[test]
fn every_interrupted_transaction_preserves_a_complete_root() {
    let mut base = initial();
    base.write("old", b"stable").unwrap();
    let disk = base.into_device();
    let payload = vec![42; 1700];
    // Four payload sectors, flush, commit sector, flush. Fail each operation.
    for cut in 0..7 {
        let mut d = disk.clone();
        d.operations = 0;
        d.fail_at = Some(cut);
        let mut fs = FileSystem::mount(d).unwrap();
        assert_eq!(
            fs.write_batch(&[("new", &payload), ("old", b"changed")], "batch"),
            Err(Error::Io)
        );
        assert_eq!(fs.read("old"), Err(Error::Io));
        assert_eq!(fs.write("later", b"x"), Err(Error::Io));
        let fs = FileSystem::mount(fs.into_device().crash()).unwrap();
        assert_eq!(fs.read("old").unwrap(), b"stable");
        assert_eq!(fs.read("new"), Err(Error::NotFound));
    }
}

#[test]
fn torn_commit_and_corrupt_payload_fall_back_to_previous_generation() {
    let mut fs = initial();
    fs.write("value", b"previous").unwrap();
    fs.write("value", b"latest").unwrap();
    let disk = fs.into_device();
    for byte in [0, 16, 24, 64, 96, 200] {
        let mut d = disk.clone();
        d.sectors.get_mut(&(2 * SLOT_SECTORS)).unwrap()[byte] ^= 1;
        assert_eq!(
            FileSystem::mount(d).unwrap().read("value").unwrap(),
            b"previous"
        );
    }
    let mut d = disk;
    d.sectors.get_mut(&(2 * SLOT_SECTORS + 1)).unwrap()[8] ^= 1;
    assert_eq!(
        FileSystem::mount(d).unwrap().read("value").unwrap(),
        b"previous"
    );
}

#[test]
fn rejects_bad_names_oversize_and_invalid_restore_without_changes() {
    let mut fs = initial();
    for name in ["", "..", "../escape", "a/b", "UPPER", "with space"] {
        assert_eq!(fs.write(name, b"x"), Err(Error::InvalidName));
    }
    assert_eq!(
        fs.write("big", &vec![0; MAX_FILE_BYTES + 1]),
        Err(Error::TooLarge)
    );
    assert_eq!(fs.restore(999), Err(Error::NotFound));
    assert_eq!(fs.generation(), 1);
}

#[test]
fn full_volume_never_overwrites_retained_snapshots() {
    let mut fs = initial();
    for _ in 1..SLOT_COUNT {
        fs.snapshot("kept").unwrap();
    }
    assert_eq!(fs.write("new", b"x"), Err(Error::Full));
    let fs = FileSystem::mount(fs.into_device()).unwrap();
    assert_eq!(fs.history().len(), SLOT_COUNT);
    assert_eq!(fs.generation(), SLOT_COUNT as u64);
}

#[test]
fn cannot_format_existing_or_corrupt_metadata() {
    let disk = initial().into_device();
    assert!(matches!(
        FileSystem::create_blank(disk.clone()),
        Err(Error::NotBlank)
    ));
    let mut corrupt = disk;
    corrupt.sectors.get_mut(&0).unwrap()[0] ^= 1;
    assert!(matches!(FileSystem::mount(corrupt), Err(Error::Corrupt)));
}

#[test]
fn batch_updates_are_one_durable_generation() {
    let mut fs = initial();
    fs.write_batch(
        &[("checkpoint", b"state"), ("trace", b"events")],
        "recovery-bundle",
    )
    .unwrap();
    let fs = FileSystem::mount(fs.into_device().crash()).unwrap();
    assert_eq!(fs.generation(), 2);
    assert_eq!(fs.read("checkpoint").unwrap(), b"state");
    assert_eq!(fs.read("trace").unwrap(), b"events");
}
