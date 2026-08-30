use core::arch::asm;
use core::fmt::Arguments;

use crate::idt::Frame;
use crate::printk::printk;
use crate::vga::{self, Color};
use crate::{idt, paging, stack};

macro_rules! kpanic {
    ($($arg:tt)*) => {
        $crate::panic::fatal(core::format_args!($($arg)*))
    };
}

macro_rules! koops {
    ($($arg:tt)*) => {
        $crate::panic::oops(core::format_args!($($arg)*))
    };
}

pub(crate) use {koops, kpanic};

static mut OOPS: u32 = 0;

pub fn oops_count() -> u32 {
    unsafe { OOPS }
}

pub fn oops(args: Arguments) {
    unsafe { OOPS += 1 };
    vga::set_color(Color::Yellow, Color::Black);
    printk!("kernel oops {}: {}\n", oops_count(), args);
    vga::set_color(Color::White, Color::Black);
}

pub fn fatal(args: Arguments) -> ! {
    vga::set_color(Color::BrightRed, Color::Black);
    printk!("\nKERNEL PANIC: {}\n", args);
    machine();
    stop()
}

pub fn fatal_fault(frame: &Frame) -> ! {
    vga::set_color(Color::BrightRed, Color::Black);
    printk!("\nKERNEL PANIC: {} (vector {})\n", idt::name(frame.vector), frame.vector);
    if frame.vector == idt::PAGE_FAULT {
        let error = frame.error;
        printk!(
            "  cr2 {:#010x}, a {} {}, the page was {}\n",
            paging::cr2(),
            if error & 4 != 0 { "user" } else { "kernel" },
            if error & 2 != 0 { "write" } else { "read" },
            if error & 1 != 0 { "present: the rights said no" } else { "not present" }
        );
    }
    printk!(
        "  error {:#010x} eip {:#010x} cs {:#06x} eflags {:#010x}\n",
        frame.error,
        frame.eip,
        frame.cs,
        frame.eflags
    );
    printk!(
        "  eax {:#010x} ecx {:#010x} edx {:#010x} ebx {:#010x}\n",
        frame.eax,
        frame.ecx,
        frame.edx,
        frame.ebx
    );
    printk!(
        "  esi {:#010x} edi {:#010x} ebp {:#010x} esp {:#010x}\n",
        frame.esi,
        frame.edi,
        frame.ebp,
        frame.esp
    );
    machine();
    stop()
}

fn machine() {
    printk!(
        "  cr0 {:#010x} cr2 {:#010x} cr3 {:#010x}, {} recovered before this one\n",
        paging::cr0(),
        paging::cr2(),
        paging::cr3(),
        oops_count()
    );
    let esp = stack::esp();
    if esp >= stack::bottom() && esp < stack::top() {
        stack::print(esp, 64);
    }
    printk!("the kernel is stopped\n");
}

pub fn stop() -> ! {
    unsafe { asm!("cli", options(nomem, nostack)) };
    loop {
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}
