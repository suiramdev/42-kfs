//! Port I/O — the other address space x86 has.
//!
//! Devices on a PC answer at two kinds of address. Memory-mapped ones,
//! like the VGA text buffer, live in ordinary RAM addresses. The rest
//! live in a separate 65 536-slot space that only `in` and `out` reach.
//! The CRT controller, the 8042 keyboard controller and the reset line
//! are all in that second space, so three modules need these two
//! instructions and none of them should own a private copy.

use core::arch::asm;

/// Read one byte from a port.
///
/// # Safety
/// The read must be one whose side effect the caller accepts. Reading
/// 0x60 consumes a scancode, and the byte is gone afterwards. `in`
/// itself cannot fault in ring 0.
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") val,
             options(nomem, nostack, preserves_flags));
    }
    val
}

/// Write one byte to a port.
///
/// # Safety
/// The write must be one whose side effect the caller accepts. Port
/// 0x64 can reset the machine. `out` itself cannot fault in ring 0.
pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val,
             options(nomem, nostack, preserves_flags));
    }
}
