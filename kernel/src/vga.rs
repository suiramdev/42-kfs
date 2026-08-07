//! VGA text mode driver — see `docs/VGA.md`.
//!
//! The screen is a memory-mapped 80x25 grid of u16 cells at physical
//! 0xb8000: low byte ASCII, high byte colour (bg << 4 | fg). Every
//! access goes through `read_volatile`/`write_volatile`: the values are
//! never otherwise observed, so a plain access is one the optimiser may
//! legally delete — the access itself is the I/O.
//!
//! The driver keeps two pieces of state, the write position and the
//! current colour. Plain `static mut`s are sound here because nothing
//! is concurrent: one CPU, interrupts never enabled, every call rooted
//! in `kmain`'s single call chain.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

const BUFFER: *mut u16 = 0xb8000 as *mut u16;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;
const CELLS: usize = WIDTH * HEIGHT;

/// The 16 VGA text colours. A cell's attribute byte is `bg << 4 | fg`;
/// values 0-7 are also valid backgrounds (8-15 would blink).
#[repr(u8)]
#[derive(Clone, Copy)]
#[allow(dead_code)] // the hardware palette, whether or not we use every entry
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGrey = 7,
    DarkGrey = 8,
    BrightBlue = 9,
    BrightGreen = 10,
    BrightCyan = 11,
    BrightRed = 12,
    BrightMagenta = 13,
    Yellow = 14,
    White = 15,
}

const DEFAULT_ATTR: u16 = (Color::White as u16) << 8;

/// Next cell `putb` writes to, always < `CELLS`.
static mut CURSOR: usize = 0;
/// Current colour, pre-shifted into the attribute byte position.
static mut ATTR: u16 = DEFAULT_ATTR;

/// How many virtual screens F1/F2/F3 switch between.
pub const NB_SCREENS: usize = 3;

/// A virtual screen while it is *not* displayed: the same 2 000 cells
/// the VGA buffer holds, plus the cursor and colour that go with them.
/// Only ever off-screen — the active screen lives in the real buffer
/// at 0xb8000 and nowhere else.
///
/// All-zero on purpose: a zeroed static lives in `.bss`, which costs
/// nothing in the binary — the loader provides the 12 KiB. Initialising
/// the cells to blanks here instead would drag the whole array into
/// `.data` and store every one of those spaces in the file (measured:
/// +12 KiB of `kernel.bin`). `USED` says which slots hold a real saved
/// screen; the others get blanked on first entry.
struct Screen {
    cells: [u16; CELLS],
    cursor: usize,
    attr: u16,
}

const ZERO: Screen = Screen {
    cells: [0; CELLS],
    cursor: 0,
    attr: 0,
};
static mut SCREENS: [Screen; NB_SCREENS] = [ZERO; NB_SCREENS];
static mut USED: [bool; NB_SCREENS] = [false; NB_SCREENS];
static mut ACTIVE: usize = 0;

fn cursor() -> usize {
    unsafe { CURSOR }
}

fn set_cursor(i: usize) {
    unsafe { CURSOR = i }
}

fn attr() -> u16 {
    unsafe { ATTR }
}

/// Colour applied to everything printed from now on.
pub fn set_color(fg: Color, bg: Color) {
    unsafe { ATTR = ((bg as u16) << 12) | ((fg as u16) << 8) }
}

/// Write one cell. `i` must be below `CELLS` — the sole bound that
/// keeps the pointer arithmetic inside the 4000-byte VGA window.
fn put_cell(i: usize, cell: u16) {
    debug_assert!(i < CELLS);
    unsafe { write_volatile(BUFFER.add(i), cell) }
}

/// Read one cell back; only `scroll` needs to.
fn read_cell(i: usize) -> u16 {
    debug_assert!(i < CELLS);
    unsafe { read_volatile(BUFFER.add(i)) }
}

/// Fill the whole screen with spaces in the current colour and put the
/// cursor back at the top-left.
pub fn clear() {
    for i in 0..CELLS {
        put_cell(i, attr() | b' ' as u16);
    }
    set_cursor(0);
    sync_hw_cursor();
}

