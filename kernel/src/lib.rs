#![no_std]

use core::panic::PanicInfo;

/// Kernel entry point, called from `_start` in `boot/boot.asm`.
///
/// Screen output is not implemented yet — see `docs/VGA.md`.
#[no_mangle]
pub extern "C" fn kmain() -> ! {
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
