#![no_std]

use core::panic::PanicInfo;

mod vga;

/// Kernel entry point, called from `_start` in `boot/boot.asm`.
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    vga::clear();
    vga::print("42");
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
