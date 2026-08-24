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

/// Kernel entry point, called from `_start` in `boot/boot.asm`.
///
/// The first act is to replace GRUB's segmentation with our own. Every
/// segment register still holds a hidden copy of GRUB's descriptor, so
/// the machine keeps working until something reloads one, and the
/// multiboot specification is explicit that reloading before the kernel
/// owns a table is not allowed. `gdt::install` earns that right.
///
/// Then the banner, then the shell, which never returns. Everything is
/// polled: waking on a key press instead takes interrupts, which is the
/// next KFS project.
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

/// Panics land here — and now show up on screen instead of silently
/// freezing the machine. Printing may itself touch the driver state the
/// panicking code was in; acceptable, because a panic is already the
/// end: nothing runs afterwards but the idle loop.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga::set_color(vga::Color::BrightRed, vga::Color::Black);
    printk!("\npanic: {}\n", info);
    loop {
        core::hint::spin_loop();
    }
}
