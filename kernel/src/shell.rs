use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use crate::panic::{koops, kpanic};
use crate::port::{inb, outb};
use crate::printk::printk;
use crate::{gdt, heap, keyboard, klib, mem, paging, pmm, stack, vga};

struct Cmd {
    name: &'static str,
    help: &'static str,
    run: fn(&str),
}

static CMDS: [Cmd; 14] = [
    Cmd { name: "mem", help: "physical memory, frames and both heaps", run: cmd_mem },
    Cmd { name: "space", help: "the kernel and user address space layout", run: cmd_space },
    Cmd { name: "pages", help: "every mapping in the page directory", run: cmd_pages },
    Cmd { name: "virt", help: "translate an address: virt [addr]", run: cmd_virt },
    Cmd { name: "alloc", help: "exercise kmalloc, vmalloc, kbrk and vbrk", run: cmd_alloc },
    Cmd { name: "user", help: "map, use and drop a user space page", run: cmd_user },
    Cmd { name: "fault", help: "make a page fault: fault [demand|ro|kernel]", run: cmd_fault },
    Cmd { name: "panic", help: "panic on purpose: panic [oops|fatal]", run: cmd_panic },
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
    let line = line.trim();
    let (name, args) = match line.find(' ') {
        Some(at) => (&line[..at], line[at + 1..].trim()),
        None => (line, ""),
    };
    if name.is_empty() {
        return;
    }
    match CMDS.iter().find(|c| c.name == name) {
        Some(c) => (c.run)(args),
        None => printk!("{}: no such command, try `help`\n", name),
    }
}

fn cmd_help(_: &str) {
    for c in &CMDS {
        printk!("  {:6}  {}\n", c.name, c.help);
    }
}

fn cmd_clear(_: &str) {
    vga::clear();
}

fn cmd_stack(_: &str) {
    stack::print(stack::esp(), stack::WINDOW);
}

fn cmd_mem(_: &str) {
    let ram = mem::ram();
    let image = mem::image();
    let window = mem::heap_window();
    printk!("ram: {} kb usable in {} regions, from the {}\n", ram.usable / 1024, ram.count, ram.source);
    for region in &ram.regions[..ram.count] {
        printk!("  {:#010x}..{:#010x}  {} kb\n", region.base, region.end, (region.end - region.base) / 1024);
    }
    printk!(
        "frames: {} of {} free, {} used, largest free run {} frames of {} kb\n",
        pmm::free_frames(),
        pmm::total_frames(),
        pmm::used_frames(),
        pmm::largest_run(),
        mem::PAGE_SIZE / 1024
    );
    printk!(
        "kernel: {:#010x}..{:#010x}, code to {:#010x}, read only to {:#010x}\n",
        image.start,
        image.end,
        image.code_end,
        image.readonly_end
    );
    printk!(
        "        physical {:#010x}..{:#010x}, {} kb\n",
        mem::kernel_phys(image.start),
        mem::kernel_phys(image.end),
        (image.end - image.start) / 1024
    );
    printk!("carved: {:#010x}..{:#010x} physical, kept for kmalloc\n", window.0, window.1);
    report(heap::kernel_stats());
    report(heap::virtual_stats());
}

fn report(stats: heap::Stats) {
    printk!(
        "{}: {:#010x}..{:#010x}, ceiling {:#010x}, {} kb mapped\n",
        stats.name,
        stats.base,
        stats.brk,
        stats.limit,
        (stats.brk - stats.base) / 1024
    );
    printk!(
        "  {} bytes used, {} free, {} of {} blocks live, {}\n",
        stats.used,
        stats.free,
        stats.live,
        stats.blocks,
        if stats.contiguous { "physically contiguous" } else { "physically scattered" }
    );
    if stats.contiguous {
        printk!("  physical {:#010x}..{:#010x}\n", stats.phys_base, stats.phys_end);
    }
}

fn cmd_space(_: &str) {
    printk!(" range                  owner   region        contents\n");
    for region in &mem::LAYOUT {
        printk!(
            " {:#010x}..{:#010x} {:7} {:13} {}\n",
            region.base,
            region.last,
            region.space.name(),
            region.name,
            region.holds
        );
    }
}

