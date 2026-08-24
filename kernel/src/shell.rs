//! A debug shell — see `docs/SHELL.md`.
//!
//! Not a POSIX shell. A prompt, a line, a name looked up in a table of
//! six commands. It exists because the two things kfs-2 adds, a
//! descriptor table and a stack dump, are worth asking for at a moment
//! you choose rather than watching scroll past at boot.
//!
//! The commands live in `CMDS`, a table rather than a `match`. `help`
//! is then a loop over the same rows the dispatcher searches, so a new
//! command cannot appear without its own help line. The keyboard tables
//! in `keyboard.rs` and the colour palette in `vga.rs` are the same
//! choice: put the domain in data, not in branches.

use core::arch::asm;

use crate::printk::printk;
use crate::port::{inb, outb};
use crate::{gdt, keyboard, stack, vga};

/// One row of the command table. `run` takes nothing and returns
/// nothing, which keeps every row the same shape.
struct Cmd {
    name: &'static str,
    help: &'static str,
    run: fn(),
}

static CMDS: [Cmd; 6] = [
    Cmd { name: "stack", help: "hex dump of the live kernel stack", run: cmd_stack },
    Cmd { name: "gdt", help: "the descriptor table, read back from the CPU", run: cmd_gdt },
    Cmd { name: "clear", help: "blank the screen", run: cmd_clear },
    Cmd { name: "reboot", help: "restart the machine through the 8042", run: cmd_reboot },
    Cmd { name: "halt", help: "stop this processor for good", run: cmd_halt },
    Cmd { name: "help", help: "this list", run: cmd_help },
];

/// A command line, prompt included, fits one 80-column row.
const LINE_MAX: usize = 74;

const PROMPT: &str = "kfs> ";

/// Read a line, run it, repeat. Never returns: there is nothing to
/// return to, because `kmain` calls this last.
pub fn run() -> ! {
    printk!("type `help` for the command list\n");
    let mut buf = [0u8; LINE_MAX];
    loop {
        vga::set_color(vga::Color::BrightCyan, vga::Color::Black);
        vga::print(PROMPT);
        vga::set_color(vga::Color::White, vga::Color::Black);
        let line = read_line(&mut buf);
        dispatch(line);
    }
}

/// Collect keys until Enter. The line buffer and the screen must always
/// agree, so a key that does not fit is dropped and not echoed, and
/// Backspace at column zero does nothing rather than eating the prompt.
///
/// F1, F2 and F3 keep switching screens in the middle of a line,
/// because the editor forwards them instead of storing them.
fn read_line(buf: &mut [u8; LINE_MAX]) -> &str {
    let mut n = 0;
    loop {
        match keyboard::poll() {
            Some(keyboard::Key::Char(b'\n')) => {
                vga::put_char(b'\n');
                break;
            }
            Some(keyboard::Key::Char(b)) if n < LINE_MAX => {
                buf[n] = b;
                n += 1;
                vga::put_char(b);
            }
            Some(keyboard::Key::Char(_)) => {}
            Some(keyboard::Key::Backspace) if n > 0 => {
                n -= 1;
                vga::backspace();
            }
            Some(keyboard::Key::Backspace) => {}
            Some(keyboard::Key::Screen(s)) => vga::switch_screen(s),
            None => core::hint::spin_loop(),
        }
    }
    // The layout tables in `keyboard.rs` only ever produce bytes below
    // 0x80, so the collected line is valid UTF-8.
    unsafe { core::str::from_utf8_unchecked(&buf[..n]) }
}

fn dispatch(line: &str) {
    let name = line.trim();
    if name.is_empty() {
        return;
    }
    match CMDS.iter().find(|c| c.name == name) {
        Some(c) => (c.run)(),
        None => printk!("{}: no such command, try `help`\n", name),
    }
}

fn cmd_help() {
    for c in &CMDS {
        printk!("  {:6}  {}\n", c.name, c.help);
    }
}

fn cmd_clear() {
    vga::clear();
}

/// The window starts in this function's own frame, so the first row
/// holds the return address back into `dispatch`. That row is not
/// noise. It is the shortest proof the dump reads real stack.
fn cmd_stack() {
    stack::print(stack::esp(), stack::WINDOW);
}

