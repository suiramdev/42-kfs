use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

use crate::gdt;
use crate::panic::{self, koops};
use crate::paging;
use crate::printk::printk;

pub const VECTORS: usize = 32;
pub const SLOTS: usize = 256;
pub const PAGE_FAULT: u32 = 14;

const INTERRUPT_GATE: u8 = 0x8e;

#[repr(C)]
pub struct Frame {
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    pub vector: u32,
    pub error: u32,
    pub eip: u32,
    pub cs: u32,
    pub eflags: u32,
}

static NAMES: [&str; VECTORS] = [
    "divide by zero",
    "debug",
    "non maskable interrupt",
    "breakpoint",
    "overflow",
    "bound range exceeded",
    "invalid opcode",
    "device not available",
    "double fault",
    "coprocessor segment overrun",
    "invalid task state segment",
    "segment not present",
    "stack segment fault",
    "general protection fault",
    "page fault",
    "reserved",
    "floating point error",
    "alignment check",
    "machine check",
    "simd floating point error",
    "virtualisation error",
    "control protection fault",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "hypervisor injection",
    "vmm communication",
    "security fault",
    "reserved",
];

pub fn name(vector: u32) -> &'static str {
    match NAMES.get(vector as usize) {
        Some(name) => name,
        None => "interrupt",
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u32,
}

static mut TABLE: [u64; SLOTS] = [0; SLOTS];

extern "C" {
    #[link_name = "isr_table"]
    static STUBS: [u32; VECTORS];
}

fn gate(handler: u32) -> u64 {
    (handler as u64 & 0xffff)
        | ((gdt::KERNEL_CODE as u64) << 16)
        | ((INTERRUPT_GATE as u64) << 40)
        | (((handler as u64) >> 16) << 48)
}

pub fn install() {
    let stubs = addr_of!(STUBS) as *const u32;
    let table = addr_of_mut!(TABLE) as *mut u64;
    for vector in 0..VECTORS {
        let handler = unsafe { read_volatile(stubs.add(vector)) };
        unsafe { write_volatile(table.add(vector), gate(handler)) };
    }
    let idtr = Idtr { limit: (SLOTS * 8 - 1) as u16, base: table as u32 };
    unsafe { asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack, preserves_flags)) };
    report();
}

fn base() -> u32 {
    addr_of!(TABLE) as u32
}

#[no_mangle]
pub extern "C" fn fault_entry(frame: *mut Frame) {
    let frame = unsafe { &mut *frame };
    if frame.vector == PAGE_FAULT {
        let addr = paging::cr2();
        if paging::handle_fault(addr, frame.error) {
            koops!("page fault at {:#010x}, demand paged, eip {:#010x}", addr, frame.eip);
            return;
        }
    }
    panic::fatal_fault(frame)
}

fn report() {
    printk!(
        "faults: {} vectors at {:#010x}, {} exceptions wired, interrupts off\n",
        SLOTS,
        base(),
        VECTORS
    );
}

global_asm!(
    r#"
.macro isr_plain vector
.globl isr\vector
isr\vector:
    pushl $0
    pushl $\vector
    jmp isr_common
.endm

.macro isr_error vector
.globl isr\vector
isr\vector:
    pushl $\vector
    jmp isr_common
.endm

isr_plain 0
isr_plain 1
isr_plain 2
isr_plain 3
isr_plain 4
isr_plain 5
isr_plain 6
isr_plain 7
isr_error 8
isr_plain 9
isr_error 10
isr_error 11
isr_error 12
isr_error 13
isr_error 14
isr_plain 15
isr_plain 16
isr_error 17
isr_plain 18
isr_plain 19
isr_plain 20
isr_error 21
isr_plain 22
isr_plain 23
isr_plain 24
isr_plain 25
isr_plain 26
isr_plain 27
isr_plain 28
isr_plain 29
isr_error 30
isr_plain 31

isr_common:
    pusha
    pushl %esp
    call fault_entry
    addl $4, %esp
    popa
    addl $8, %esp
    iret

.section .rodata
.align 4
.globl isr_table
isr_table:
    .long isr0
    .long isr1
    .long isr2
    .long isr3
    .long isr4
    .long isr5
    .long isr6
    .long isr7
    .long isr8
    .long isr9
    .long isr10
    .long isr11
    .long isr12
    .long isr13
    .long isr14
    .long isr15
    .long isr16
    .long isr17
    .long isr18
    .long isr19
    .long isr20
    .long isr21
    .long isr22
    .long isr23
    .long isr24
    .long isr25
    .long isr26
    .long isr27
    .long isr28
    .long isr29
    .long isr30
    .long isr31
.text
"#,
    options(att_syntax)
);
