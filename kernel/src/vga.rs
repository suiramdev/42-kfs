//! VGA text mode driver — see `docs/VGA.md`.
//!
//! The screen is a memory-mapped 80x25 grid of u16 cells at physical
//! 0xb8000: low byte ASCII, high byte colour (bg << 4 | fg). Every store
//! goes through `write_volatile`: the value is never read back, so a
//! plain store is one the optimiser may legally delete — the write
//! itself is the I/O.

const BUFFER: *mut u16 = 0xb8000 as *mut u16;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;

/// Foreground 15 (white) on background 0 (black), in the high byte.
const WHITE_ON_BLACK: u16 = 0x0F00;

/// Write one cell. `i` must be below `WIDTH * HEIGHT` — the sole bound
/// that keeps the pointer arithmetic inside the 4000-byte VGA window.
fn put_cell(i: usize, cell: u16) {
    debug_assert!(i < WIDTH * HEIGHT);
    unsafe { core::ptr::write_volatile(BUFFER.add(i), cell) }
}

/// Fill the whole screen with white-on-black spaces.
pub fn clear() {
    for i in 0..WIDTH * HEIGHT {
        put_cell(i, WHITE_ON_BLACK | b' ' as u16);
    }
}

/// Print `s` white-on-black from the top-left cell. Bytes are written
/// as-is: non-ASCII renders as code page 437 glyphs. Output past the
/// last cell is dropped — no wrapping or scrolling yet.
pub fn print(s: &str) {
    for (i, b) in s.bytes().take(WIDTH * HEIGHT).enumerate() {
        put_cell(i, WHITE_ON_BLACK | b as u16);
    }
}
