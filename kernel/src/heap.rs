use core::ptr::{addr_of_mut, null_mut, read_volatile, write_volatile};

use crate::mem::{self, PAGE_SIZE};
use crate::paging;
use crate::panic::koops;
use crate::pmm;

const HEADER: u32 = 8;
const ALIGN: u32 = 8;
const SMALLEST: u32 = 8;

#[derive(Clone, Copy, PartialEq)]
pub enum Backing {
    Contiguous,
    Scattered,
}

pub struct Heap {
    pub name: &'static str,
    pub base: u32,
    pub limit: u32,
    pub brk: u32,
    pub backing: Backing,
    pub phys_base: u32,
    pub phys_end: u32,
    flags: u32,
}

#[derive(Clone, Copy)]
pub struct Stats {
    pub name: &'static str,
    pub base: u32,
    pub brk: u32,
    pub limit: u32,
    pub used: u32,
    pub free: u32,
    pub blocks: u32,
    pub live: u32,
    pub contiguous: bool,
    pub phys_base: u32,
    pub phys_end: u32,
}

fn size_at(at: u32) -> u32 {
    unsafe { read_volatile(at as *const u32) }
}

fn used_at(at: u32) -> bool {
    let flag: u32 = unsafe { read_volatile((at + 4) as *const u32) };
    flag != 0
}

fn write_header(at: u32, size: u32, used: bool) {
    unsafe {
        write_volatile(at as *mut u32, size);
        write_volatile((at + 4) as *mut u32, used as u32);
    }
}

fn round_up(size: u32) -> u32 {
    let size = if size < SMALLEST { SMALLEST } else { size };
    (size + ALIGN - 1) & !(ALIGN - 1)
}

impl Heap {
    pub const fn new(
        name: &'static str,
        base: u32,
        limit: u32,
        backing: Backing,
        flags: u32,
    ) -> Heap {
        Heap { name, base, limit, brk: base, backing, phys_base: 0, phys_end: 0, flags }
    }

    pub fn attach(&mut self, window: (u32, u32)) {
        self.phys_base = window.0;
        self.phys_end = window.1;
    }

    fn frame_for(&self, at: u32) -> Option<u32> {
        match self.backing {
            Backing::Contiguous => {
                let frame = self.phys_base + (at - self.base);
                if frame + PAGE_SIZE > self.phys_end {
                    koops!("{} ran out of its physical block", self.name);
                    return None;
                }
                Some(frame)
            }
            Backing::Scattered => match pmm::alloc_frame() {
                Some(frame) => Some(frame),
                None => {
                    koops!("{} found no free physical frame", self.name);
                    None
                }
            },
        }
    }

    fn grow(&mut self, pages: u32) -> bool {
        let start = self.brk;
        let mut at = start;
        for _ in 0..pages {
            if at + PAGE_SIZE > self.limit {
                koops!("{} reached the end of its address range", self.name);
                break;
            }
            let frame = match self.frame_for(at) {
                Some(frame) => frame,
                None => break,
            };
            if !unsafe { paging::map_page(at, frame, self.flags) } {
                if self.backing == Backing::Scattered {
                    pmm::free_frame(frame);
                }
                break;
            }
            at += PAGE_SIZE;
        }
        if at == start {
            return false;
        }
        let tail = self.last_block(start);
        self.brk = at;
        match tail {
            Some(block) if !used_at(block) => {
                write_header(block, size_at(block) + (at - start), false);
            }
            _ => write_header(start, (at - start) - HEADER, false),
        }
        true
    }

    fn shrink(&mut self, pages: u32) -> bool {
        let span = core::cmp::min(pages * PAGE_SIZE, self.brk - self.base);
        let target = self.brk - span;
        if span == 0 {
            return true;
        }
        if let Some(block) = self.last_block(self.brk) {
            if used_at(block) || target < block {
                koops!("the top of {} is in use, it cannot shrink", self.name);
                return false;
            }
            if target > block {
                if target < block + HEADER + SMALLEST {
                    koops!("the free top of {} is too small to give back", self.name);
                    return false;
                }
                write_header(block, target - block - HEADER, false);
            }
        }
        let mut at = target;
        while at < self.brk {
            if let Some(frame) = unsafe { paging::unmap_page(at) } {
                if self.backing == Backing::Scattered {
                    pmm::free_frame(frame);
                }
            }
            at += PAGE_SIZE;
        }
        self.brk = target;
        true
    }

    pub fn brk(&mut self, delta: i32) -> *mut u8 {
        let was = self.brk;
        if delta == 0 {
            return was as *mut u8;
        }
        let pages = (delta.unsigned_abs() + PAGE_SIZE - 1) / PAGE_SIZE;
        let moved = if delta > 0 { self.grow(pages) } else { self.shrink(pages) };
        if moved {
            was as *mut u8
        } else {
            null_mut()
        }
    }

    fn last_block(&self, end: u32) -> Option<u32> {
        let mut at = self.base;
        let mut last = None;
        while at + HEADER <= end {
            last = Some(at);
            at += HEADER + size_at(at);
        }
        last
    }

