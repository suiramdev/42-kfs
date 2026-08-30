use core::ptr::addr_of;

use crate::multiboot::{self, Ram, NOTHING};
use crate::panic::kpanic;
use crate::printk::printk;
use crate::{heap, paging, pmm};

pub const PAGE_SIZE: u32 = 4096;
pub const PAGE_SHIFT: u32 = 12;
pub const ENTRIES: usize = 1024;
pub const TABLE_SPAN: u32 = PAGE_SIZE * ENTRIES as u32;

pub const LOW_WINDOW: u32 = 0x0000_0000;
pub const LOW_WINDOW_END: u32 = 0x0040_0000;

pub const USER_BASE: u32 = LOW_WINDOW_END;
pub const USER_END: u32 = KERNEL_SPACE;

pub const DEMAND_BASE: u32 = 0x0080_0000;
pub const DEMAND_END: u32 = 0x0090_0000;

pub const KERNEL_SPACE: u32 = 0xC000_0000;
pub const KERNEL_WINDOW_END: u32 = KERNEL_SPACE + TABLE_SPAN;

pub const KHEAP_BASE: u32 = 0xD000_0000;
pub const KHEAP_LIMIT: u32 = KHEAP_BASE + 0x0200_0000;

pub const VMALLOC_BASE: u32 = 0xE000_0000;
pub const VMALLOC_LIMIT: u32 = VMALLOC_BASE + 0x1000_0000;

pub const TABLE_WINDOW: u32 = 0xFFC0_0000;

const BIOS_END: u32 = 0x0010_0000;

#[derive(Clone, Copy, PartialEq)]
pub enum Space {
    Low,
    User,
    Kernel,
}

impl Space {
    pub fn name(self) -> &'static str {
        match self {
            Space::Low => "kernel",
            Space::User => "user",
            Space::Kernel => "kernel",
        }
    }
}

pub struct Named {
    pub name: &'static str,
    pub base: u32,
    pub last: u32,
    pub space: Space,
    pub holds: &'static str,
}

pub static LAYOUT: [Named; 8] = [
    Named {
        name: "low window",
        base: LOW_WINDOW,
        last: LOW_WINDOW_END - 1,
        space: Space::Low,
        holds: "identity: gdt, vga, kernel image",
    },
    Named {
        name: "user space",
        base: USER_BASE,
        last: DEMAND_BASE - 1,
        space: Space::User,
        holds: "free for user pages",
    },
    Named {
        name: "user demand",
        base: DEMAND_BASE,
        last: DEMAND_END - 1,
        space: Space::User,
        holds: "paged in by the fault handler",
    },
    Named {
        name: "user space",
        base: DEMAND_END,
        last: USER_END - 1,
        space: Space::User,
        holds: "free for user pages",
    },
    Named {
        name: "kernel image",
        base: KERNEL_SPACE,
        last: KERNEL_WINDOW_END - 1,
        space: Space::Kernel,
        holds: "ram 0..4mb, kernel code read only",
    },
    Named {
        name: "kernel heap",
        base: KHEAP_BASE,
        last: KHEAP_LIMIT - 1,
        space: Space::Kernel,
        holds: "kmalloc, contiguous physical",
    },
    Named {
        name: "vmalloc",
        base: VMALLOC_BASE,
        last: VMALLOC_LIMIT - 1,
        space: Space::Kernel,
        holds: "vmalloc, scattered frames",
    },
    Named {
        name: "page tables",
        base: TABLE_WINDOW,
        last: u32::MAX,
        space: Space::Kernel,
        holds: "the directory mapped into itself",
    },
];

extern "C" {
    #[link_name = "boot_start"]
    static BOOT_START: u8;
    #[link_name = "kernel_start"]
    static KERNEL_START: u8;
    #[link_name = "kernel_code_end"]
    static KERNEL_CODE_END: u8;
    #[link_name = "kernel_readonly_end"]
    static KERNEL_READONLY_END: u8;
    #[link_name = "kernel_end"]
    static KERNEL_END: u8;
}

pub fn load_phys() -> u32 {
    addr_of!(BOOT_START) as u32
}

#[derive(Clone, Copy)]
pub struct Image {
    pub start: u32,
    pub code_end: u32,
    pub readonly_end: u32,
    pub end: u32,
}

pub fn image() -> Image {
    Image {
        start: addr_of!(KERNEL_START) as u32,
        code_end: addr_of!(KERNEL_CODE_END) as u32,
        readonly_end: addr_of!(KERNEL_READONLY_END) as u32,
        end: addr_of!(KERNEL_END) as u32,
    }
}

pub fn space_of(addr: u32) -> Space {
    if addr < LOW_WINDOW_END {
        Space::Low
    } else if addr < KERNEL_SPACE {
        Space::User
    } else {
        Space::Kernel
    }
}

pub fn is_kernel(addr: u32) -> bool {
    space_of(addr) != Space::User
}

pub fn is_demand(addr: u32) -> bool {
    addr >= DEMAND_BASE && addr < DEMAND_END
}

pub fn kernel_phys(addr: u32) -> u32 {
    addr - KERNEL_SPACE
}

pub fn page_of(addr: u32) -> u32 {
    addr & !(PAGE_SIZE - 1)
}

static mut RAM: Ram = NOTHING;
static mut WINDOW: (u32, u32) = (0, 0);

pub fn ram() -> Ram {
    unsafe { RAM }
}

pub fn heap_window() -> (u32, u32) {
    unsafe { WINDOW }
}

pub fn init(magic: u32, info: u32) {
    let ram = match multiboot::probe(magic, info) {
        Ok(ram) => ram,
        Err(why) => kpanic!("{}", why),
    };
    unsafe { RAM = ram };

    let image = image();
    if image.end > KERNEL_WINDOW_END {
        kpanic!("the kernel image ends at {:#010x}, past its 4 mb window", image.end);
    }
    pmm::init(&ram);
    pmm::reserve(0, BIOS_END);
    pmm::reserve(load_phys(), kernel_phys(image.end));

    unsafe { paging::init(&image) };

    let want = core::cmp::min((KHEAP_LIMIT - KHEAP_BASE) as u64, ram.usable / 4) as u32;
    let window = match pmm::carve(page_of(want)) {
        Some(window) => window,
        None => kpanic!("no contiguous physical block left for the kernel heap"),
    };
    unsafe { WINDOW = window };
    heap::init(window);

    printk!(
        "memory: {} kb usable in {} regions, {} frames of {} kb\n",
        ram.usable / 1024,
        ram.count,
        pmm::total_frames(),
        PAGE_SIZE / 1024
    );
    printk!(
        "paging: directory at {:#010x}, {} kb carved at {:#010x} for kmalloc\n",
        paging::directory_phys(),
        (window.1 - window.0) / 1024,
        window.0
    );
}
