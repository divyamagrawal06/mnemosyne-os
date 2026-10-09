//! Minimal single-sector LBA28 PIO driver for QEMU's legacy primary IDE master.
//! No DMA, host disk pass-through, IRQ completion, or unbounded device polling.
use x86_64::instructions::port::Port;
const DATA: u16 = 0x1f0;
fn input(port: u16) -> u8 {
    unsafe { Port::<u8>::new(port).read() }
}
fn output(port: u16, value: u8) {
    unsafe {
        Port::<u8>::new(port).write(value);
    }
}
fn wait(drq: bool) -> Result<(), ()> {
    for _ in 0..1_000_000 {
        let status = input(0x1f7);
        if status == 0 || status == 0xff {
            return Err(());
        }
        if status & 0x80 != 0 {
            core::hint::spin_loop();
            continue;
        }
        if status & 0x21 != 0 {
            return Err(());
        }
        if !drq || status & 8 != 0 {
            return Ok(());
        }
    }
    Err(())
}
fn select(lba: u64) -> Result<(), ()> {
    if lba >= 32768 {
        return Err(());
    }
    wait(false)?;
    output(0x3f6, 2); // Disable ATA interrupts; completion is polled.
    output(0x1f6, 0xe0 | ((lba >> 24) as u8 & 0xf));
    for _ in 0..4 {
        input(0x3f6);
    }
    output(0x1f2, 1);
    output(0x1f3, lba as u8);
    output(0x1f4, (lba >> 8) as u8);
    output(0x1f5, (lba >> 16) as u8);
    Ok(())
}
pub fn read(lba: u64, bytes: &mut [u8; 512]) -> Result<(), ()> {
    select(lba)?;
    output(0x1f7, 0x20);
    wait(true)?;
    for pair in bytes.chunks_exact_mut(2) {
        let value = unsafe { Port::<u16>::new(DATA).read() };
        pair.copy_from_slice(&value.to_le_bytes());
    }
    wait(false)
}
pub fn write(lba: u64, bytes: &[u8; 512]) -> Result<(), ()> {
    select(lba)?;
    output(0x1f7, 0x30);
    wait(true)?;
    for pair in bytes.chunks_exact(2) {
        unsafe {
            Port::<u16>::new(DATA).write(u16::from_le_bytes([pair[0], pair[1]]));
        }
    }
    wait(false)
}
pub fn flush() -> Result<(), ()> {
    wait(false)?;
    output(0x1f7, 0xe7);
    wait(false)
}
