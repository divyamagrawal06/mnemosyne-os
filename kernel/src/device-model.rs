//! Pure bounds checks and PS/2 set-1/three-byte packet decoding.
//! No firmware code or external implementation is copied here.
use mnemosyne_abi::*;

pub fn rectangle_bytes(screen: DisplayInfo, x: u32, y: u32, w: u32, h: u32) -> Option<usize> {
    if w == 0 || h == 0 || x.checked_add(w)? > screen.width || y.checked_add(h)? > screen.height {
        return None;
    }
    let pixels = (w as usize).checked_mul(h as usize)?;
    (pixels <= DISPLAY_MAX_BLIT_PIXELS).then_some(pixels * 4)
}

#[derive(Default)]
pub struct Keyboard {
    left_shift: bool,
    right_shift: bool,
    caps: bool,
    caps_down: bool,
    extended: bool,
    pause_left: u8,
}
impl Keyboard {
    pub const fn new() -> Self {
        Self {
            left_shift: false,
            right_shift: false,
            caps: false,
            caps_down: false,
            extended: false,
            pause_left: 0,
        }
    }
    pub fn feed(&mut self, byte: u8) -> Option<u32> {
        if self.pause_left != 0 {
            self.pause_left -= 1;
            return None;
        }
        if byte == 0xe1 {
            self.pause_left = 5;
            return None;
        }
        if byte == 0xe0 {
            self.extended = true;
            return None;
        }
        if matches!(byte, 0xfa | 0xfe | 0x00 | 0xff) {
            return None;
        }
        let extended = core::mem::take(&mut self.extended);
        let released = byte & 0x80 != 0;
        let scan = byte & 0x7f;
        if !extended {
            match scan {
                0x2a => {
                    self.left_shift = !released;
                    return None;
                }
                0x36 => {
                    self.right_shift = !released;
                    return None;
                }
                0x3a => {
                    if !released && !self.caps_down {
                        self.caps = !self.caps;
                    }
                    self.caps_down = !released;
                    return None;
                }
                _ => (),
            }
        }
        if released {
            return None;
        }
        if extended {
            return match scan {
                0x48 => Some(KEY_UP),
                0x50 => Some(KEY_DOWN),
                0x4b => Some(KEY_LEFT),
                0x4d => Some(KEY_RIGHT),
                0x1c => Some(13),
                _ => None,
            };
        }
        if (0x3b..=0x3e).contains(&scan) {
            return Some(KEY_F1 + (scan - 0x3b) as u32);
        }
        let plain = match scan {
            0x01 => 27,
            0x0e => 8,
            0x0f => 9,
            0x1c => 13,
            0x39 => b' ',
            0x02..=0x0d => b"1234567890-="[(scan - 2) as usize],
            0x10..=0x1b => b"qwertyuiop[]"[(scan - 0x10) as usize],
            0x1e..=0x28 => b"asdfghjkl;'"[(scan - 0x1e) as usize],
            0x29 => b'`',
            0x2b => b'\\',
            0x2c..=0x35 => b"zxcvbnm,./"[(scan - 0x2c) as usize],
            _ => return None,
        };
        let shift = self.left_shift || self.right_shift;
        let code = if plain.is_ascii_lowercase() {
            if shift ^ self.caps {
                plain.to_ascii_uppercase()
            } else {
                plain
            }
        } else if shift {
            match plain {
                b'1'..=b'9' => b"!@#$%^&*("[(plain - b'1') as usize],
                b'0' => b')',
                b'-' => b'_',
                b'=' => b'+',
                b'[' => b'{',
                b']' => b'}',
                b';' => b':',
                b'\'' => b'"',
                b'`' => b'~',
                b'\\' => b'|',
                b',' => b'<',
                b'.' => b'>',
                b'/' => b'?',
                _ => plain,
            }
        } else {
            plain
        };
        Some(code as u32)
    }
}

