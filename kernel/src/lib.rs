#![no_std]

use core::panic::PanicInfo;

mod klib;
mod vga;

/// Kernel entry point, called from `_start` in `boot/boot.asm`.
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    vga::clear();
    let mut buf = [0u8; 10];
    vga::print(klib::utoa(42, &mut buf));
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
