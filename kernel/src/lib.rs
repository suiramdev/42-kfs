#![no_std]

use core::panic::PanicInfo;

mod gdt;
mod keyboard;
mod klib;
mod port;
mod printk;
mod shell;
mod stack;
mod vga;

use printk::printk;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    unsafe { gdt::install() };
    vga::clear();
    let mut buf = [0u8; 10];
    vga::print(klib::utoa(42, &mut buf));
    vga::print("\n");
    vga::set_color(vga::Color::BrightGreen, vga::Color::Black);
    printk!("kfs-{}\n", 2);
    vga::set_color(vga::Color::White, vga::Color::Black);
    shell::run()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga::set_color(vga::Color::BrightRed, vga::Color::Black);
    printk!("\npanic: {}\n", info);
    loop {
        core::hint::spin_loop();
    }
}
