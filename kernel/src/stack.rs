use core::arch::asm;
use core::ptr::{addr_of, read_volatile};

use crate::printk::printk;

extern "C" {
    #[link_name = "stack_bottom"]
    static STACK_BOTTOM: u8;
    #[link_name = "stack_top"]
    static STACK_TOP: u8;
}

pub const WINDOW: usize = 256;

const PER_ROW: usize = 16;

#[inline(always)]
pub fn esp() -> u32 {
    let esp: u32;
    unsafe { asm!("mov {}, esp", out(reg) esp, options(nomem, nostack, preserves_flags)) };
    esp
}

pub fn top() -> u32 {
    addr_of!(STACK_TOP) as u32
}

pub fn bottom() -> u32 {
    addr_of!(STACK_BOTTOM) as u32
}

pub fn snapshot(from: u32, buf: &mut [u8]) -> &[u8] {
    let n = core::cmp::min(top().saturating_sub(from) as usize, buf.len());
    for (i, slot) in buf[..n].iter_mut().enumerate() {
        *slot = unsafe { read_volatile((from as *const u8).add(i)) };
    }
    &buf[..n]
}

#[inline(never)]
pub fn print(esp: u32, bytes: usize) {
    let (bottom, top) = (bottom(), top());
    printk!(
        "stack: esp {:#010x} -> top {:#010x}, {} of {} bytes in use\n",
        esp,
        top,
        top.saturating_sub(esp),
        top - bottom
    );
    let mut buf = [0u8; WINDOW];
    let mut at = esp;
    let mut left = core::cmp::min(bytes, top.saturating_sub(esp) as usize);
    while left > 0 {
        let got = snapshot(at, &mut buf[..core::cmp::min(left, WINDOW)]);
        if got.is_empty() {
            break;
        }
        render(at, got);
        at += got.len() as u32;
        left -= got.len();
    }
}

fn render(base: u32, bytes: &[u8]) {
    for (row, chunk) in bytes.chunks(PER_ROW).enumerate() {
        printk!("{:08x}  ", base + (row * PER_ROW) as u32);
        for i in 0..PER_ROW {
            match chunk.get(i) {
                Some(b) => printk!("{:02x} ", b),
                None => printk!("   "),
            }
            if i == 7 {
                printk!(" ");
            }
        }
        let mut text = [b'.'; PER_ROW];
        for (slot, b) in text.iter_mut().zip(chunk) {
            *slot = if (0x20..0x7f).contains(b) { *b } else { b'.' };
        }
        let text = unsafe { core::str::from_utf8_unchecked(&text[..chunk.len()]) };
        printk!("|{}|\n", text);
    }
}
