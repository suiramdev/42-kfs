//! Kernel stack printer — see `docs/STACK.md`.
//!
//! The kernel stack is the 16 KiB `boot/boot.asm` reserves in `.bss`.
//! It grows down from `stack_top`, so the part in use is the range from
//! the live `esp` up to `stack_top`, and everything below `esp` is
//! either free or the remains of calls that already returned.
//!
//! A dump is a copy of that range, not a view of it. A `&[u8]` over
//! live stack memory would tell the compiler nobody writes those bytes,
//! and the printer itself writes there on its very next call. Copying
//! first into a buffer that sits *below* the captured `esp` keeps the
//! window and the machinery that prints it in separate memory.
//!
//! No frame-pointer walk. `kernel/i686-kfs.json` names no
//! `frame-pointer` setting, so rustc omits it where it can and `ebp` is
//! an ordinary register in this kernel. A backtrace built from `[ebp]`
//! would print structured-looking nonsense. Bytes are true whatever the
//! optimiser did.

use core::arch::asm;
use core::ptr::{addr_of, read_volatile};

use crate::printk::printk;

extern "C" {
    #[link_name = "stack_bottom"]
    static STACK_BOTTOM: u8;
    #[link_name = "stack_top"]
    static STACK_TOP: u8;
}

/// One screenful: 16 rows of 16 bytes. This is the copy buffer's size,
/// so a longer dump costs another pass and not a bigger frame.
pub const WINDOW: usize = 256;

const PER_ROW: usize = 16;

/// The live stack pointer, read in the *caller's* frame.
///
/// `inline(always)` carries the whole contract. A real call would push
/// a return address first, and the answer would describe this function
/// instead of the code that asked.
#[inline(always)]
pub fn esp() -> u32 {
    let esp: u32;
    unsafe { asm!("mov {}, esp", out(reg) esp, options(nomem, nostack, preserves_flags)) };
    esp
}

/// The high end of the stack, where `_start` set `esp` before calling
/// `kmain`. Only the linker knows the address, so `boot.asm` exports it.
pub fn top() -> u32 {
    addr_of!(STACK_TOP) as u32
}

/// The low end. One byte further down overflows the stack, and nothing
/// today would notice.
pub fn bottom() -> u32 {
    addr_of!(STACK_BOTTOM) as u32
}

/// Copy stack bytes upward from `from`, stopping at `stack_top` or when
/// `buf` is full, and return the part that was filled.
///
/// `read_volatile` rather than a plain read: an address cast from an
/// integer carries no provenance any Rust memory model recognises, and
/// volatile also stops the optimiser folding these reads against the
/// caller's own locals, which live in the same region.
pub fn snapshot(from: u32, buf: &mut [u8]) -> &[u8] {
    let n = core::cmp::min(top().saturating_sub(from) as usize, buf.len());
    for (i, slot) in buf[..n].iter_mut().enumerate() {
        *slot = unsafe { read_volatile((from as *const u8).add(i)) };
    }
    &buf[..n]
}

/// Print the live stack, from `esp` up to `stack_top` or `bytes`,
/// whichever ends first.
///
/// `esp` is a parameter because it must be the *caller's*. Reading it
/// here would report this function's own frame, and the caller's saved
/// registers would sit below the window instead of inside it.
///
/// `inline(never)` is as load-bearing as `esp`'s `inline(always)`, and
/// for the mirror-image reason. The copy buffer must sit below the
/// captured `esp`, and it only does while this function owns a frame.
/// Inlined into the caller, the buffer lands inside the window it is
/// meant to capture, every read past its start returns a byte the copy
/// already wrote, and the dump repeats with a period equal to the
/// distance from `esp` up to the buffer. That output looks like real
/// stack data, which is what earns the attribute a paragraph.
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

/// One 77-column row per 16 bytes: the address, the bytes in memory
/// order, then the printable ones as characters.
///
/// Memory order is the caveat that matters on x86. The return address
/// 0x0010063c appears as `3c 06 10 00`, lowest byte first, so an
/// address reads backwards in the hex column.
///
/// This function names no stack concept and reads no raw pointer. That
/// is what keeps every unsafe read on the other side of the boundary.
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
        // Every byte was just forced into the printable ASCII range.
        let text = unsafe { core::str::from_utf8_unchecked(&text[..chunk.len()]) };
        printk!("|{}|\n", text);
    }
}