/// Print `s` at the cursor, in the current colour. `\n` starts a new
/// line; other bytes are written as-is (non-ASCII renders as code page
/// 437 glyphs) and wrap at column 80. Output past the bottom row
/// scrolls the screen up one line.
pub fn print(s: &str) {
    for b in s.bytes() {
        putb(b);
    }
    sync_hw_cursor();
}

/// Print one byte at the cursor — `print` for the keyboard's pace,
/// where syncing the hardware cursor per byte costs nothing.
pub fn put_char(b: u8) {
    putb(b);
    sync_hw_cursor();
}

/// Undo one character: step the cursor back and blank the cell. At the
/// top-left there is nothing to undo.
pub fn backspace() {
    if cursor() > 0 {
        set_cursor(cursor() - 1);
        put_cell(cursor(), attr() | b' ' as u16);
        sync_hw_cursor();
    }
}

/// Make screen `n` the displayed one. The 4 000 live bytes move out of
/// the VGA buffer into the leaving screen's save slot, the entering
/// screen's slot moves in, and cursor + colour travel with each. Out of
/// range or already active: no-op.
pub fn switch_screen(n: usize) {
    if n >= NB_SCREENS || n == unsafe { ACTIVE } {
        return;
    }
    unsafe {
        let a = ACTIVE;
        for i in 0..CELLS {
            SCREENS[a].cells[i] = read_cell(i);
        }
        SCREENS[a].cursor = CURSOR;
        SCREENS[a].attr = ATTR;
        USED[a] = true;
        if USED[n] {
            for i in 0..CELLS {
                put_cell(i, SCREENS[n].cells[i]);
            }
            CURSOR = SCREENS[n].cursor;
            ATTR = SCREENS[n].attr;
        } else {
            ATTR = DEFAULT_ATTR;
            CURSOR = 0;
            for i in 0..CELLS {
                put_cell(i, DEFAULT_ATTR | b' ' as u16);
            }
        }
        ACTIVE = n;
    }
    sync_hw_cursor();
}

/// The hook `printk` hangs the formatting engine on: implementing
/// `write_str` is what makes `write_fmt` — and every format rule core
/// knows — work against this screen.
pub struct Writer;

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        print(s);
        Ok(())
    }
}

fn putb(b: u8) {
    match b {
        b'\n' => set_cursor(cursor() - cursor() % WIDTH + WIDTH),
        _ => {
            put_cell(cursor(), attr() | b as u16);
            set_cursor(cursor() + 1);
        }
    }
    if cursor() >= CELLS {
        scroll();
        set_cursor(CELLS - WIDTH);
    }
}

/// Shift rows 1-24 up over rows 0-23 and blank the bottom row. Cells
/// move with their colour: the copy is of whole u16s, read and written
/// volatile like every other buffer access.
fn scroll() {
    for i in 0..CELLS - WIDTH {
        put_cell(i, read_cell(i + WIDTH));
    }
    for i in CELLS - WIDTH..CELLS {
        put_cell(i, attr() | b' ' as u16);
    }
}

/// CRT controller ports: write a register index to 0x3D4, then that
/// register's value to 0x3D5. Registers 0x0E/0x0F hold the blinking
/// hardware cursor's cell index, high and low byte.
const CRTC_INDEX: u16 = 0x3D4;
const CRTC_DATA: u16 = 0x3D5;

/// Park the blinking hardware cursor on the driver's write position.
/// The cursor is not a character: it lives in CRT controller registers
/// reached by port I/O, x86's other address space, hence `out`.
fn sync_hw_cursor() {
    let pos = cursor() as u16;
    unsafe {
        outb(CRTC_INDEX, 0x0F);
        outb(CRTC_DATA, pos as u8);
        outb(CRTC_INDEX, 0x0E);
        outb(CRTC_DATA, (pos >> 8) as u8);
    }
}

/// # Safety
/// The port must be one whose side effects the caller accepts; `out`
/// itself cannot fault in ring 0.
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val,
             options(nomem, nostack, preserves_flags));
    }
}