fn cmd_pages(_: &str) {
    paging::dump();
}

fn cmd_virt(args: &str) {
    if args.is_empty() {
        let image = mem::image();
        printk!(" address     dir  table offset physical    rights\n");
        for addr in [
            gdt::BASE as u32,
            0x000b_8000,
            image.start,
            image.end,
            mem::KHEAP_BASE,
            mem::VMALLOC_BASE,
            mem::TABLE_WINDOW,
        ] {
            paging::summary(addr);
        }
        printk!("name one address for the full walk, as in `virt 0xc0101000`\n");
        return;
    }
    match klib::parse_u32(args) {
        Some(addr) => paging::describe(addr),
        None => printk!("{}: not an address, try `virt 0xc0101000`\n", args),
    }
}

fn cmd_alloc(_: &str) {
    let mut checks = 0;
    let small = heap::kmalloc(64);
    if small.is_null() {
        return failed("kmalloc(64) returned nothing");
    }
    checks += 1;
    printk!(
        "kmalloc(64)   -> {:#010x}, ksize {}, physical {:#010x}\n",
        small as u32,
        heap::ksize(small),
        paging::translate(small as u32).unwrap_or(0)
    );
    if heap::ksize(small) < 64 {
        return failed("ksize is smaller than the request");
    }
    checks += 1;
    if !pattern(small, 64) {
        return failed("what kmalloc gave back does not hold bytes");
    }
    checks += 1;

    let big = heap::kmalloc(5000);
    if big.is_null() {
        return failed("kmalloc(5000) returned nothing");
    }
    checks += 1;
    let first = paging::translate(big as u32).unwrap_or(0);
    let second = paging::translate(big as u32 + 4096).unwrap_or(0);
    printk!(
        "kmalloc(5000) -> {:#010x}, physical {:#010x} then {:#010x}\n",
        big as u32,
        first,
        second
    );
    if second != first + 4096 {
        return failed("the kmalloc block is not physically contiguous");
    }
    checks += 1;
    if !pattern(big, 5000) {
        return failed("the big block does not hold bytes");
    }
    checks += 1;

    let before = heap::kernel_stats().used;
    heap::kfree(small);
    heap::kfree(big);
    let after = heap::kernel_stats().used;
    printk!("kfree both    -> used bytes {} then {}\n", before, after);
    if after >= before {
        return failed("kfree did not give the bytes back");
    }
    checks += 1;

    let pages = heap::vmalloc(3 * 4096);
    if pages.is_null() {
        return failed("vmalloc(12288) returned nothing");
    }
    checks += 1;
    printk!("vmalloc(12288)-> {:#010x}, vsize {}\n", pages as u32, heap::vsize(pages));
    printk!("  frames");
    let mut scattered = false;
    let mut previous = 0;
    for page in 0..3u32 {
        let at = paging::translate(mem::page_of(pages as u32) + page * 4096).unwrap_or(0);
        printk!(" {:#010x}", at);
        if page > 0 && at != previous + 4096 {
            scattered = true;
        }
        previous = at;
    }
    printk!(
        ", physically {}\n",
        if scattered { "scattered" } else { "in a row this time" }
    );
    if !pattern(pages, 3 * 4096) {
        return failed("the vmalloc range does not hold bytes");
    }
    checks += 1;
    heap::vfree(pages);
    checks += 1;

    let was = heap::kbrk(4096);
    if was.is_null() {
        return failed("kbrk could not grow the heap");
    }
    checks += 1;
    printk!("kbrk(+4096)   -> break was {:#010x}, now {:#010x}\n", was as u32, heap::kbrk(0) as u32);
    if heap::kbrk(-4096).is_null() {
        return failed("kbrk could not give the page back");
    }
    checks += 1;
    printk!("kbrk(-4096)   -> break {:#010x}\n", heap::kbrk(0) as u32);

    let was = heap::vbrk(8192);
    if was.is_null() {
        return failed("vbrk could not grow the vmalloc area");
    }
    checks += 1;
    printk!("vbrk(+8192)   -> break was {:#010x}, now {:#010x}\n", was as u32, heap::vbrk(0) as u32);
    if heap::vbrk(-8192).is_null() {
        return failed("vbrk could not give the pages back");
    }
    checks += 1;
    printk!("vbrk(-8192)   -> break {:#010x}\n", heap::vbrk(0) as u32);

    printk!("alloc: {} checks passed\n", checks);
}

