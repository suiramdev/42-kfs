//! Global Descriptor Table — see `docs/GDT.md`.
//!
//! Multiboot hands the kernel five segment registers that already work,
//! and one warning: "the GDTR may be invalid, so the OS image must not
//! load any segment registers (even just reloading the same values!)
//! until it sets up its own GDT". Every segment register carries a
//! hidden copy of its descriptor, so GRUB's segments keep working after
//! GRUB's table is gone — right up to the first instruction that reloads
//! one. Owning the table is what makes that instruction safe.
//!
//! The table lives at physical 0x800, the address the subject fixes. It
//! is copied there rather than linked there: `linker.ld` pins the image
//! at 1 MiB, and asking GRUB to load a second segment into the first
//! page would put it in the middle of memory the BIOS and GRUB itself
//! are still using while the load happens. 56 bytes of data move down
//! after the handoff instead; all executable code stays at 1 MiB.
//!
//! All six segments are flat: base 0, limit 4 GiB. They differ only in
//! the access byte, which is what carries the type and the ring. A
//! narrower stack segment is expressible on x86 (an expand-down data
//! descriptor) but not usable here: LLVM assumes `ss` and `ds` share a
//! base, so any address taken from the stack and dereferenced through
//! `ds` would resolve somewhere else.

use core::arch::asm;
use core::ptr::write_volatile;

/// Where the subject demands the table sit. 2048 is 8-byte aligned as
/// Intel recommends, above the BIOS data area at 0x400, and inside the
/// 0x500..0x7BFF window the BIOS leaves free.
pub const BASE: usize = 0x0000_0800;

/// A selector is an index into the table, shifted left 3, plus the
/// requested privilege level in the low two bits. Kernel selectors ask
/// for ring 0, so they are the bare offsets; user selectors carry a 3.
pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
pub const KERNEL_STACK: u16 = 0x18;

/// One row of the table. The descriptor is the 8 bytes the CPU reads;
/// the name and selector exist so `shell`'s `gdt` command can print the
/// table from the same source of truth the CPU is using.
pub struct Segment {
    pub name: &'static str,
    pub selector: u16,
    pub descriptor: u64,
}

/// Access byte: present, ring in bits 6-5, code/data in bit 4, then
/// executable, direction/conforming, readable/writable, accessed.
const KERNEL_EXEC: u8 = 0x9a;
const KERNEL_RW: u8 = 0x92;
const USER_EXEC: u8 = 0xfa;
const USER_RW: u8 = 0xf2;

/// The table itself, in the order the subject lists it. Entry 0 must be
/// eight zero bytes: the CPU refuses to load a selector of 0, which is
/// what turns an uninitialised segment register into a fault instead of
/// a silent read of whatever entry 0 described.
pub const SEGMENTS: [Segment; 7] = [
    Segment { name: "null", selector: 0x00, descriptor: 0 },
    Segment { name: "kernel code", selector: KERNEL_CODE, descriptor: flat(KERNEL_EXEC) },
    Segment { name: "kernel data", selector: KERNEL_DATA, descriptor: flat(KERNEL_RW) },
    Segment { name: "kernel stack", selector: KERNEL_STACK, descriptor: flat(KERNEL_RW) },
    Segment { name: "user code", selector: 0x20 | 3, descriptor: flat(USER_EXEC) },
    Segment { name: "user data", selector: 0x28 | 3, descriptor: flat(USER_RW) },
    Segment { name: "user stack", selector: 0x30 | 3, descriptor: flat(USER_RW) },
];

/// What the table register's limit field must hold: the offset of the
/// last valid byte, so one less than the table's size in bytes.
///
/// Derived from the descriptor count and the descriptor size, never
/// from `size_of_val(&SEGMENTS)`. A `Segment` carries a name and a
/// selector besides the 8 bytes the CPU reads, so that expression
/// returns 0x8b instead of 0x37. The CPU accepts the wrong value and
/// the kernel keeps running, willing to load a bogus eighth
/// descriptor. `tools/check.sh` is what caught it.
pub const LIMIT: u16 = (SEGMENTS.len() * core::mem::size_of::<u64>() - 1) as u16;

/// The 6-byte operand `lgdt` reads: a limit then a base, in that order,
/// unaligned. `packed` is what keeps the u32 from sliding to offset 4.
#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u32,
}

/// Pack one flat descriptor. The 8 bytes are, low to high: limit 15-0,
/// base 15-0, base 23-16, the access byte, the granularity flags over
/// limit 19-16, base 31-24. Base 0 leaves three of those fields zero,
/// so only the limit, the access byte and the flags are written.
///
/// Flags 0xc means 4 KiB granularity and 32-bit operands, which turns a
/// limit of 0xfffff into the full 4 GiB.
const fn flat(access: u8) -> u64 {
    0xffff | ((access as u64) << 40) | (0xcf << 48)
}

/// Copy the table to 0x800, point the CPU at it, and reload every
/// segment register from it. Returns with `cs` = kernel code, `ss` =
/// kernel stack, and the other four = kernel data.
///
/// `lgdt` alone changes nothing a running instruction can observe,
/// because each segment register still holds its hidden descriptor from
/// GRUB's table. Reloading is the point, and `cs` is the awkward one:
/// no `mov` may write it. A far return will, so the selector and the
/// address of the next instruction are pushed and `retf` pops them into
/// `cs:eip` together.
///
/// # Safety
/// Overwrites 56 bytes at 0x800 and replaces the segmentation the
/// caller is running under. Sound only because every descriptor
/// installed here is flat, which leaves every address already in flight
/// meaning what it did before.
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

/// What the CPU currently believes, read back from the CPU rather than
/// from `SEGMENTS`. `sgdt` reports the table the processor is using, so
/// a mismatch with 0x800 is a real failure and not a stale constant.
pub struct Gdtr32 {
    pub base: u32,
    pub limit: u16,
}

/// Ask the CPU where its table is.
pub fn current() -> Gdtr32 {
    let mut gdtr = Gdtr { limit: 0, base: 0 };
    unsafe { asm!("sgdt [{}]", in(reg) &mut gdtr) };
    Gdtr32 { base: gdtr.base, limit: gdtr.limit }
}

/// The selector held in `cs`, `ss` and `ds` right now. Printed by the
/// shell next to the table: the table being installed and the segment
/// registers actually using it are two separate claims.
pub fn selectors() -> (u16, u16, u16) {
    let (cs, ss, ds): (u16, u16, u16);
    unsafe {
        asm!("mov {:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        asm!("mov {:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
        asm!("mov {:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
    }
    (cs, ss, ds)
}
