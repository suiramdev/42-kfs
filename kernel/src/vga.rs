use core::ptr::{read_volatile, write_volatile};

use crate::port::outb;

const BUFFER: *mut u16 = 0xc00b8000 as *mut u16;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;
const CELLS: usize = WIDTH * HEIGHT;

#[repr(u8)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
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

static mut CURSOR: usize = 0;
static mut ATTR: u16 = DEFAULT_ATTR;

pub const NB_SCREENS: usize = 3;

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

pub fn set_color(fg: Color, bg: Color) {
    unsafe { ATTR = ((bg as u16) << 12) | ((fg as u16) << 8) }
}

fn put_cell(i: usize, cell: u16) {
    debug_assert!(i < CELLS);
    unsafe { write_volatile(BUFFER.add(i), cell) }
}

fn read_cell(i: usize) -> u16 {
    debug_assert!(i < CELLS);
    unsafe { read_volatile(BUFFER.add(i)) }
}

pub fn clear() {
    for i in 0..CELLS {
        put_cell(i, attr() | b' ' as u16);
    }
    set_cursor(0);
    sync_hw_cursor();
}

pub fn print(s: &str) {
    for b in s.bytes() {
        putb(b);
    }
    sync_hw_cursor();
}

pub fn put_char(b: u8) {
    putb(b);
    sync_hw_cursor();
}

pub fn backspace() {
    if cursor() > 0 {
        set_cursor(cursor() - 1);
        put_cell(cursor(), attr() | b' ' as u16);
        sync_hw_cursor();
    }
}

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

fn scroll() {
    for i in 0..CELLS - WIDTH {
        put_cell(i, read_cell(i + WIDTH));
    }
    for i in CELLS - WIDTH..CELLS {
        put_cell(i, attr() | b' ' as u16);
    }
}

const CRTC_INDEX: u16 = 0x3D4;
const CRTC_DATA: u16 = 0x3D5;

fn sync_hw_cursor() {
    let pos = cursor() as u16;
    unsafe {
        outb(CRTC_INDEX, 0x0F);
        outb(CRTC_DATA, pos as u8);
        outb(CRTC_INDEX, 0x0E);
        outb(CRTC_DATA, (pos >> 8) as u8);
    }
}
