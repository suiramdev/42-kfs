use core::ptr::{addr_of, read_unaligned, read_volatile};

pub const BOOT_MAGIC: u32 = 0x2BAD_B002;

const FLAG_MEMORY: u32 = 1 << 0;
const FLAG_MMAP: u32 = 1 << 6;

const AVAILABLE: u32 = 1;

pub const MAX_REGIONS: usize = 12;

const REACHABLE: u32 = 0x0040_0000;

#[repr(C)]
struct Info {
    flags: u32,
    mem_lower: u32,
    mem_upper: u32,
    boot_device: u32,
    cmdline: u32,
    mods_count: u32,
    mods_addr: u32,
    syms: [u32; 4],
    mmap_length: u32,
    mmap_addr: u32,
}

#[repr(C, packed)]
struct MmapEntry {
    size: u32,
    base: u64,
    length: u64,
    kind: u32,
}

#[derive(Clone, Copy)]
pub struct Region {
    pub base: u32,
    pub end: u32,
}

const EMPTY: Region = Region { base: 0, end: 0 };

#[derive(Clone, Copy)]
pub struct Ram {
    pub regions: [Region; MAX_REGIONS],
    pub count: usize,
    pub usable: u64,
    pub highest: u32,
    pub source: &'static str,
}

pub const NOTHING: Ram = Ram {
    regions: [EMPTY; MAX_REGIONS],
    count: 0,
    usable: 0,
    highest: 0,
    source: "none",
};

impl Ram {
    fn push(&mut self, base: u64, length: u64) {
        if self.count == MAX_REGIONS || length == 0 {
            return;
        }
        let top = base.saturating_add(length);
        if base >= u32::MAX as u64 {
            return;
        }
        let base = base as u32;
        let end = if top > u32::MAX as u64 { u32::MAX & !0xfff } else { top as u32 };
        if end <= base {
            return;
        }
        self.regions[self.count] = Region { base, end };
        self.count += 1;
        self.usable += (end - base) as u64;
        if end > self.highest {
            self.highest = end;
        }
    }
}

pub fn probe(magic: u32, info: u32) -> Result<Ram, &'static str> {
    if magic != BOOT_MAGIC {
        return Err("no multiboot magic in eax, this kernel needs a multiboot loader");
    }
    if info == 0 || info >= REACHABLE {
        return Err("the multiboot information is out of the low window");
    }
    let mut ram = NOTHING;
    let header = info as *const Info;
    let flags = unsafe { read_volatile(&(*header).flags) };
    if flags & FLAG_MMAP != 0 {
        let length = unsafe { read_volatile(&(*header).mmap_length) };
        let start = unsafe { read_volatile(&(*header).mmap_addr) };
        if start != 0 && start < REACHABLE {
            ram.source = "multiboot memory map";
            let mut at = start;
            let stop = start.saturating_add(length);
            while at + 8 <= stop {
                let entry = at as *const MmapEntry;
                let size = unsafe { read_unaligned(addr_of!((*entry).size)) };
                let kind = unsafe { read_unaligned(addr_of!((*entry).kind)) };
                let base = unsafe { read_unaligned(addr_of!((*entry).base)) };
                let len = unsafe { read_unaligned(addr_of!((*entry).length)) };
                if kind == AVAILABLE {
                    ram.push(base, len);
                }
                if size == 0 {
                    break;
                }
                at += size + 4;
            }
        }
    }
    if ram.count == 0 && flags & FLAG_MEMORY != 0 {
        ram.source = "multiboot mem_lower and mem_upper";
        let lower = unsafe { read_volatile(addr_of!((*header).mem_lower)) } as u64;
        let upper = unsafe { read_volatile(addr_of!((*header).mem_upper)) } as u64;
        ram.push(0, lower * 1024);
        ram.push(0x0010_0000, upper * 1024);
    }
    if ram.count == 0 {
        return Err("the loader described no usable memory");
    }
    Ok(ram)
}
