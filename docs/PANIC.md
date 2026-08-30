# Kernel panics

A panic is what this kernel does when it meets a state it did not
plan for.  `kernel/src/panic.rs` holds the two macros that report
one, `kernel/src/idt.rs` catches the CPU exceptions behind most of
them, and `kernel/src/paging.rs` decides whether a page fault is a
mistake or a request.  This file explains what a panic means with
no operating system underneath, how a fault travels from the CPU
into Rust, what every field of the report says, and how `make check`
proves the path from outside the guest.

The address space is in [PAGING.md](PAGING.md), the allocators in
[MEMORY.md](MEMORY.md), and terms in [GLOSSARY.md](GLOSSARY.md).
Every screen quoted here comes from one build: page boundaries and
physical addresses are stable, but an `eip` inside a dump moves
whenever the kernel is recompiled.

## What a panic is when nothing is underneath

On a hosted system a segmentation fault is a signal.  The kernel
delivers it to one process, the process dies, and the rest of the
machine does not notice.  Here nothing is bigger.  This kernel is
the only program on the machine: no process to kill, no signal, no
supervisor.  When the CPU raises an exception, the only code that
can answer is the code that just failed.

The kfs-3 subject asks for two things: **print**, because the screen
is the only channel out, and **stop the kernel**, because running on
from an unplanned state corrupts more with every instruction.  It
then adds: *all panics are not fatal, make the difference*.

- A **fatal** panic prints a report, dumps the machine, and halts.
  Nothing runs afterwards.
- A **recovered** panic prints one line, counts itself, and returns.
  The kernel keeps running.

Nothing between the two exists.

## The two severities in code

Both are macros over `core::format_args!`, so they take the same
arguments as `printk!` and allocate nothing.

`kpanic!` is fatal.  `panic::fatal` prints `KERNEL PANIC:` in red,
calls `machine()` for control registers and stack, and ends in
`stop()`.  `stop()` executes `cli` then `hlt` in a loop; with
interrupts masked the processor stops for good.  The loop catches a
non-maskable interrupt or debugger wake.  Outside the guest the
state reads as `HLT=1` in the QEMU monitor.

`koops!` is recovered.  `panic::oops` increments a static counter,
prints `kernel oops <n>: …` in yellow, restores white, and returns.
The counter is the only state it leaves behind; the fatal report
prints it as `<n> recovered before this one`.

**The rule.**  A situation is recovered when the caller asked for
something the kernel can refuse and the refusal leaves the machine
unchanged.  Everything else is fatal.  A refused `kfree` is
recovered.  A page fault on unmapped kernel space is fatal: the
pointer is wrong, and returning would run the same instruction.

Every caller of `koops!`, messages shortened:

| Site | Message | Refused |
| ---- | ------- | ------- |
| `heap.rs` | `ran out of its physical block` | kmalloc past carved end |
| `heap.rs` | `found no free physical frame` | vmalloc, no frame |
| `heap.rs` | `reached the end of its address range` | past ceiling |
| `heap.rs` | `the top of <heap> is in use` | shrink over live blocks |
| `heap.rs` | `the free top ... is too small` | shrink, no room |
| `heap.rs` | `<addr> did not come from <heap>` | foreign pointer |
| `heap.rs` | `<addr> is freed twice` | double free |
| `paging.rs` | `it takes no user page` | user page in kernel space |
| `paging.rs` | `no free frame for the page table` | out of frames |
| `paging.rs` | `<addr> is mapped already` | present entry |
| `idt.rs` | `page fault at <addr>, demand paged` | a fault repaired |

The last row is the only one that is not a refusal: it reports work
that succeeded but is counted anyway, because a demand fault is
still a fault.

## Why this subject needs an interrupt descriptor table

The table is 256 slots of 8 bytes.  Without it the CPU cannot
deliver exception 14 (page fault), tries exception 8 (double fault),
fails for the same reason, and triple-faults: the machine silently
reboots.  That is the opposite of print and stop.  So the table is
not next subject's work borrowed early; it is the only way to meet
the panic requirement.  `idt::install()` runs before `mem::init`.

What it does **not** do:

- Interrupts are never enabled.  The pushed `eflags 0x00010092` in
  every fatal report confirms it: bit 9 is clear.
- The 8259 PIC is not remapped.
- The keyboard stays polled via the 8042 status port.
- Only 32 of the 256 slots are filled; the rest have their present
  bit clear.

Interrupts proper belong to kfs-4.  The boot banner reports:

```
faults: 256 vectors at 0xc0114004, 32 exceptions wired, interrupts off
```

## How a fault reaches Rust

The CPU jumps to a code address, not to a Rust function.  32 stubs
built by `global_asm!` in `idt.rs` catch the jump, save registers,
and call Rust.  The CPU pushes an error code for vectors 8, 10-14,
17, 21 and 30; two macros absorb the difference:

