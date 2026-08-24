//! PS/2 keyboard, polled — see `docs/VGA.md`.
//!
//! The 8042 controller exposes two ports: status on 0x64, data on
//! 0x60. Bit 0 of the status byte means "a scancode is waiting"; the
//! scancode names a physical key position (set 1, make = press, make
//! | 0x80 = release), not a character — translating positions to ASCII
//! is this module's whole job, done here against the US QWERTY layout.
//!
//! Polled, not interrupt-driven: an IDT does not exist yet (that is
//! the next KFS), so `kmain` asks "anything pressed?" in its idle loop
//! instead of the keyboard barging in.

use crate::port::inb;

const STATUS_PORT: u16 = 0x64;
const DATA_PORT: u16 = 0x60;

/// What a keypress means to the caller, layout already applied.
pub enum Key {
    /// A printable byte, `\n` included.
    Char(u8),
    Backspace,
    /// F1/F2/F3: switch to that virtual screen.
    Screen(usize),
}

/// Held-shift state, carried across polls. Same single-threaded
/// argument as the VGA statics: one CPU, no interrupts, one caller.
static mut SHIFT: bool = false;

/// Scancode -> ASCII for the unshifted and shifted US QWERTY layers.
/// Index is the scancode; 0 marks keys with no printable meaning
/// (Esc, Ctrl, Alt, F-keys) or handled specially (Backspace, Shift).
const LOWER: &[u8; 58] = b"\x00\x001234567890-=\x00\x00qwertyuiop[]\n\x00asdfghjkl;'`\x00\\zxcvbnm,./\x00*\x00 ";
const UPPER: &[u8; 58] = b"\x00\x00!@#$%^&*()_+\x00\x00QWERTYUIOP{}\n\x00ASDFGHJKL:\"~\x00|ZXCVBNM<>?\x00*\x00 ";

/// One non-blocking poll: `None` when no scancode is waiting or the
/// one read carries no meaning for us (releases, dead keys).
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
        _ if scancode & 0x80 != 0 => None, // release of a non-shift key
        _ => {
            let table = if unsafe { SHIFT } { UPPER } else { LOWER };
            match table.get(scancode as usize) {
                Some(&b) if b != 0 => Some(Key::Char(b)),
                _ => None,
            }
        }
    }
}
