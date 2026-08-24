use core::arch::asm;

use crate::printk::printk;
use crate::port::{inb, outb};
use crate::{gdt, keyboard, stack, vga};

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

const LINE_MAX: usize = 74;

const PROMPT: &str = "kfs> ";

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

fn cmd_stack() {
    stack::print(stack::esp(), stack::WINDOW);
}

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

fn seg_range(descriptor: u64) -> Option<(u32, u32)> {
    if descriptor == 0 {
        return None;
    }
    let base = (((descriptor >> 16) & 0xff_ffff) | (((descriptor >> 56) & 0xff) << 24)) as u32;
    let raw = ((descriptor & 0xffff) | (((descriptor >> 48) & 0xf) << 16)) as u32;
    let limit = if descriptor & (1 << 55) != 0 { (raw << 12) | 0xfff } else { raw };
    let code = descriptor & (1 << 43) != 0;
    if !code && descriptor & (1 << 42) != 0 {
        let lo = limit.checked_add(1)?;
        return Some((base.wrapping_add(lo), u32::MAX));
    }
    Some((base, base.wrapping_add(limit)))
}

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

#[repr(C, packed)]
struct Dtr {
    limit: u16,
    base: u32,
}

static NULL_IDT: Dtr = Dtr { limit: 0, base: 0 };

fn cmd_halt() {
    printk!("halted\n");
    unsafe { asm!("cli", options(nomem, nostack)) };
    loop {
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}
