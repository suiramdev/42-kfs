use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

use crate::mem::{self, Image, ENTRIES, PAGE_SIZE};
use crate::panic::koops;
use crate::pmm;
use crate::printk::printk;

pub const PRESENT: u32 = 1 << 0;
pub const WRITABLE: u32 = 1 << 1;
pub const USER: u32 = 1 << 2;
pub const WRITE_THROUGH: u32 = 1 << 3;
pub const NO_CACHE: u32 = 1 << 4;
pub const ACCESSED: u32 = 1 << 5;
pub const DIRTY: u32 = 1 << 6;
pub const LARGE: u32 = 1 << 7;
pub const GLOBAL: u32 = 1 << 8;

pub const KERNEL_PAGE: u32 = PRESENT | WRITABLE;
pub const KERNEL_READONLY: u32 = PRESENT;
pub const USER_PAGE: u32 = PRESENT | WRITABLE | USER;
pub const USER_READONLY: u32 = PRESENT | USER;

const FLAGS: u32 = 0xfff;
const FRAME: u32 = !0xfff;

const PAGING_ON: u32 = 1 << 31;
const WRITE_PROTECT: u32 = 1 << 16;

const DIRECTORY: *mut u32 = 0xffff_f000 as *mut u32;

#[repr(align(4096))]
struct Table([u32; ENTRIES]);

static mut KERNEL_TABLE: Table = Table([0; ENTRIES]);

pub fn dir_index(addr: u32) -> usize {
    (addr >> 22) as usize
}

pub fn table_index(addr: u32) -> usize {
    ((addr >> 12) & 0x3ff) as usize
}

pub fn offset(addr: u32) -> u32 {
    addr & (PAGE_SIZE - 1)
}

fn table(dir: usize) -> *mut u32 {
    (mem::TABLE_WINDOW as usize + dir * PAGE_SIZE as usize) as *mut u32
}