```asm
.macro isr_plain vector        ; .macro isr_error vector
isr\vector:                    ; isr\vector:
    pushl $0                   ;     pushl $\vector
    pushl $\vector             ;     jmp isr_common
    jmp isr_common             ;
.endm                          ; .endm
```

`isr_plain` pushes a zero where the CPU would have pushed an error
code, so both produce the same stack shape.

```asm
isr_common:
    pusha
    pushl %esp
    call fault_entry
    addl $4, %esp
    popa
    addl $8, %esp
    iret
```

`pusha` saves eight general registers; `pushl %esp` passes their
address to `fault_entry` as a `*mut Frame`.  On return `popa`
restores them, `addl $8` drops vector and error, and `iret` pops
`eip`, `cs` and `eflags`, rerunning the faulting instruction.  No
stack switch happens: both gate and fault are ring 0, so the handler
runs on the same 16 KiB kernel stack ([STACK.md](STACK.md)).

### The frame

`Frame` is `#[repr(C)]`; offsets from the pointer `fault_entry`
receives:

| offset | field | pushed by |
| ------ | ----- | --------- |
| 0-28 | `edi esi ebp esp ebx edx ecx eax` | `pusha` |
| 32 | `vector` | the stub |
| 36 | `error` | the CPU, or stub's zero |
| 40 | `eip` | the CPU |
| 44 | `cs` | the CPU |
| 48 | `eflags` | the CPU |

`esp` here points 32 bytes above the frame, not at the code that
faulted.  `eip` is the faulting instruction, not the one after it,
because a page fault is a fault and not a trap.

### The gate bytes

| bits | value | meaning |
| ---- | ----- | ------- |
| 0-15 | stub low half | offset 15-0 |
| 16-31 | `0x08` | kernel code selector |
| 32-39 | `0` | reserved |
| 40-47 | `0x8e` | present, ring 0, 32-bit interrupt gate |
| 48-63 | stub high half | offset 31-16 |

`0x8e` = `1000 1110`: present, DPL 0, interrupt gate.  The selector
`0x08` is the kernel code segment from [GDT.md](GDT.md), which is
why every fatal report shows `cs 0x0008`.  The interrupt gate also
clears the interrupt flag on entry, correct once interrupts exist.

`install()` reads 32 stub addresses from `isr_table` in `.rodata`,
writes one gate per vector, and loads the register with limit
`256 * 8 - 1`.  A vector above 31 finds a zero slot (not present)
and becomes a general protection fault, which has a gate.

## The page-fault decision

`fault_entry` is the only Rust function the stubs call.  For
vector 14 it reads `CR2` and offers it to `paging::handle_fault`;
if that returns `true`, it prints the `koops!` and returns into
`isr_common`.  Every other case calls `panic::fatal_fault(frame)`.

| bit | name | 0 | 1 |
| --- | ---- | - | - |
| 0 | P | not present | present, rights refused |
| 1 | W/R | read | write |
| 2 | U/S | ring 0 | ring 3 |

```rust
pub fn handle_fault(addr: u32, error: u32) -> bool {
    if error & PRESENT != 0 || !mem::is_demand(addr) {
        return false;
    }
    let page = mem::page_of(addr);
    unsafe { map_new(page, USER_PAGE) }.is_some()
}
```

The demand zone is `0x00800000..0x00900000`, one megabyte inside
user space.  Only a not-present fault there can be recovered.  A
present fault means the rights refused the access, and relaxing them
would delete the guarantee they were installed for.  `map_new` takes
a frame from `pmm::alloc_frame` and maps it as present, writable,
user-accessible; `None` means no frame is left, and the fault
becomes fatal.

What *recovered* means at the hardware level: the handler maps a
frame, prints one yellow line, and returns.  `iret` reloads the
faulting `eip`, the CPU runs the **same instruction again**, and
this time the translation exists.  `fault demand` shows it:

```
kfs> fault demand
reading 0x00800000, which is not mapped yet
kernel oops 1: page fault at 0x00800000, demand paged, eip 0xc0105bac
the handler mapped it, the read returned 0x00000000
kfs>
```

The shell printed the first line, the handler the second, and the
shell printed the third after the retried read returned.  The prompt
proves nothing was lost.  Error code `0`: not present, a read, from
ring 0.  The value is `0x00000000` because frames are not cleared.

## The fatal report

Read `fault ro` field by field:

```
KERNEL PANIC: page fault (vector 14)
  cr2 0xc0101000, a kernel write, the page was present: the rights said no
  error 0x00000003 eip 0xc0105a7c cs 0x0008 eflags 0x00010092
  eax 0x00000000 ecx 0x000000a0 edx 0x000003d5 ebx 0xc010d560
  esi 0xc0101000 edi 0x000000a0 ebp 0xc010d3c8 esp 0xc0113f08
  cr0 0x80010011 cr2 0xc0101000 cr3 0x0010e000, 0 recovered before this one
stack: esp 0xc0113e10 -> top 0xc0114000, 496 of 16384 bytes in use
c0113e10  11 00 01 80 00 10 10 c0  00 e0 10 00 00 00 00 00 |................|
c0113e20  10 3e 11 c0 50 92 10 c0  14 3e 11 c0 50 92 10 c0 |.>..P....>..P...|
the kernel is stopped
```

