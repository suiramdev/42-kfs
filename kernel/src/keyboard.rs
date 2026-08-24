use crate::port::inb;

const STATUS_PORT: u16 = 0x64;
const DATA_PORT: u16 = 0x60;

pub enum Key {
    Char(u8),
    Backspace,
    Screen(usize),
}

static mut SHIFT: bool = false;

const LOWER: &[u8; 58] = b"\x00\x001234567890-=\x00\x00qwertyuiop[]\n\x00asdfghjkl;'`\x00\\zxcvbnm,./\x00*\x00 ";
const UPPER: &[u8; 58] = b"\x00\x00!@#$%^&*()_+\x00\x00QWERTYUIOP{}\n\x00ASDFGHJKL:\"~\x00|ZXCVBNM<>?\x00*\x00 ";

pub fn poll() -> Option<Key> {
    if unsafe { inb(STATUS_PORT) } & 1 == 0 {
        return None;
    }
    let scancode = unsafe { inb(DATA_PORT) };
    match scancode {
        0x2A | 0x36 => {
            unsafe { SHIFT = true };
            None
        }
        0xAA | 0xB6 => {
            unsafe { SHIFT = false };
            None
        }
        0x0E => Some(Key::Backspace),
        0x3B..=0x3D => Some(Key::Screen((scancode - 0x3B) as usize)),
        _ if scancode & 0x80 != 0 => None,
        _ => {
            let table = if unsafe { SHIFT } { UPPER } else { LOWER };
            match table.get(scancode as usize) {
                Some(&b) if b != 0 => Some(Key::Char(b)),
                _ => None,
            }
        }
    }
}
