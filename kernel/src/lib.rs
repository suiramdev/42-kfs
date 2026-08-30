#![no_std]

use core::panic::PanicInfo;

mod gdt;
mod heap;
mod idt;
mod keyboard;
mod klib;
mod mem;
mod multiboot;
mod paging;
mod panic;
mod pmm;
mod port;
mod printk;
mod shell;
mod stack;
mod vga;

use printk::printk;

#[no_mangle]
pub extern "C" fn kmain(magic: u32, info: u32) -> ! {
    unsafe { gdt::install() };
    vga::clear();
    let mut buf = [0u8; 10];
    vga::print(klib::utoa(42, &mut buf));
    vga::print("\n");
    vga::set_color(vga::Color::BrightGreen, vga::Color::Black);
    printk!("kfs-{}\n", 3);
    vga::set_color(vga::Color::White, vga::Color::Black);
    idt::install();
    mem::init(magic, info);
    shell::run()
}

#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    panic::fatal(core::format_args!("{}", info))
}
