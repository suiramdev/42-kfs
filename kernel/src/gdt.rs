use core::arch::asm;
use core::ptr::write_volatile;

pub const BASE: usize = 0x0000_0800;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
pub const KERNEL_STACK: u16 = 0x18;

pub struct Segment {
    pub name: &'static str,
    pub selector: u16,
    pub descriptor: u64,
}

const KERNEL_EXEC: u8 = 0x9a;
const KERNEL_RW: u8 = 0x92;
const USER_EXEC: u8 = 0xfa;
const USER_RW: u8 = 0xf2;

pub const SEGMENTS: [Segment; 7] = [
    Segment { name: "null", selector: 0x00, descriptor: 0 },
    Segment { name: "kernel code", selector: KERNEL_CODE, descriptor: flat(KERNEL_EXEC) },
    Segment { name: "kernel data", selector: KERNEL_DATA, descriptor: flat(KERNEL_RW) },
    Segment { name: "kernel stack", selector: KERNEL_STACK, descriptor: flat(KERNEL_RW) },
    Segment { name: "user code", selector: 0x20 | 3, descriptor: flat(USER_EXEC) },
    Segment { name: "user data", selector: 0x28 | 3, descriptor: flat(USER_RW) },
    Segment { name: "user stack", selector: 0x30 | 3, descriptor: flat(USER_RW) },
];

pub const LIMIT: u16 = (SEGMENTS.len() * core::mem::size_of::<u64>() - 1) as u16;

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u32,
}

const fn flat(access: u8) -> u64 {
    0xffff | ((access as u64) << 40) | (0xcf << 48)
}

pub unsafe fn install() {
    for (i, segment) in SEGMENTS.iter().enumerate() {
        unsafe { write_volatile((BASE as *mut u64).add(i), segment.descriptor) };
    }
    let gdtr = Gdtr { limit: LIMIT, base: BASE as u32 };
    unsafe {
        asm!(
            "lgdt [{gdtr}]",
            "mov ds, {data:x}",
            "mov es, {data:x}",
            "mov fs, {data:x}",
            "mov gs, {data:x}",
            "mov ss, {stack:x}",
            "push {code:e}",
            "lea {next:e}, [2f]",
            "push {next:e}",
            "retf",
            "2:",
            gdtr = in(reg) &gdtr,
            data = in(reg) KERNEL_DATA as u32,
            stack = in(reg) KERNEL_STACK as u32,
            code = in(reg) KERNEL_CODE as u32,
            next = out(reg) _,
        );
    }
}

pub struct Gdtr32 {
    pub base: u32,
    pub limit: u16,
}

pub fn current() -> Gdtr32 {
    let mut gdtr = Gdtr { limit: 0, base: 0 };
    unsafe { asm!("sgdt [{}]", in(reg) &mut gdtr) };
    Gdtr32 { base: gdtr.base, limit: gdtr.limit }
}

pub fn selectors() -> (u16, u16, u16) {
    let (cs, ss, ds): (u16, u16, u16);
    unsafe {
        asm!("mov {:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        asm!("mov {:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
        asm!("mov {:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
    }
    (cs, ss, ds)
}