/// Print the table the CPU is using, not the one the source declares.
/// `sgdt` answers with the base and limit in force, so a mismatch
/// against `gdt::BASE` is a real failure and shows up as one.
fn cmd_gdt() {
    let gdtr = gdt::current();
    let expected = gdt::LIMIT;
    let verdict = if gdtr.base == gdt::BASE as u32 && gdtr.limit == expected {
        "as declared"
    } else {
        "DOES NOT MATCH THE SOURCE"
    };
    printk!(
        "gdtr: base {:#010x} limit {:#06x} {} entries, {}\n",
        gdtr.base,
        gdtr.limit,
        (gdtr.limit as usize + 1) / 8,
        verdict
    );
    let (cs, ss, ds) = gdt::selectors();
    printk!(" sel  segment       range                   ring type     in use\n");
    for segment in &gdt::SEGMENTS {
        printk!(" {:#04x} {:13} ", segment.selector, segment.name);
        match seg_range(segment.descriptor) {
            None => printk!("{:23}", "-"),
            Some((lo, hi)) => printk!("{:#010x}..{:#010x} ", lo, hi),
        }
        if segment.descriptor == 0 {
            printk!("-    -       ");
        } else {
            // The ring is two bits, and `as u8` keeps the format that
            // way. `Display` for a u64 divides by ten in 64 bits, which
            // asks `compiler_builtins` for a division helper this
            // kernel otherwise never needs.
            let ring = ((segment.descriptor >> 45) & 3) as u8;
            printk!("{}    {:8}", ring, seg_type(segment.descriptor));
        }
        for (name, held) in [("cs", cs), ("ss", ss), ("ds", ds)] {
            if held & !3 == segment.selector & !3 {
                printk!(" {}", name);
            }
        }
        printk!("\n");
    }
}

/// Linear addresses this selector may reach, or `None` when it may
/// reach none.
///
/// A data segment with the direction bit set is expand-down: Intel
/// inverts the limit, and valid offsets run from limit + 1 upward. A
/// flat expand-down segment therefore reaches nothing at all, which is
/// worth seeing as an empty range instead of a healthy-looking limit.
fn seg_range(descriptor: u64) -> Option<(u32, u32)> {
    if descriptor == 0 {
        return None;
    }
    let base = (((descriptor >> 16) & 0xff_ffff) | (((descriptor >> 56) & 0xff) << 24)) as u32;
    let raw = ((descriptor & 0xffff) | (((descriptor >> 48) & 0xf) << 16)) as u32;
    // The granularity bit multiplies the limit by the 4 KiB page size,
    // and the last page counts in full.
    let limit = if descriptor & (1 << 55) != 0 { (raw << 12) | 0xfff } else { raw };
    let code = descriptor & (1 << 43) != 0;
    if !code && descriptor & (1 << 42) != 0 {
        let lo = limit.checked_add(1)?;
        return Some((base.wrapping_add(lo), u32::MAX));
    }
    Some((base, base.wrapping_add(limit)))
}

/// What the access byte says this segment is. Bit 43 splits code from
/// data, bit 41 grants the second right each kind can have, and bit 42
/// means expand-down on data and conforming on code.
fn seg_type(descriptor: u64) -> &'static str {
    let rw = descriptor & (1 << 41) != 0;
    let dc = descriptor & (1 << 42) != 0;
    match (descriptor & (1 << 43) != 0, rw, dc) {
        (true, true, false) => "code rx",
        (true, true, true) => "code rx c",
        (true, false, _) => "code x",
        (false, true, false) => "data rw",
        (false, true, true) => "data rw dn",
        (false, false, _) => "data r",
    }
}

/// Restart the machine. The 8042 keyboard controller drives the CPU
/// reset line, and command 0xfe pulls it low. Wait for the controller's
/// input buffer to drain first, or the byte is dropped.
///
/// A machine with no 8042 answering falls through to a deliberate
/// triple fault: an interrupt descriptor table of length zero, then a
/// breakpoint. The CPU cannot find the breakpoint handler, cannot find
/// the double-fault handler either, and a third fault shuts the
/// processor down, which every chipset wires to reset. Linux reboots
/// exactly this way.
fn cmd_reboot() {
    printk!("rebooting\n");
    unsafe {
        for _ in 0..10 {
            while inb(0x64) & 0x02 != 0 {}
            outb(0x64, 0xfe);
        }
        asm!("lidt [{}]", in(reg) &NULL_IDT, options(readonly, nostack, preserves_flags));
        asm!("int3", options(nostack));
    }
    cmd_halt();
}

/// The interrupt table that guarantees a triple fault: base zero,
/// length zero, so no vector can be fetched.
#[repr(C, packed)]
struct Dtr {
    limit: u16,
    base: u32,
}

static NULL_IDT: Dtr = Dtr { limit: 0, base: 0 };

/// Stop the processor. `hlt` on its own can be resumed by an interrupt
/// the firmware raises, so the loop is the idiom, and `_start` already
/// ends the same way.
fn cmd_halt() {
    printk!("halted\n");
    // `cli` writes the interrupt flag, so promising to preserve flags
    // here would be a lie.
    unsafe { asm!("cli", options(nomem, nostack)) };
    loop {
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}