fn failed(why: &str) {
    printk!("alloc: FAILED, {}\n", why);
}

fn pattern(at: *mut u8, size: u32) -> bool {
    for i in 0..size {
        unsafe { write_volatile(at.add(i as usize), (i & 0xff) as u8) };
    }
    for i in 0..size {
        if unsafe { read_volatile(at.add(i as usize)) } != (i & 0xff) as u8 {
            return false;
        }
    }
    true
}

const USER_DEMO: u32 = 0x00c0_0000;

fn cmd_user(_: &str) {
    if let Some(frame) = unsafe { paging::unmap_page(USER_DEMO) } {
        pmm::free_frame(frame);
    }
    let frame = match unsafe { paging::map_new(USER_DEMO, paging::USER_PAGE) } {
        Some(frame) => frame,
        None => return printk!("user: no free frame\n"),
    };
    printk!("mapped {:#010x} -> frame {:#010x} in user space\n", USER_DEMO, frame);
    paging::describe(USER_DEMO);
    unsafe { write_volatile(USER_DEMO as *mut u32, 0x4242_4242) };
    printk!("wrote {:#010x}, read back {:#010x}\n", 0x4242_4242u32, unsafe {
        read_volatile(USER_DEMO as *const u32)
    });
    unsafe { paging::protect(USER_DEMO, paging::USER_READONLY) };
    printk!(
        "now read only: table entry {:#010x}, {}\n",
        paging::entry_of(USER_DEMO).unwrap_or(0),
        paging::rights(paging::entry_of(USER_DEMO).unwrap_or(0))
    );
    printk!("a user page in kernel space is refused:\n");
    unsafe { paging::map_page(mem::KHEAP_BASE + 0x0100_0000, frame, paging::USER_PAGE) };
    if let Some(frame) = unsafe { paging::unmap_page(USER_DEMO) } {
        pmm::free_frame(frame);
    }
    printk!("unmapped {:#010x}, translate says {}\n", USER_DEMO, match paging::translate(USER_DEMO) {
        Some(_) => "still mapped",
        None => "nothing there",
    });
}

fn cmd_fault(args: &str) {
    match args {
        "" | "demand" => {
            printk!("reading {:#010x}, which is not mapped yet\n", mem::DEMAND_BASE);
            let value = unsafe { read_volatile(mem::DEMAND_BASE as *const u32) };
            printk!("the handler mapped it, the read returned {:#010x}\n", value);
        }
        "ro" => {
            let at = mem::image().start;
            printk!("writing to {:#010x}, the kernel's own read only code\n", at);
            unsafe { write_volatile(at as *mut u32, 0) };
        }
        "kernel" => {
            let at = mem::KHEAP_BASE + 0x0180_0000;
            printk!("writing to {:#010x}, kernel space with no page\n", at);
            unsafe { write_volatile(at as *mut u32, 0) };
        }
        _ => printk!("fault: say demand, ro or kernel\n"),
    }
}

fn cmd_panic(args: &str) {
    match args {
        "" | "oops" => koops!("the shell asked for a panic it can walk away from"),
        "fatal" => kpanic!("the shell asked for a panic with no way back"),
        _ => printk!("panic: say oops or fatal\n"),
    }
}

fn cmd_gdt(_: &str) {
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

fn cmd_reboot(_: &str) {
    printk!("rebooting\n");
    unsafe {
        for _ in 0..10 {
            while inb(0x64) & 0x02 != 0 {}
            outb(0x64, 0xfe);
        }
        asm!("lidt [{}]", in(reg) &NULL_IDT, options(readonly, nostack, preserves_flags));
        asm!("int3", options(nostack));
    }
    cmd_halt("");
}

#[repr(C, packed)]
struct Dtr {
    limit: u16,
    base: u32,
}

static NULL_IDT: Dtr = Dtr { limit: 0, base: 0 };

fn cmd_halt(_: &str) {
    printk!("halted\n");
    unsafe { asm!("cli", options(nomem, nostack)) };
    loop {
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}
