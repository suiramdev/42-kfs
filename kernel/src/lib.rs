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
    vga::print("\n");
    vga::set_color(vga::Color::BrightGreen, vga::Color::Black);
    vga::print("kfs-1\n");
    vga::set_color(vga::Color::White, vga::Color::Black);
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
