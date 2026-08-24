#![no_std]

use core::panic::PanicInfo;

mod keyboard;
mod klib;
mod port;
mod printk;
mod vga;

use printk::printk;

/// Kernel entry point, called from `_start` in `boot/boot.asm`.
///
/// After the banner, `kmain` becomes a dumb terminal: poll the
/// keyboard, echo what it says. Busy-polling is the honest option
/// today — waking on a key press instead takes interrupts, which is
/// the next KFS project.
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    vga::clear();
    let mut buf = [0u8; 10];
    vga::print(klib::utoa(42, &mut buf));
    vga::print("\n");
    vga::set_color(vga::Color::BrightGreen, vga::Color::Black);
    printk!("kfs-{}\n", 1);
    vga::set_color(vga::Color::White, vga::Color::Black);
    loop {
        match keyboard::poll() {
            Some(keyboard::Key::Char(b)) => vga::put_char(b),
            Some(keyboard::Key::Backspace) => vga::backspace(),
            Some(keyboard::Key::Screen(n)) => vga::switch_screen(n),
            None => core::hint::spin_loop(),
        }
    }
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
