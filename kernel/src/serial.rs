use core::fmt::{self, Write};
use spin::{Mutex, Once};
use uart_16550::{Config, Uart16550Tty, backend::PioBackend};

static SERIAL: Once<Mutex<Uart16550Tty<PioBackend>>> = Once::new();

pub fn read_byte() -> Option<u8> {
    // COM1 IRQs are masked; the syscall polls its receive register under the
    // kernel's single-CPU, interrupts-disabled serialization.
    unsafe {
        use x86_64::instructions::port::Port;
        if Port::<u8>::new(0x3fd).read() & 1 == 0 {
            None
        } else {
            Some(Port::<u8>::new(0x3f8).read())
        }
    }
}

pub fn write(arguments: fmt::Arguments<'_>) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let serial = SERIAL.call_once(|| {
            // SAFETY: COM1 is reserved for this single-core kernel's serial driver.
            let port = unsafe { Uart16550Tty::new_port(0x3f8, Config::default()) }
                .expect("COM1 initialization");
            Mutex::new(port)
        });
        let _ = serial.lock().write_fmt(arguments);
    });
}

pub fn panic_write(arguments: fmt::Arguments<'_>) {
    if let Some(serial) = SERIAL.get()
        && let Some(mut port) = serial.try_lock()
    {
        let _ = port.write_fmt(arguments);
    }
}

#[macro_export]
macro_rules! serialln {
    ($($arg:tt)*) => {
        $crate::serial::write(format_args!("{}\n", format_args!($($arg)*)))
    };
}
