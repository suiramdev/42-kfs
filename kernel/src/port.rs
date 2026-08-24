use core::arch::asm;

pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") val,
             options(nomem, nostack, preserves_flags));
    }
    val
}

pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val,
             options(nomem, nostack, preserves_flags));
    }
}