pub struct Mouse {
    packet: [u8; 3],
    used: usize,
}
impl Mouse {
    pub const fn new() -> Self {
        Self {
            packet: [0; 3],
            used: 0,
        }
    }
    pub fn reset(&mut self) {
        self.used = 0;
    }
    pub fn feed(&mut self, byte: u8) -> Option<InputEvent> {
        // The first byte always has bit 3 set. Command ACKs are not packets.
        if self.used == 0 && (byte & 8 == 0 || matches!(byte, 0xfa | 0xfe)) {
            return None;
        }
        self.packet[self.used] = byte;
        self.used += 1;
        if self.used != 3 {
            return None;
        }
        self.used = 0;
        let flags = self.packet[0];
        // Overflow means movement was lost. Discard the whole packet instead of
        // inventing a large jump; the next complete packet resynchronizes.
        if flags & 0xc0 != 0 {
            return None;
        }
        let x = self.packet[1] as i32 - if flags & 0x10 != 0 { 256 } else { 0 };
        let y = self.packet[2] as i32 - if flags & 0x20 != 0 { 256 } else { 0 };
        Some(InputEvent {
            kind: INPUT_MOUSE,
            code: (flags & 7) as u32,
            x,
            y: -y,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangles_reject_overflow_partial_and_unbounded_blits() {
        let screen = DisplayInfo {
            width: 1024,
            height: 768,
        };
        assert_eq!(rectangle_bytes(screen, 0, 0, 1024, 16), Some(65536));
        for (x, y, w, h) in [
            (0, 0, 1024, 17),
            (1023, 0, 2, 1),
            (0, 767, 1, 2),
            (u32::MAX, 0, 2, 1),
            (0, 0, 0, 1),
        ] {
            assert_eq!(rectangle_bytes(screen, x, y, w, h), None);
        }
    }
    #[test]
    fn keyboard_modifiers_releases_prefixes_and_specials() {
        let mut keyboard = Keyboard::new();
        assert_eq!(keyboard.feed(0x1e), Some(b'a' as u32));
        assert_eq!(keyboard.feed(0x9e), None);
        keyboard.feed(0x2a);
        keyboard.feed(0x36);
        keyboard.feed(0xaa);
        assert_eq!(keyboard.feed(0x1e), Some(b'A' as u32));
        keyboard.feed(0xb6);
        keyboard.feed(0x3a);
        keyboard.feed(0x3a);
        keyboard.feed(0xba);
        assert_eq!(keyboard.feed(0x1e), Some(b'A' as u32));
        keyboard.feed(0x2a);
        assert_eq!(keyboard.feed(0x1e), Some(b'a' as u32));
        assert_eq!(keyboard.feed(0x02), Some(b'!' as u32));
        keyboard.feed(0xe0);
        assert_eq!(keyboard.feed(0x48), Some(KEY_UP));
        keyboard.feed(0xe0);
        assert_eq!(keyboard.feed(0xc8), None);
        assert_eq!(keyboard.feed(0x3e), Some(KEY_F4));
        keyboard.feed(0xe1);
        for b in [0x1d, 0x45, 0xe1, 0x9d, 0xc5] {
            assert_eq!(keyboard.feed(b), None);
        }
        assert_eq!(keyboard.feed(0x1c), Some(13));
    }
    #[test]
    fn mouse_sign_buttons_overflow_and_resynchronization() {
        let mut mouse = Mouse::new();
        assert!(mouse.feed(0xfa).is_none());
        assert!(mouse.feed(0).is_none());
        mouse.feed(0x39);
        mouse.feed(0xfe);
        let event = mouse.feed(0xfd).unwrap();
        assert_eq!((event.code, event.x, event.y), (1, -2, 3));
        mouse.feed(0xc8);
        mouse.feed(1);
        assert!(mouse.feed(2).is_none());
        mouse.feed(0x08);
        mouse.feed(0);
        assert_eq!(mouse.feed(0).unwrap().code, 0);
        mouse.feed(0x08);
        mouse.reset();
        mouse.feed(0x0a);
        mouse.feed(2);
        assert_eq!(mouse.feed(3).unwrap().code, 2);
    }
}
