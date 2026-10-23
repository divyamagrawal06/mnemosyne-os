//! Polling PS/2 driver. IRQ1/IRQ12 remain masked, input work is finite per syscall.
//! Controller command reference: https://intel.github.io/ecfw-zephyr/reference/kbchost/index.html
use crate::device_model::{Keyboard, Mouse, command_ack};
use mnemosyne_abi::*;
use spin::Mutex;
use x86_64::instructions::port::Port;

struct Input {
    keyboard: Keyboard,
    mouse: Mouse,
    available: bool,
}
static INPUT: Mutex<Input> = Mutex::new(Input {
    keyboard: Keyboard::new(),
    mouse: Mouse::new(),
    available: false,
});
const WAIT_LIMIT: usize = 100_000;

fn status() -> u8 {
    // SAFETY: this module exclusively owns the 8042 ports; calls run with IRQs
    // disabled, serial access uses different ports, IRQ1/12 are always masked.
    unsafe { Port::<u8>::new(0x64).read() }
}
fn wait_write() -> bool {
    for _ in 0..WAIT_LIMIT {
        if status() & 2 == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}
fn command(value: u8) -> bool {
    if !wait_write() {
        return false;
    }
    unsafe {
        Port::<u8>::new(0x64).write(value);
    }
    true
}
fn data(value: u8) -> bool {
    if !wait_write() {
        return false;
    }
    unsafe {
        Port::<u8>::new(0x60).write(value);
    }
    true
}
fn read() -> u8 {
    unsafe { Port::<u8>::new(0x60).read() }
}
fn response() -> Option<u8> {
    for _ in 0..WAIT_LIMIT {
        let flags = status();
        if flags & 1 != 0 {
            let byte = read();
            return (flags & 0xc0 == 0).then_some(byte);
        }
        core::hint::spin_loop();
    }
    None
}
fn device(auxiliary: bool, value: u8) -> bool {
    if (auxiliary && !command(0xd4)) || !data(value) {
        return false;
    }
    // Keyboard and mouse share the controller output buffer. A key typed while
    // booting must not be mistaken for the mouse command ACK. Discard unrelated
    // boot input and accept only a parity-clean ACK from the addressed port.
    // The outer bound also limits a continuous stream of unrelated key presses.
    for _ in 0..WAIT_LIMIT {
        let flags = status();
        if flags & 1 == 0 {
            core::hint::spin_loop();
            continue;
        }
        let byte = read();
        if let Some(accepted) = command_ack(flags, byte, auxiliary) {
            return accepted;
        }
    }
    false
}

pub fn initialize() {
    let mut input = INPUT.lock();
    // Disable ports before changing controller config, drain stale firmware data,
    // retain translation to set 1, explicitly disable device IRQ generation.
    if !command(0xad) || !command(0xa7) {
        return;
    }
    for _ in 0..64 {
        if status() & 1 == 0 {
            break;
        }
        read();
    }
    if !command(0x20) {
        return;
    }
    let Some(config) = response() else {
        return;
    };
    if !command(0x60) || !data((config | 0x40) & !0x33) {
        return;
    }
    let keyboard = command(0xae) && device(false, 0xf4);
    let mouse = command(0xa8) && device(true, 0xf6) && device(true, 0xf4);
    input.available = keyboard || mouse;
    crate::serialln!(
        "{{\"event\":\"input-ready\",\"keyboard\":{keyboard},\"mouse\":{mouse},\"mode\":\"ps2-polled\"}}"
    );
}

pub fn poll() -> Result<Option<InputEvent>, ()> {
    let mut input = INPUT.lock();
    if !input.available {
        return Err(());
    }
    for _ in 0..64 {
        let flags = status();
        if flags & 1 == 0 {
            break;
        }
        let byte = read();
        if flags & 0xc0 != 0 {
            input.mouse.reset();
            continue;
        }
        if flags & 0x20 != 0 {
            if let Some(event) = input.mouse.feed(byte) {
                return Ok(Some(event));
            }
        } else if let Some(code) = input.keyboard.feed(byte) {
            return Ok(Some(InputEvent {
                kind: INPUT_KEYBOARD,
                code,
                x: 0,
                y: 0,
            }));
        }
    }
    Ok(None)
}