pub fn cr0() -> u32 {
    let value: u32;
    unsafe { asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

pub fn cr2() -> u32 {
    let value: u32;
    unsafe { asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

pub fn cr3() -> u32 {
    let value: u32;
    unsafe { asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

pub fn enabled() -> bool {
    cr0() & PAGING_ON != 0
}

pub fn write_protected() -> bool {
    cr0() & WRITE_PROTECT != 0
}

pub fn directory_phys() -> u32 {
    cr3() & FRAME
}

pub fn invalidate(addr: u32) {
    unsafe { asm!("invlpg [{}]", in(reg) addr, options(nostack, preserves_flags)) };
}

pub fn flush() {
    let value = cr3();
    unsafe { asm!("mov cr3, {}", in(reg) value, options(nostack, preserves_flags)) };
}

pub unsafe fn init(image: &Image) {
    let table = addr_of_mut!(KERNEL_TABLE.0) as *mut u32;
    for i in 0..ENTRIES {
        let virt = mem::KERNEL_SPACE + i as u32 * PAGE_SIZE;
        let phys = i as u32 * PAGE_SIZE;
        let readonly = virt >= image.start && virt < image.readonly_end;
        let flags = if readonly { KERNEL_READONLY } else { KERNEL_PAGE };
        write_volatile(table.add(i), phys | flags);
    }
    let phys = mem::kernel_phys(addr_of!(KERNEL_TABLE.0) as u32);
    write_volatile(DIRECTORY.add(dir_index(mem::KERNEL_SPACE)), phys | KERNEL_PAGE);
    flush();
    let value = cr0() | WRITE_PROTECT;
    asm!("mov cr0, {}", in(reg) value, options(nostack, preserves_flags));
}

pub fn directory_entry(dir: usize) -> u32 {
    unsafe { read_volatile(DIRECTORY.add(dir)) }
}

pub unsafe fn get_page(addr: u32, create: bool) -> Option<*mut u32> {
    let dir = dir_index(addr);
    if directory_entry(dir) & PRESENT == 0 {
        if !create {
            return None;
        }
        let frame = pmm::alloc_frame()?;
        let mut flags = KERNEL_PAGE;
        if !mem::is_kernel(addr) {
            flags |= USER;
        }
        write_volatile(DIRECTORY.add(dir), frame | flags);
        invalidate(table(dir) as u32);
        let fresh = table(dir);
        for i in 0..ENTRIES {
            write_volatile(fresh.add(i), 0);
        }
    }
    Some(table(dir).add(table_index(addr)))
}

pub fn entry_of(addr: u32) -> Option<u32> {
    let entry = unsafe { get_page(addr, false)? };
    let value = unsafe { read_volatile(entry) };
    if value & PRESENT == 0 {
        return None;
    }
    Some(value)
}

pub fn translate(addr: u32) -> Option<u32> {
    Some((entry_of(addr)? & FRAME) | offset(addr))
}

pub unsafe fn map_page(addr: u32, frame: u32, flags: u32) -> bool {
    if flags & USER != 0 && mem::is_kernel(addr) {
        koops!("{:#010x} is kernel space, it takes no user page", addr);
        return false;
    }
    let entry = match get_page(addr, true) {
        Some(entry) => entry,
        None => {
            koops!("no free frame for the page table of {:#010x}", addr);
            return false;
        }
    };
    if read_volatile(entry) & PRESENT != 0 {
        koops!("{:#010x} is mapped already", addr);
        return false;
    }
    write_volatile(entry, (frame & FRAME) | flags | PRESENT);
    invalidate(addr);
    true
}

pub unsafe fn map_new(addr: u32, flags: u32) -> Option<u32> {
    let frame = pmm::alloc_frame()?;
    if !map_page(addr, frame, flags | WRITABLE) {
        pmm::free_frame(frame);
        return None;
    }
    let page = mem::page_of(addr) as *mut u32;
    for i in 0..(PAGE_SIZE / 4) as usize {
        write_volatile(page.add(i), 0);
    }
    if flags & WRITABLE == 0 {
        protect(addr, flags);
    }
    Some(frame)
}

pub unsafe fn unmap_page(addr: u32) -> Option<u32> {
    let entry = get_page(addr, false)?;
    let value = read_volatile(entry);
    if value & PRESENT == 0 {
        return None;
    }
    write_volatile(entry, 0);
    invalidate(addr);
    Some(value & FRAME)
}

pub unsafe fn protect(addr: u32, flags: u32) -> bool {
    let entry = match get_page(addr, false) {
        Some(entry) => entry,
        None => return false,
    };
    let value = read_volatile(entry);
    if value & PRESENT == 0 {
        return false;
    }
    write_volatile(entry, (value & FRAME) | flags | PRESENT);
    invalidate(addr);
    true
}

pub fn rights(entry: u32) -> &'static str {
    match (entry & WRITABLE != 0, entry & USER != 0) {
        (true, true) => "rw user",
        (false, true) => "r  user",
        (true, false) => "rw kernel",
        (false, false) => "r  kernel",
    }
}

pub fn handle_fault(addr: u32, error: u32) -> bool {
    if error & PRESENT != 0 || !mem::is_demand(addr) {
        return false;
    }
    let page = mem::page_of(addr);
    unsafe { map_new(page, USER_PAGE) }.is_some()
}

pub fn print_flags(entry: u32) {
    printk!(
        "    p {} rw {} us {} a {} d {} ps {} g {} pwt {} pcd {}\n",
        bit(entry, PRESENT),
        bit(entry, WRITABLE),
        bit(entry, USER),
        bit(entry, ACCESSED),
        bit(entry, DIRTY),
        bit(entry, LARGE),
        bit(entry, GLOBAL),
        bit(entry, WRITE_THROUGH),
        bit(entry, NO_CACHE)
    );
}

fn bit(entry: u32, mask: u32) -> u32 {
    (entry & mask != 0) as u32
}

pub fn summary(addr: u32) {
    printk!(
        " {:#010x}  {:4} {:5} {:6} ",
        addr,
        dir_index(addr),
        table_index(addr),
        offset(addr)
    );
    match entry_of(addr) {
        Some(entry) => printk!("{:#010x}  {}\n", (entry & FRAME) | offset(addr), rights(entry)),
        None => printk!("nothing      not mapped\n"),
    }
}

pub fn describe(addr: u32) {
    let dir = dir_index(addr);
    let index = table_index(addr);
    printk!(
        "{:#010x}: directory {} table {} offset {} in {} space\n",
        addr,
        dir,
        index,
        offset(addr),
        mem::space_of(addr).name()
    );
    let pde = directory_entry(dir);
    if pde & PRESENT == 0 {
        printk!("  directory entry {:#010x}, not present\n", pde);
        return;
    }
    printk!("  directory entry {:#010x} -> table at {:#010x}, {}\n", pde, pde & FRAME, rights(pde));
    print_flags(pde);
    let pte = unsafe { read_volatile(table(dir).add(index)) };
    if pte & PRESENT == 0 {
        printk!("  table entry     {:#010x}, not present\n", pte);
        return;
    }
    printk!("  table entry     {:#010x} -> frame at {:#010x}, {}\n", pte, pte & FRAME, rights(pte));
    print_flags(pte);
    printk!("  physical        {:#010x}\n", (pte & FRAME) | offset(addr));
}

pub fn dump() {
    printk!(
        "cr3 {:#010x}, paging {}, write protect {}\n",
        directory_phys(),
        if enabled() { "on" } else { "off" },
        if write_protected() { "on" } else { "off" }
    );
    printk!(" virtual                  physical      pages  rights\n");
    let mut run: Option<(u32, u32, u32, u32)> = None;
    for dir in 0..ENTRIES {
        let pde = directory_entry(dir);
        if pde & PRESENT == 0 {
            run = extend_run(run, None);
            continue;
        }
        for index in 0..ENTRIES {
            let pte = unsafe { read_volatile(table(dir).add(index)) };
            let virt = (dir as u32) << 22 | (index as u32) << 12;
            if pte & PRESENT == 0 {
                run = extend_run(run, None);
                continue;
            }
            run = extend_run(run, Some((virt, pte & FRAME, pte & FLAGS)));
        }
    }
    extend_run(run, None);
}

fn extend_run(
    run: Option<(u32, u32, u32, u32)>,
    next: Option<(u32, u32, u32)>,
) -> Option<(u32, u32, u32, u32)> {
    match (run, next) {
        (Some((virt, phys, flags, pages)), Some((at, frame, bits))) => {
            let follows = at == virt.wrapping_add(pages * PAGE_SIZE)
                && frame == phys.wrapping_add(pages * PAGE_SIZE)
                && bits & (WRITABLE | USER) == flags & (WRITABLE | USER);
            if follows {
                return Some((virt, phys, flags, pages + 1));
            }
            print_run(virt, phys, flags, pages);
            Some((at, frame, bits, 1))
        }
        (Some((virt, phys, flags, pages)), None) => {
            print_run(virt, phys, flags, pages);
            None
        }
        (None, Some((at, frame, bits))) => Some((at, frame, bits, 1)),
        (None, None) => None,
    }
}

fn print_run(virt: u32, phys: u32, flags: u32, pages: u32) {
    let span = pages * PAGE_SIZE;
    printk!(
        " {:#010x}..{:#010x} {:#010x}  {:5}  {}\n",
        virt,
        virt.wrapping_add(span - 1),
        phys,
        pages,
        rights(flags)
    );
}
