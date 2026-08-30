use crate::mem::{PAGE_SHIFT, PAGE_SIZE};
use crate::multiboot::Ram;

pub const MAX_FRAMES: usize = 1 << 20;
const WORDS: usize = MAX_FRAMES / 32;

static mut FREE: [u32; WORDS] = [0; WORDS];
static mut TOTAL: usize = 0;
static mut AVAILABLE: usize = 0;

fn frame_of(phys: u32) -> usize {
    (phys >> PAGE_SHIFT) as usize
}

fn address_of(frame: usize) -> u32 {
    (frame as u32) << PAGE_SHIFT
}

fn free_bit(frame: usize) -> bool {
    frame < MAX_FRAMES && unsafe { FREE[frame / 32] } & (1 << (frame % 32)) != 0
}

fn set_free(frame: usize) {
    if frame >= MAX_FRAMES || free_bit(frame) {
        return;
    }
    unsafe {
        FREE[frame / 32] |= 1 << (frame % 32);
        AVAILABLE += 1;
    }
}

fn set_used(frame: usize) {
    if frame >= MAX_FRAMES || !free_bit(frame) {
        return;
    }
    unsafe {
        FREE[frame / 32] &= !(1 << (frame % 32));
        AVAILABLE -= 1;
    }
}

pub fn init(ram: &Ram) {
    for region in &ram.regions[..ram.count] {
        let first = frame_of(region.base + PAGE_SIZE - 1);
        let last = frame_of(region.end);
        for frame in first..last {
            if frame >= MAX_FRAMES {
                break;
            }
            if !free_bit(frame) {
                unsafe { TOTAL += 1 };
                set_free(frame);
            }
        }
    }
}

pub fn reserve(start: u32, end: u32) {
    let first = frame_of(start);
    let last = frame_of(end + PAGE_SIZE - 1);
    for frame in first..last {
        set_used(frame);
    }
}

pub fn total_frames() -> usize {
    unsafe { TOTAL }
}

pub fn free_frames() -> usize {
    unsafe { AVAILABLE }
}

pub fn used_frames() -> usize {
    total_frames() - free_frames()
}

pub fn alloc_frame() -> Option<u32> {
    for word in 0..WORDS {
        let bits = unsafe { FREE[word] };
        if bits == 0 {
            continue;
        }
        let frame = word * 32 + bits.trailing_zeros() as usize;
        set_used(frame);
        return Some(address_of(frame));
    }
    None
}

pub fn alloc_range(count: usize) -> Option<u32> {
    let base = find_run(count)?;
    for frame in base..base + count {
        set_used(frame);
    }
    Some(address_of(base))
}

pub fn free_frame(phys: u32) {
    set_free(frame_of(phys));
}

pub fn carve(size: u32) -> Option<(u32, u32)> {
    let mut pages = (size / PAGE_SIZE) as usize;
    while pages >= 16 {
        if let Some(base) = alloc_range(pages) {
            return Some((base, base + pages as u32 * PAGE_SIZE));
        }
        pages /= 2;
    }
    None
}

pub fn largest_run() -> usize {
    let mut best = 0;
    let mut run = 0;
    for frame in 0..MAX_FRAMES {
        if free_bit(frame) {
            run += 1;
            if run > best {
                best = run;
            }
        } else {
            run = 0;
        }
    }
    best
}

fn find_run(count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let mut frame = 0;
    while frame + count <= MAX_FRAMES {
        if !free_bit(frame) {
            frame += 1;
            continue;
        }
        let mut span = 1;
        while span < count && free_bit(frame + span) {
            span += 1;
        }
        if span == count {
            return Some(frame);
        }
        frame += span + 1;
    }
    None
}