    fn find_block(&self, payload: u32) -> Option<u32> {
        let mut at = self.base;
        while at + HEADER <= self.brk {
            if at + HEADER == payload {
                return Some(at);
            }
            at += HEADER + size_at(at);
        }
        None
    }

    fn first_fit(&mut self, need: u32) -> Option<u32> {
        let mut at = self.base;
        while at + HEADER <= self.brk {
            let mut size = size_at(at);
            if !used_at(at) {
                loop {
                    let next = at + HEADER + size;
                    if next + HEADER > self.brk || used_at(next) {
                        break;
                    }
                    size += HEADER + size_at(next);
                    write_header(at, size, false);
                }
                if size >= need {
                    return Some(at);
                }
            }
            at += HEADER + size;
        }
        None
    }

    fn take(&mut self, block: u32, need: u32) -> *mut u8 {
        let size = size_at(block);
        if size >= need + HEADER + SMALLEST {
            write_header(block, need, true);
            write_header(block + HEADER + need, size - need - HEADER, false);
        } else {
            write_header(block, size, true);
        }
        (block + HEADER) as *mut u8
    }

    pub fn alloc(&mut self, want: u32) -> *mut u8 {
        if want == 0 {
            return null_mut();
        }
        if want > self.limit - self.base - HEADER {
            koops!("{} bytes is more than {} can ever hold", want, self.name);
            return null_mut();
        }
        let need = round_up(want);
        if let Some(block) = self.first_fit(need) {
            return self.take(block, need);
        }
        let pages = (need + HEADER + PAGE_SIZE - 1) / PAGE_SIZE;
        if !self.grow(pages) {
            return null_mut();
        }
        match self.first_fit(need) {
            Some(block) => self.take(block, need),
            None => null_mut(),
        }
    }

    pub fn free(&mut self, pointer: *mut u8) {
        if pointer.is_null() {
            return;
        }
        let payload = pointer as u32;
        let block = match self.find_block(payload) {
            Some(block) => block,
            None => {
                koops!("{:#010x} did not come from {}", payload, self.name);
                return;
            }
        };
        if !used_at(block) {
            koops!("{:#010x} is freed twice in {}", payload, self.name);
            return;
        }
        write_header(block, size_at(block), false);
    }

    pub fn size(&self, pointer: *mut u8) -> u32 {
        if pointer.is_null() {
            return 0;
        }
        let payload = pointer as u32;
        match self.find_block(payload) {
            Some(block) => size_at(block),
            None => {
                koops!("{:#010x} did not come from {}", payload, self.name);
                0
            }
        }
    }

    pub fn stats(&self) -> Stats {
        let mut stats = Stats {
            name: self.name,
            base: self.base,
            brk: self.brk,
            limit: self.limit,
            used: 0,
            free: 0,
            blocks: 0,
            live: 0,
            contiguous: self.backing == Backing::Contiguous,
            phys_base: self.phys_base,
            phys_end: self.phys_end,
        };
        let mut at = self.base;
        while at + HEADER <= self.brk {
            let size = size_at(at);
            stats.blocks += 1;
            if used_at(at) {
                stats.used += size;
                stats.live += 1;
            } else {
                stats.free += size;
            }
            at += HEADER + size;
        }
        stats
    }
}

static mut KERNEL_HEAP: Heap = Heap::new(
    "kmalloc",
    mem::KHEAP_BASE,
    mem::KHEAP_LIMIT,
    Backing::Contiguous,
    paging::KERNEL_PAGE,
);

static mut VIRTUAL_HEAP: Heap = Heap::new(
    "vmalloc",
    mem::VMALLOC_BASE,
    mem::VMALLOC_LIMIT,
    Backing::Scattered,
    paging::KERNEL_PAGE,
);

fn kernel_heap() -> &'static mut Heap {
    unsafe { &mut *addr_of_mut!(KERNEL_HEAP) }
}

fn virtual_heap() -> &'static mut Heap {
    unsafe { &mut *addr_of_mut!(VIRTUAL_HEAP) }
}

pub fn init(window: (u32, u32)) {
    kernel_heap().attach(window);
}

pub fn kmalloc(size: u32) -> *mut u8 {
    kernel_heap().alloc(size)
}

pub fn kfree(pointer: *mut u8) {
    kernel_heap().free(pointer)
}

pub fn ksize(pointer: *mut u8) -> u32 {
    kernel_heap().size(pointer)
}

pub fn kbrk(delta: i32) -> *mut u8 {
    kernel_heap().brk(delta)
}

pub fn vmalloc(size: u32) -> *mut u8 {
    virtual_heap().alloc(size)
}

pub fn vfree(pointer: *mut u8) {
    virtual_heap().free(pointer)
}

pub fn vsize(pointer: *mut u8) -> u32 {
    virtual_heap().size(pointer)
}

pub fn vbrk(delta: i32) -> *mut u8 {
    virtual_heap().brk(delta)
}

pub fn kernel_stats() -> Stats {
    kernel_heap().stats()
}

pub fn virtual_stats() -> Stats {
    virtual_heap().stats()
}
