//! Limine framebuffer remains supervisor-only; init submits bounded pixel copies.
//! Protocol: https://github.com/limine-bootloader/limine-protocol/blob/trunk/include/limine.h
use crate::device_model::rectangle_bytes;
use limine::framebuffer::{Framebuffer, MemoryModel};
use mnemosyne_abi::*;
use spin::{Mutex, Once};

struct Display {
    address: usize,
    info: DisplayInfo,
    pitch: usize,
    bytes_per_pixel: usize,
    shifts: [u8; 3],
}
static DISPLAY: Once<Mutex<Display>> = Once::new();

pub fn initialize(framebuffer: &Framebuffer<'_>) {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let pitch = framebuffer.pitch();
    let bytes_per_pixel = (framebuffer.bpp() / 8) as usize;
    let shifts = [
        framebuffer.red_mask_shift(),
        framebuffer.green_mask_shift(),
        framebuffer.blue_mask_shift(),
    ];
    let sizes = [
        framebuffer.red_mask_size(),
        framebuffer.green_mask_size(),
        framebuffer.blue_mask_size(),
    ];
    // RGB 8:8:8, byte aligned, 24/32-bit formats only. Validate multiplication,
    // padding and every component before any pointer arithmetic or MMIO store.
    if framebuffer.memory_model() != MemoryModel::RGB
        || !matches!(framebuffer.bpp(), 24 | 32)
        || sizes != [8; 3]
        || shifts
            .iter()
            .any(|s| *s % 8 != 0 || *s as usize + 8 > framebuffer.bpp() as usize)
        || shifts[0] == shifts[1]
        || shifts[0] == shifts[2]
        || shifts[1] == shifts[2]
        || width == 0
        || height == 0
        || width > 4096
        || height > 2160
        || pitch < width * bytes_per_pixel as u64
        || pitch > 65536
        || pitch.checked_mul(height).is_none()
        || framebuffer.addr().is_null()
        || (framebuffer.addr() as usize)
            .checked_add((pitch * height) as usize)
            .is_none()
    {
        crate::serialln!(
            "{{\"event\":\"display-unavailable\",\"reason\":\"unsupported-framebuffer\"}}"
        );
        return;
    }
    DISPLAY.call_once(|| {
        Mutex::new(Display {
            address: framebuffer.addr() as usize,
            info: DisplayInfo {
                width: width as u32,
                height: height as u32,
            },
            pitch: pitch as usize,
            bytes_per_pixel,
            shifts,
        })
    });
    crate::serialln!(
        "{{\"event\":\"display-ready\",\"width\":{width},\"height\":{height},\"bpp\":{}}}",
        framebuffer.bpp()
    );
}

pub fn info() -> Option<DisplayInfo> {
    DISPLAY.get().map(|display| display.lock().info)
}

pub fn blit(
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    mut read: impl FnMut(usize, &mut [u8]) -> Result<(), ()>,
) -> i64 {
    let Some(display) = DISPLAY.get() else {
        return ERR_IO;
    };
    let display = display.lock();
    if rectangle_bytes(display.info, x, y, w, h).is_none() {
        return ERR_INVALID;
    }
    // At most 1 KiB of kernel stack, no user pointers ever dereferenced here.
    // The syscall checks the entire source mapping before the first visible store.
    let mut pixels = [0u8; 1024];
    for row in 0..h as usize {
        let mut column = 0;
        while column < w as usize {
            let count = (w as usize - column).min(256);
            let source = (row * w as usize + column) * 4;
            if read(source, &mut pixels[..count * 4]).is_err() {
                return ERR_INVALID;
            }
            for pixel in 0..count {
                let at = pixel * 4;
                let rgb = u32::from_le_bytes(pixels[at..at + 4].try_into().unwrap());
                let encoded = (((rgb >> 16) & 255) << display.shifts[0])
                    | (((rgb >> 8) & 255) << display.shifts[1])
                    | ((rgb & 255) << display.shifts[2]);
                let destination = display.address
                    + (y as usize + row) * display.pitch
                    + (x as usize + column + pixel) * display.bytes_per_pixel;
                for byte in 0..display.bytes_per_pixel {
                    // SAFETY: the Limine-owned framebuffer is mapped in the shared
                    // supervisor half. Bounds/layout were validated above; this
                    // mutex is its sole writer. Byte stores need no alignment.
                    unsafe {
                        ((destination + byte) as *mut u8)
                            .write_volatile((encoded >> (byte * 8)) as u8);
                    }
                }
            }
            column += count;
        }
    }
    0
}