| field | reading |
| ----- | ------- |
| vector name | `NAMES[14]`, the 32-entry table in `idt.rs` |
| cause | for vector 14 only, from the error code bits |
| `error 0x3` | P set, W/R set, U/S clear |
| `eip 0xc0105a7c` | the faulting instruction in `cmd_fault` |
| `cs 0x0008` | the kernel code selector |
| `eflags 0x00010092` | IF clear; bit 16 is the resume flag |
| eight registers | the `pusha` block |
| `cr0 0x80010011` | PG, WP, ET, PE |
| `cr2 0xc0101000` | the faulting address |
| `cr3 0x0010e000` | the page directory |
| `0 recovered` | the oops counter |
| stack window | 64 bytes from the live stack pointer |

The cause line is built from the three bits: `user`/`kernel` from
U/S, `write`/`read` from W/R, and `present: the rights said no` or
`not present` from P.  The frame's `esp 0xc0113f08` is the value
`pusha` saved; the stack header's `esp 0xc0113e10` is the live value
inside `machine()`, 248 bytes deeper.

The window is printed only when the stack pointer is inside the
kernel stack.  Outside it the read could fault, and a fault inside
the panic handler has nowhere to go.  Printing itself is safe:
`printk!` writes straight into the VGA buffer at `0xC00B8000`
([VGA.md](VGA.md)), takes no lock, allocates nothing, and nothing
runs after the report.

## The three fatal cases the shell can trigger

Each proves a different claim, with its own cause line:

```
  cr2 0xc0101000, a kernel write, the page was present: the rights said no
  cr2 0xd1800000, a kernel write, the page was not present
KERNEL PANIC: the shell asked for a panic with no way back
```

**`fault ro`** writes to `0xc0101000`, the first page of `.text`.
Error `0x3`: present, write.  The page is mapped read-only
(`0xc0101000..0xc010cfff`, `r  kernel`) and the write was refused.
That proves the R/W bit is really zero and `CR0.WP` is really set;
without WP a ring 0 write ignores the read-only bit.

**`fault kernel`** writes to `0xd1800000`, inside the kmalloc range
but past what is mapped.  Error `0x2`: write, not present.  The
address is kernel space, so the demand zone does not apply.  This is
the shape of a wild kernel pointer.

**`panic fatal`** calls `kpanic!` with no CPU fault.  The report has
no `Frame`, so no vector, no error code, no registers.  Only
`machine()` runs; `cr2 0x00000000` because no page fault has touched
CR2 since boot.  **On a software panic CR2 is stale.**

## Rust's own `panic!`

A `no_std` crate must provide `#[panic_handler]`; the program does
not link without it:

```rust
#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    panic::fatal(core::format_args!("{}", info))
}
```

It routes into the same `fatal` that `kpanic!` uses: one fatal path,
no second reporting style.  `panic = "abort"` in both Cargo profiles
and `-C panic=abort` in `RUSTFLAGS` means no unwinding, no landing
pads, no `eh_personality`.  A Rust panic is never recovered; only
`koops!` recovers.

## What `make check` proves

`tools/check.sh` types commands via the QEMU monitor, reads the VGA
buffer for screen text, and reads `info registers` for CPU state.
Seven of its 36 assertions cover this file:

- The boot reports the exception table.
- `user` maps a page, and the refused user page in kernel space
  prints `kernel oops` without stopping the kernel.
- `fault demand` shows `demand paged` and `the read returned`,
  proving the retried instruction completed.
- `panic oops` prints its message and the prompt returns.
- Each of `fault ro`, `fault kernel` and `panic fatal` shows its
  cause text, and then a register read asserts `HLT=1`:

```sh
hlt=$(registers | sed -n 's/.*HLT=\([0-9]\).*/\1/p' | head -1)
[ "$hlt" = "1" ] || fail "the fatal panic left the CPU running"
```

`HLT=1` is the CPU's own report, taken from outside the guest.  A
fatal panic ends the run, so `system_reset` is issued between the
three cases.

## The limits

- **No double-fault task gate.**  Vector 8 uses the same stack, so
  a fault inside the handler triple-faults the machine.
- **No backtrace.**  The release build omits frame pointers; the
  report prints 64 raw stack bytes instead ([STACK.md](STACK.md)).
- **The oops counter is never reset.**  `0 recovered` means a fresh
  boot; a reboot is the only way back to zero.
- **No user-mode fault yet.**  Nothing runs in ring 3, so the U/S
  bit is always 0 and the user half of the cause line is untested.
- **Only 32 vectors are wired.**  A higher vector becomes a general
  protection fault (vector 13).  Nothing can fire one today.
