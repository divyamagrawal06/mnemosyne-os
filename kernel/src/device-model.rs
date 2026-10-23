//! Pure bounds checks and PS/2 set-1/three-byte packet decoding.
//! No firmware code or external implementation is copied here.
use mnemosyne_abi::*;

pub const INPUT_QUEUE_CAPACITY: usize = 256;
const EMPTY_EVENT: InputEvent = InputEvent {
    kind: 0,
    code: 0,
    x: 0,
    y: 0,
};

/// Fixed event ring. Adjacent motion with unchanged buttons and direction
/// coalesces. Reversals stay separate: screen-edge clamping after each direction
/// segment must have the same result as clamping after each original packet. At full
/// capacity discard the oldest keyboard event first, retaining the latest mouse
/// button state even during a long keyboard-only flood. If all entries are mouse
/// transitions, discard the oldest transition; the newest state remains queued.
/// Keyboard releases update the decoder before enqueueing and never consume
/// queue slots, so overflow cannot leave Shift/Caps in a stale decoder state.
pub struct InputQueue {
    events: [InputEvent; INPUT_QUEUE_CAPACITY],
    head: usize,
    len: usize,
    dropped: u64,
}
impl InputQueue {
    pub const fn new() -> Self {
        Self {
            events: [EMPTY_EVENT; INPUT_QUEUE_CAPACITY],
            head: 0,
            len: 0,
            dropped: 0,
        }
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
    pub fn push(&mut self, event: InputEvent) {
        if self.len != 0 && event.kind == INPUT_MOUSE {
            let tail = (self.head + self.len - 1) % INPUT_QUEUE_CAPACITY;
            let previous = &mut self.events[tail];
            let same_direction = |a: i32, b: i32| a == 0 || b == 0 || a.signum() == b.signum();
            if previous.kind == INPUT_MOUSE
                && previous.code == event.code
                && same_direction(previous.x, event.x)
                && same_direction(previous.y, event.y)
            {
                previous.x = previous.x.saturating_add(event.x).clamp(-32767, 32767);
                previous.y = previous.y.saturating_add(event.y).clamp(-32767, 32767);
                return;
            }
        }
        if self.len == INPUT_QUEUE_CAPACITY {
            let victim = (0..self.len)
                .find(|index| {
                    self.events[(self.head + index) % INPUT_QUEUE_CAPACITY].kind == INPUT_KEYBOARD
                })
                .unwrap_or(0);
            for index in victim..self.len - 1 {
                self.events[(self.head + index) % INPUT_QUEUE_CAPACITY] =
                    self.events[(self.head + index + 1) % INPUT_QUEUE_CAPACITY];
            }
            self.len -= 1;
            self.dropped = self.dropped.saturating_add(1);
        }
        self.events[(self.head + self.len) % INPUT_QUEUE_CAPACITY] = event;
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<InputEvent> {
        if self.len == 0 {
            return None;
        }
        let event = self.events[self.head];
        self.head = (self.head + 1) % INPUT_QUEUE_CAPACITY;
        self.len -= 1;
        Some(event)
    }
}

/// Some(true/false) only for ACK/resend from the addressed PS/2 device.
/// Unrelated input and corrupt/controller-empty bytes never complete a command.
pub fn command_ack(status: u8, byte: u8, auxiliary: bool) -> Option<bool> {
    if status & 1 == 0 || status & 0xc0 != 0 || (status & 0x20 != 0) != auxiliary {
        return None;
    }
    match byte {
        0xfa => Some(true),
        0xfe => Some(false),
        _ => None,
    }
}

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
    fn key(code: u32) -> InputEvent {
        InputEvent {
            kind: INPUT_KEYBOARD,
            code,
            x: 0,
            y: 0,
        }
    }
    fn mouse(buttons: u32, x: i32, y: i32) -> InputEvent {
        InputEvent {
            kind: INPUT_MOUSE,
            code: buttons,
            x,
            y,
        }
    }
    #[test]
    fn queue_preserves_keyboard_order_across_delayed_consumer_and_wraparound() {
        let mut queue = InputQueue::new();
        for cycle in 0..10 {
            for code in 0..200 {
                queue.push(key(cycle * 200 + code));
            }
            for code in 0..200 {
                assert_eq!(queue.pop().unwrap().code, cycle * 200 + code);
            }
            assert!(queue.pop().is_none());
        }
        assert_eq!(queue.dropped(), 0);
    }
    #[test]
    fn queue_bounds_and_overflow_preserve_latest_mouse_release() {
        let mut queue = InputQueue::new();
        queue.push(mouse(1, 0, 0));
        queue.push(mouse(0, 0, 0));
        for code in 0..1024 {
            queue.push(key(code));
        }
        assert_eq!(queue.len, INPUT_QUEUE_CAPACITY);
        assert_eq!(queue.dropped(), 770);
        assert_eq!(queue.pop().unwrap().code, 1);
        let release = queue.pop().unwrap();
        assert_eq!((release.kind, release.code), (INPUT_MOUSE, 0));
        let mut last = 0;
        while let Some(event) = queue.pop() {
            last = event.code;
        }
        assert_eq!(last, 1023);
        for buttons in 0..512 {
            queue.push(mouse(buttons % 2, 0, 0));
        }
        assert_eq!(queue.len, INPUT_QUEUE_CAPACITY);
        let mut last = EMPTY_EVENT;
        while let Some(event) = queue.pop() {
            last = event;
        }
        assert_eq!(last.code, 1);
    }
    #[test]
    fn queue_coalesces_motion_without_merging_button_or_keyboard_boundaries() {
        let mut queue = InputQueue::new();
        for _ in 0..1000 {
            queue.push(mouse(0, 2, -1));
        }
        assert_eq!(queue.len, 1);
        queue.push(mouse(1, 0, 0));
        queue.push(key(13));
        queue.push(mouse(1, 1, 1));
        queue.push(mouse(0, 0, 0));
        let movement = queue.pop().unwrap();
        assert_eq!((movement.x, movement.y), (2000, -1000));
        assert_eq!(queue.pop().unwrap().code, 1);
        assert_eq!(queue.pop().unwrap().kind, INPUT_KEYBOARD);
        assert_eq!(queue.pop().unwrap().code, 1);
        assert_eq!(queue.pop().unwrap().code, 0);
        queue.push(mouse(0, i32::MAX, i32::MIN));
        queue.push(mouse(0, i32::MAX, i32::MIN));
        let movement = queue.pop().unwrap();
        assert_eq!((movement.x, movement.y), (32767, -32767));
        assert_eq!(queue.dropped(), 0);
    }
    #[test]
    fn mouse_direction_changes_preserve_screen_edge_clamping() {
        let mut queue = InputQueue::new();
        let (mut original_x, mut original_y) = (40i32, 30i32);
        for (x, y) in core::iter::repeat_n((-100, -100), 20).chain([(100, 100), (0, 100), (0, 50)])
        {
            original_x = (original_x + x).clamp(0, 1023);
            original_y = (original_y + y).clamp(0, 767);
            queue.push(mouse(0, x, y));
        }
        let (mut queued_x, mut queued_y) = (40i32, 30i32);
        while let Some(event) = queue.pop() {
            queued_x = (queued_x + event.x).clamp(0, 1023);
            queued_y = (queued_y + event.y).clamp(0, 767);
        }
        assert_eq!((original_x, original_y), (100, 250));
        assert_eq!((queued_x, queued_y), (original_x, original_y));

        // Reversing only one axis still needs a distinct packet; otherwise an
        // edge on that axis changes the result even if the other axis agrees.
        queue.push(mouse(0, -100, 50));
        queue.push(mouse(0, 20, 50));
        assert_eq!(queue.len, 2);
    }
    #[test]
    fn keyboard_release_state_survives_event_queue_overflow() {
        let mut keyboard = Keyboard::new();
        let mut queue = InputQueue::new();
        keyboard.feed(0x2a); // Shift held while userspace is painting.
        for _ in 0..512 {
            if let Some(code) = keyboard.feed(0x1e) {
                queue.push(key(code));
            }
            assert_eq!(keyboard.feed(0x9e), None);
        }
        assert_eq!(keyboard.feed(0xaa), None); // Release processed even when full.
        queue.push(key(keyboard.feed(0x1e).unwrap()));
        assert!(queue.dropped() > 0);
        let mut final_key = 0;
        while let Some(event) = queue.pop() {
            final_key = event.code;
        }
        assert_eq!(final_key, b'a' as u32);
    }
    #[test]
    fn command_ack_requires_clean_origin_and_reply() {
        assert_eq!(command_ack(0x21, 0xfa, true), Some(true));
        assert_eq!(command_ack(0x01, 0xfa, false), Some(true));
        assert_eq!(command_ack(0x21, 0xfe, true), Some(false));
        for (status, byte, auxiliary) in [
            (0x01, 0xfa, true),  // Keyboard ACK cannot enable the mouse.
            (0x21, 0xfa, false), // Mouse ACK cannot enable the keyboard.
            (0x21, 0x1e, true),  // Arbitrary input is not an ACK.
            (0xa1, 0xfa, true),  // Parity error.
            (0x61, 0xfa, true),  // Controller timeout.
            (0x20, 0xfa, true),  // Output buffer empty.
        ] {
            assert_eq!(command_ack(status, byte, auxiliary), None);
        }
    }
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
