# Glossary

Every technical term used by this project, defined once. Grouped by subject
rather than alphabetically, because the terms only make sense in clusters —
use your reader's search to jump to a word.

One idea underpins all of them. Every program you have ever written ran *on top
of* an operating system. `printf`, `malloc`, `open`, `import`, `Promise` — all
of those are, sooner or later, requests to a kernel. This project *is* the thing
that would have answered them. There is nothing underneath it: no files, no
threads, no `println!`, and no heap until kfs-3 built one out of the frames it
found in RAM. Dereference a bad pointer and no one kills your process: since
kfs-3 the kernel catches the fault itself, prints the address and the registers,
and stops the processor.

"**Here:**" marks where the concept shows up in this repository.

## Start here

If the whole vocabulary is new, read these eleven in this order and the rest
will have somewhere to attach:

**Kernel**, **Bare metal / freestanding**, **Bootloader**, **Multiboot
specification**, **Entry point**, **Linker script**, **Triple fault**,
**`no_std`**, **Page**, **Page table**, **Page fault**.

---

## Operating systems and kernels

These words describe your own programs from the outside. You have always been
the guest: something else owned the CPU, handed you memory, and answered your
system calls. Here there is no host — this code is the host, and the words below
name the job it has inherited.

**Kernel** — the one program on a machine allowed to touch hardware directly;
everything else runs on top of it and asks it for favours. In kfs-1 there is no
"everything else": the kernel is the only software running. Here:
`kernel/src/lib.rs` plus `boot/boot.asm`.

**Operating system** — a kernel plus the programs around it that you actually
interact with: a shell, libraries, services. kfs-1 is a kernel, not an OS.

**Bare metal / freestanding** — code with no operating system underneath it.
No files, no processes, no `malloc`, no `printf`, no `println!` — only the CPU,
RAM, and whatever hardware you program yourself. Here: `"os": "none"` in
`kernel/i686-kfs.json`.

**Kernel space / user space** — the privileged half of a running system versus
the sandboxed half where applications live. kfs-1 had the words only: every
address belonged to the kernel. kfs-3 makes the split a property of the address
space. Kernel space is `0xC0000000` and above, plus the identity-mapped first
4 MB; user space is `0x00400000..0xBFFFFFFF`, and its pages carry the U/S bit
set, which is what would let ring 3 reach them. `paging::map_page` refuses a
user page at a kernel address. Nothing runs in ring 3 yet, so user space is a
set of rights with no tenant ([PAGING.md](PAGING.md)).

**Ring 0** — x86's name for its most privileged level, the one a kernel runs in;
ordinary applications get ring 3. Our code is in ring 0 from its very first
instruction, because GRUB left the CPU there. A ring lives in two bits of a
segment descriptor's access byte, and in the low two bits of a selector. kfs-2
declares user code, data and stack descriptors at ring 3, so the table the
subject asks for is complete, but nothing runs there yet: leaving ring 0 needs
a way back in, and that means an interrupt table. Here: `kernel/src/gdt.rs`.

**System call** — the single legal doorway a user-space program has for asking
the kernel to do something. Every `open`, `write` and `mmap` you have called was
one. Not implemented here: there is no user space to knock on the door.

**Driver** — kernel code that knows one specific device's private protocol.
The VGA text output ([VGA.md](VGA.md)) is this kernel's first driver.

**Panic / kernel panic** — a bug the program has decided it cannot survive. In
userland something catches it and kills your process; here nobody is listening,
so a panic stops the machine. Here: `kpanic!` prints the reason in bright red,
dumps CR0, CR2, CR3 and 64 bytes of stack, then runs `cli` and `hlt` for good.
Rust's own `panic!` ends in the same place, because the `#[panic_handler]` in
`kernel/src/lib.rs` calls the fatal path ([PANIC.md](PANIC.md)).

**Oops (recovered panic)** — a fault the kernel can repair, reported rather
than fatal. Linux's word for it, and this kernel's answer to the subject's "all
panics are not fatal". Here: `koops!` prints in yellow, adds one to a counter,
and returns to its caller. A refused allocation, a double free and a page fault
inside the demand zone are all oopses, and every fatal report ends with how
many came before it: "0 recovered before this one" on a clean boot.

---

## The x86 processor

You have never had to care what mode the CPU is in, how many registers it has,
or whether floating point is switched on, because something arranged all of that
long before `main` ran. Here nobody did. Beyond the little GRUB sets up, the
CPU's state is our problem, and these words name the pieces of it.

**x86** — the family of instruction sets Intel began with the 8086 and PCs still
use. Backwards-compatible to a fault, which is the entire reason booting walks
through several CPU modes.

**i386 / i686** — generation names inside the 32-bit x86 family. i386 is the
first 32-bit member; i686 is the Pentium-Pro-era baseline, and the name inside
our build target. The subject requires a 32-bit kernel.

**Register** — a storage slot inside the CPU itself, and the fastest memory that
exists. Registers are the CPU's local variables: 32-bit x86 has eight
general-purpose ones (`EAX`, `EBX`, `ECX`, `EDX`, `ESI`, `EDI`, `EBP`, `ESP`)
plus a few special ones.

**EIP** — the register holding the address of the instruction being executed. It
answers a debugger's most basic question, "which line am I on". Here: `make
check` passes only when `EIP` is inside the kernel.

**ESP** — the register holding the address of the top of the stack (see
**Stack**), otherwise called the stack pointer. Here: `_start`'s first
instruction loads it with `stack_top`. The `stack` command dumps memory upward
from the live value, so the first address of a dump is the stack pointer itself.

**EFLAGS** — the register holding one-bit facts about the CPU: was the last
comparison equal, are interrupts enabled, did the last addition overflow.

**Control register (CR0 to CR4)** — the registers that configure the CPU itself
rather than hold data: which modes are on, where the page directory lives. They
are not general-purpose, no arithmetic reaches them, and only a `mov` in ring 0
can read or write one. Here: `kernel/src/paging.rs` reads CR0, CR2 and CR3
through inline assembly, and `boot.asm` writes CR3 and CR0 to switch paging on.

**CR0** — the oldest switchboard: bit 0 PE protected mode, bit 4 ET, bit 16 WP
write protect, bit 31 PG paging. It reads `0x80010011` in this kernel, which is
those four bits and nothing else. Setting bit 31 is the instant paging begins.

**CR2** — written by the CPU when a page fault happens, and holding the address
that faulted. Nothing else writes it, and it is only meaningful inside the
fault handler. Here: printed as `cr2` on every fault report, and read by
`paging::handle_fault` to decide whether the fault can be repaired.

**CR3** — the physical address of the page directory, so the CPU knows where
translation starts. Writing it, even with the same value, empties the
translation cache. It reads `0x0010e000` in this build, and `make check`
compares that against the address the kernel prints for its own directory.

**CR4** — the later feature switches: PSE for 4 MB pages, PAE for 36-bit
addressing, PGE for global pages. This kernel never writes it, which is one way
of saying 4 kb pages only, no PAE, no global pages.

**Write protect (CR0.WP)** — bit 16 of CR0. Without it the read-only bit in a
page table binds ring 3 only, and kernel code may write any page it can see.
With it the CPU refuses a ring 0 write to a read-only page, which is what makes
"memory rights" mean something in a kernel that never leaves ring 0. Here: set
at the end of `paging::init`, and proven by the shell's `fault ro`.

**Real mode** — the 16-bit mode the CPU wakes up in: 1 MiB of addressable
memory, no memory protection, 1978 rules. The BIOS and GRUB's first stage run
here.

**Protected mode** — the 32-bit mode with memory protection and privilege
levels, and what a modern 32-bit kernel expects to find itself in. GRUB switches
into it before jumping to us, which is why this repo contains no mode-switching
code.

**Long mode** — 64-bit x86 mode. Out of scope; the subject mandates 32-bit.

**Segment / GDT** — x86 can slice memory into segments, each described by an
entry in a table called the Global Descriptor Table. GRUB installs a flat,
permissive GDT before handing over, so the kernel works without touching it;
kfs-2 replaces it with the kernel's own, seven descriptors copied to physical
`0x800`. Here: `kernel/src/gdt.rs`, the subject of [GDT.md](GDT.md).

**Segment descriptor** -- one 8-byte row of the descriptor table, and the thing
the CPU actually reads. It carries a 32-bit base, a 20-bit limit, an access
byte and four flag bits. The fields are scattered across the eight bytes for
compatibility with the 80286, so building one is bit shuffling.

**Access byte** -- byte 5 of a descriptor, which says what the segment is: bit
7 present, bits 6-5 the ring, bit 4 code or data, bit 3 executable, bit 2
direction or conforming, bit 1 readable or writable, bit 0 accessed. The six
segments of this kernel differ in nothing else: `0x9a` and `0x92` at ring 0,
`0xfa` and `0xf2` at ring 3.

**Selector** -- the value a segment register holds: an index into the
descriptor table, shifted left by 3, plus the ring it asks for in the low two
bits. Here: `0x08` kernel code, `0x10` kernel data, `0x18` kernel stack, and
`0x23`, `0x2b`, `0x33` for the ring 3 rows, whose low bits carry the 3.

**Segment register** -- the six registers that hold those selectors: `cs` for
instructions, `ss` for the stack, and `ds`, `es`, `fs`, `gs` for data. No `mov`
can write `cs`, which is why a table install ends in a far return.

**Descriptor cache** -- the hidden copy of a descriptor that each segment
register keeps. The CPU reads the table when a selector is loaded and works
from the copy afterwards. That is why GRUB's segments keep working after GRUB's
table is gone, and why the multiboot specification forbids reloading a segment
register before the kernel owns a table.

**Flat memory model** -- one segment over the whole address space: base 0,
limit 4 GiB, for every descriptor. Segmentation then adds nothing to an
address, and a linear address is a physical address. Every segment here is
flat, which is also what makes it safe to swap tables while running, since no
address in flight changes meaning.

**Granularity flag** -- the descriptor bit that multiplies the limit by 4096. A
20-bit limit reaches 1 MiB on its own; with the flag set, `0xfffff` becomes the
full 4 GiB. Here: the flags byte `0xcf` of all six segments.

**Expand-down segment** -- a data segment with the direction bit set, which
inverts the meaning of the limit: valid offsets start above it rather than
below. Intended for a stack that grows downwards. A flat expand-down segment
reaches no address at all, so this kernel has none, and `shell::seg_range`
prints such a descriptor as an empty range instead of a healthy-looking limit.

**Accessed bit** -- bit 0 of the access byte. The CPU sets it in the descriptor
*in memory* the first time its selector is loaded, so the table you authored is
not byte-for-byte the table you read back: an authored `0x92` returns `0x93`.
`tools/check.sh` masks that one bit. A page-table entry has a bit of the same
name in a different place; see **Accessed bit (A) and dirty bit (D)** below.

**Null descriptor** -- entry 0 of the table, which must be eight zero bytes.
The CPU refuses to load selector 0, and that is what turns an uninitialised
segment register into a fault instead of a silent read through whatever entry 0
happened to describe.

**Table register (GDTR)** -- the 6-byte register that tells the CPU where its
descriptor table is: a 16-bit limit, then a 32-bit base, unaligned. The limit
is the last valid byte, so seven descriptors give `0x0037`, which the kernel
computes once as `gdt::LIMIT`. QEMU's monitor prints the register as
`GDT=base limit`.

**`lgdt`** -- the instruction that loads the table register from a 6-byte
operand in memory. On its own it changes nothing an executing instruction can
observe, because every segment register still holds its cached descriptor. The
reloads after it are what take effect.

**`sgdt`** -- the instruction that stores the table register back to memory. It
answers what the CPU is really using rather than what the source declares,
which is why the shell's `gdt` command reads the table through it.

**Far jump / far return** -- a jump or a return that loads `cs` as well as
`eip`. Since no `mov` may write `cs`, a kernel that has just installed its own
table pushes the code selector and a return address and runs `retf`, which pops
both together. Skipping that does not fault at once: the cached descriptor keeps
working, and the failure arrives later, at the first interrupt, far call or ring
change.

**Interrupt** — an event that makes the CPU drop what it is doing and jump to a
handler: a key press, a timer tick, a division by zero. It is a callback the
hardware invokes whether your code was ready or not. Device interrupts arrive
disabled at boot and stay disabled here: `sti` appears nowhere, the PIC is
never remapped, and the keyboard is still read by polling. kfs-3 handles the
CPU's own exceptions only, which is what a fault report needs; devices are
kfs-4's work.

**IDT** — the Interrupt Descriptor Table, the array mapping interrupt numbers to
handler addresses. kfs-3 installs one: 256 slots, of which the first 32 hold
real gates, at `0xc0114004` in this build. Its address goes into the CPU with
`lidt`, the interrupt-table twin of `lgdt`. Here: `kernel/src/idt.rs`
([PANIC.md](PANIC.md)). The shell still exploits an empty table on purpose:
`reboot`'s fallback replaces the live one with a table of base 0 and limit 0,
so no vector can be fetched at all.

**Vector** — the number that identifies an interrupt or an exception, and the
index of its row in the IDT. Vectors 0 to 31 belong to the CPU: 0 is divide by
zero, 8 double fault, 13 general protection fault, 14 page fault. Here:
`kernel/src/idt.rs` names all 32, which is how a report can say "page fault
(vector 14)" instead of a bare number.

**Interrupt gate** — one 8-byte row of the IDT: the code selector to enter, the
32-bit address of the handler split across two halves, and a type byte. Every
gate here is type `0x8e` with selector `0x08`: present, ring 0, 32-bit
interrupt gate. An interrupt gate clears the interrupt flag on entry; a trap
gate, the other choice, leaves it alone.

**Stub (interrupt stub)** — the few assembly instructions a vector actually
points at, needed because the CPU does not tell the handler which vector it
was, and pushes an error code for some vectors but not others. Here: the
`isr_plain` and `isr_error` macros in `kernel/src/idt.rs` push a zero where the
CPU pushed no error code, push the vector number, and jump to `isr_common`,
which is the one place that saves registers and calls Rust. 32 stubs, a table
of their addresses in `.rodata`, and one handler.

**`pusha`** — one instruction that pushes all eight general-purpose registers;
`popa` pops them back. It is how `isr_common` turns the CPU's own pushes plus
the register set into the `Frame` struct the Rust handler reads field by field.

**`iret`** — the return from an interrupt handler: it pops EIP, CS and EFLAGS
together, which an ordinary `ret` cannot do. Here: the last instruction of
`isr_common`, reached only when the fault was recovered, since a fatal one
never returns.

**Exception** — an interrupt the CPU raises about itself: invalid opcode, page
fault, division by zero. Same idea as a hardware-level thrown error. Since
kfs-3 there is exactly one `catch`: a page fault inside the user demand zone is
repaired and the faulting instruction runs again. Every other exception prints
a report and stops the machine.

**Triple fault** — the CPU faults, faults again while handling that fault,
faults a third time, gives up and resets. Before kfs-3 there was no IDT, so
*any* CPU exception ended this way, and from outside it looked like the machine
rebooting in a loop. With the 32 exception vectors wired, a triple fault now
takes deliberate work. It is the kernel-land equivalent of a segfault, except
nothing catches it. Here: `make check` rules out the accidental kind by proving
the guest settles inside the kernel and stays there. The shell's `reboot`
command still causes one on purpose, as its fallback: it loads an interrupt
table of length zero and runs `int3`, so the CPU cannot find the handler,
cannot find the double-fault handler either, and the third fault stops the
processor, which the chipset wires to a reset. Linux reboots the same way
([SHELL.md](SHELL.md)).

**Paging / MMU** — the hardware that translates the addresses your code uses
into real addresses in RAM, and so lets code use memory that is not where it
claims to be, or is not there at all. kfs-3 switches it on: `boot.asm` sets bit
31 of CR0, and every address after that instruction is a virtual address. The
vocabulary of tables, entries, rights and faults has a section of its own
below ([PAGING.md](PAGING.md)).

**TLB (translation lookaside buffer)** — the CPU's cache of translations it has
already worked out, so that a memory access does not cost two extra table reads
every time. It is a cache with no coherency: edit a page table and the CPU may
keep using the stale translation until something tells it not to. Most mappings
that "should have worked" fail here.

**`invlpg`** — the instruction that drops one page's cached translation.
Cheaper than emptying the cache, and enough after a single mapping change.
Here: `paging::invalidate`, called from `map_page`, `unmap_page` and `protect`,
so no caller has to remember it.

**TLB flush** — emptying the whole cache, done on 32-bit x86 by writing CR3
back into itself. Here: `paging::flush`, used once, when the kernel installs
its own page table over the boot one.

**Red zone** — an optimisation where a function scribbles in the 128 bytes just
below the stack pointer without formally reserving them. Fine in userland, fatal
in a kernel: an interrupt pushes its own data there and silently overwrites it.
Here: `"disable-redzone": true`.

**FPU / MMX / SSE** — the floating-point and vector units on an x86 chip. They
must be switched on explicitly after boot, so a kernel that has not switched
them on must not contain a single one of their instructions — banning them is
cheaper than initialising hardware the kernel does not use. Here: `"features":
"-mmx,-sse,+soft-float"`.

**Soft float** — doing floating-point arithmetic in software with ordinary
integer instructions instead of hardware float instructions. Slow, but it works
on a chip whose FPU nobody turned on.

**MMIO (memory-mapped I/O)** — a memory address that is really a device:
writing to it changes hardware instead of storing a value. The VGA text screen
is MMIO, which is why writes to it must be `volatile` — the compiler must not
optimise away a write whose whole point is the side effect. Since kfs-3 the
driver writes to `0xc00b8000` rather than `0xb8000`: paging gives the same
physical cells a second virtual address inside kernel space, and the screen
cannot tell the difference. Two virtual addresses for one frame is an ordinary
mapping, not a trick.

**Port I/O** — x86's other way of reaching devices, using `in` and `out`
instructions on a separate, small address space of its own. Here: the hardware
cursor (ports `0x3D4`/`0x3D5`) and the PS/2 keyboard (status `0x64`, data
`0x60`), and the reset command the shell writes to `0x64`. The two instructions
live in one module, `kernel/src/port.rs`, which the screen, the keyboard and
the shell all call.

**PS/2 / 8042** — the classic PC keyboard interface and the controller chip
behind it. QEMU emulates one regardless of your real keyboard. It hands out
scancodes, never characters. The same chip drives the CPU reset line, so a
write of `0xfe` to port `0x64` restarts the machine, which is what the shell's
`reboot` command does. QEMU's monitor can put a key straight into the chip's
queue with `sendkey`.

**Scancode** — the number a keyboard sends for a physical key *position*:
press and release are separate codes (release = press | 0x80), and "which
character is that" is a layout decision the kernel makes with a lookup table.
Here: `kernel/src/keyboard.rs`, US QWERTY, set 1.

**Polling** — asking a device "anything new?" in a loop, as opposed to the
device raising an interrupt when something happens. Wasteful but simple, and
the only option before an IDT exists. Here: the shell's line editor, which asks
the keyboard for a key on every lap and pauses when there is none.

**VGA text mode** — the 80×25 grid of characters a PC starts up in. Each cell is
two bytes at `0xb8000`: a character code and a colour attribute. Here: driven
by `kernel/src/vga.rs`, the subject of [VGA.md](VGA.md).

**Framebuffer** — the region of memory a display continuously reads its pixels
or characters out of. Change the memory and the screen changes.

**`hlt`** — the instruction that stops the CPU until the next interrupt arrives.
The correct way for a kernel to idle: a bare `while (true) {}` would burn a core
for nothing. Here: the hang loop after `call kmain` in `_start`, and the
shell's `halt` command, which runs `cli` and then `hlt` in a loop, because one
`hlt` on its own can be resumed by the next interrupt.

**`cli`** — "clear interrupt flag": tells the CPU to stop accepting maskable
interrupts. Here: immediately before `hlt` in `_start`, and in the shell's
`halt` command. Non-maskable events can still wake the CPU, which is why the
`jmp` after it halts again.

**`pause`** — an instruction meaning "this loop is only waiting", letting the
CPU save power and avoid a pipeline penalty. Here: what
`core::hint::spin_loop()` compiles to inside the shell's key loop (bytes
`f3 90`).

**Little-endian** — x86 stores multi-byte values least-significant byte first,
which is why the multiboot magic `0x1BADB002` appears in the binary as
`02 b0 ad 1b`.

**Stack** — the memory holding return addresses and local variables, tracked
through `ESP`. Nobody hands a kernel one; it reserves memory and points `ESP` at
it. On x86 the stack grows *downwards*, so the initial pointer is the *highest*
address of the reserved region. Here: 16 KiB in `.bss`, with `esp` starting at
`stack_top` = `0xc0114000` and `stack_bottom` at `0xc0110000`. Those are
kernel-space addresses since kfs-3: the stack sits inside the kernel image and
is reached through the higher-half mapping. `boot.asm` exports both ends, so the
shell's `stack` command can dump the part in use ([STACK.md](STACK.md)).

**Stack frame** -- the slice of the stack that belongs to one call: the return
address the call pushed, saved registers, and that function's own locals. The
newest frame is the lowest in memory, because x86 pushes downwards. A dump
taken inside a function starts in that function's own frame, which is why the
first four bytes of a `stack` dump are the return address into its caller.

**Hex dump** -- the classic way to read raw memory: an address, then the bytes
as two-digit hex, then the printable ones as characters. Here: 16 bytes to a
row and 77 columns per row ([STACK.md](STACK.md)). The bytes come out in memory
order, so on a little-endian machine a saved address reads backwards, and
`0x0010063c` appears as `3c 06 10 00`.

---

## Memory and paging

kfs-3 puts hardware between a pointer and RAM. Until now every address this
kernel used was a real address on a memory chip, and every byte of RAM was
equally reachable and equally writable. Now the CPU translates each address
through two tables the kernel writes itself, a page can be read-only, absent or
owned by user space, and asking for memory means asking an allocator this
kernel had to build first. These words name that machinery
([PAGING.md](PAGING.md), [MEMORY.md](MEMORY.md), [PANIC.md](PANIC.md)).

**Page** — a fixed-size, fixed-aligned block of an address space: 4 KiB here,
and the unit everything below counts in. Presence, rights and translation are
decided per page, never per byte, which is why a kernel that wants its code
read-only must first know which pages hold code. This kernel uses 4 kb pages
and nothing else.

**Page frame** — a page-sized block of *physical* memory, and what a page is
mapped to. A page is an address; a frame is RAM. QEMU's default machine gives
32 639 frames of 4 kb, of which 24 263 are still free once low memory, the
kernel image and the kmalloc block are accounted for (`mem`).

**Virtual address** — the address the code uses, and the only kind of address a
pointer in this kernel holds once paging is on. It means nothing to the memory
chips: the CPU has to look it up first. `0xc0101000` is a virtual address; the
byte it names lives at physical `0x00101000`.

**Linear address** — the address that comes out of segmentation and goes into
paging. On x86 an address is computed, the segment base is added to it, and the
result is what paging translates. Every segment here is flat with base 0, so a
virtual address and a linear address are the same number. It matters once:
`lgdt` takes a *linear* address, so the descriptor table at physical `0x800`
needs linear `0x800` to keep pointing at it, which is one reason the first 4 MB
stays identity mapped.

**Physical address** — the address that reaches the memory bus, the one RAM and
devices answer to. Only three things here deal in physical addresses: CR3, the
table entries, and the frame allocator. Everything else works in virtual
addresses. `paging::translate` converts one into the other, and the `virt`
command prints both.

**Address translation** — the lookup itself: the CPU splits the address into
three fields, reads one entry from the directory, one from the table that entry
names, and adds the offset to the frame it finds.

| bits | field | selects |
|---|---|---|
| 31..22 | directory index, 0..1023 | which page table |
| 21..12 | table index, 0..1023 | which page inside it |
| 11..0 | offset, 0..4095 | which byte inside the page |

**Offset within a page** — the low 12 bits of an address, carried through
translation untouched. That is why a mapping can only move memory in 4 KiB
steps, and why an entry needs to store 20 bits of address rather than 32.
Here: `paging::offset`, printed as the `offset` column of `virt`.

**Page directory** — the upper of the two tables: one page holding 1024
four-byte entries, each covering 4 MB of address space. CR3 holds its physical
address. Here: `boot_directory` in `boot/boot.asm`, at physical `0x0010e000` in
this build, filled by the entry stub before paging is switched on.

**Page table** — the lower level: one page of 1024 entries, each describing one
4 KiB page. A directory entry that is not present has no table at all, which is
how a 4 GB address space costs a few pages instead of 4 MB of tables. Here: the
boot low table, the static kernel-window table at physical `0x00138000`, and
one table per 4 MB region the allocators touch.

**Page directory entry (PDE)** — one 32-bit word of the directory: the top 20
bits are the physical address of a page table, the low 12 are flags. `pages`
and `virt` print them raw. `0x00138023` is the entry for `0xc0000000`: table at
`0x00138000`, present, writable, accessed.

**Page table entry (PTE)** — one 32-bit word of a table, the same shape as a
PDE but naming the frame itself. `0x00101021` maps `0xc0101000` to frame
`0x00101000`, present and accessed, with the write bit clear because that page
holds kernel code.

**Present bit (P)** — bit 0 of an entry. Clear means "nothing here": the CPU
raises a page fault instead of translating, and the other 31 bits are the
kernel's to use as it likes. Every mapping helper checks it first, and
`paging::get_page(addr, create)` allocates a fresh table when a directory entry
is not present.

**Read/write bit (R/W)** — bit 1. Clear makes the page read-only. It constrains
ring 0 only when CR0.WP is set, which this kernel does, so the twelve pages
`0xc0101000..0xc010cfff` genuinely cannot be written by the code inside them.
Here: `paging::init` clears it for kernel code and read-only data,
`paging::protect` changes it afterwards, and `fault ro` proves it.

**Supervisor bit (U/S)** — bit 2, misleading only in its name: set means user
code may touch the page, clear means ring 0 only. Kernel pages leave it clear;
`paging::USER_PAGE` sets it. `paging::map_page` refuses a user page at a kernel
address and reports an oops instead of mapping it.

**Accessed bit (A) and dirty bit (D)** — bits 5 and 6, both written by the CPU
and never by the kernel: A on the first read or write through the page, D on
the first write. A system that swaps clears them to find pages worth evicting.
This kernel only reads them, so they record what has been touched:
`virt 0xc0101000` prints `a 1 d 0` for code that has run but was never written,
and `user` prints `a 1 d 1` for a page it has just stored into.

**Page size bit (PS)** — bit 7 of a *directory* entry. Set, the entry maps a
single 4 MB page and there is no second level. It is 0 everywhere here, so
every present directory entry names a real table and every mapping is 4 kb.
Using it would also mean enabling PSE in CR4.

**Global bit (G)** — bit 8, which asks the CPU to keep a translation across a
CR3 reload. It exists for kernel mappings, which are identical in every address
space. This kernel names the flag and never sets it: with one address space
there is nothing to keep it for, and it needs CR4.PGE to have any effect.

**Identity mapping** — a mapping where the virtual address equals the physical
address, so translation changes nothing. It is what a kernel needs for the
addresses hardware and firmware handed it. Here: the first 4 MB, supervisor
only, which keeps the descriptor table at `0x800`, the VGA cells at `0xb8000`
and GRUB's multiboot information reachable at the numbers other code already
believes. The cost is that user space starts at 4 MB instead of 0.

**Higher-half kernel** — a kernel that lives in the top part of every address
space, leaving the bottom to user code. The i386 convention splits at
`0xC0000000`, three quarters of the way up, and that is what this kernel does:
its bytes are loaded at physical `0x00100000` and its code runs at `0xC0101000`
upwards. The gain is that the kernel keeps the same addresses whatever runs
below it.

**Recursive page directory entry** — the trick of pointing a directory entry at
the directory itself. Entry 1023 does that here, which makes every live page
table appear in the address space without mapping any of them by hand. Without
it the kernel would have to map a frame temporarily each time it wanted to edit
a table, because a table's own frame is otherwise reachable through no address.

**Page-table window** — the address range that trick produces: the table for
directory slot *n* at `0xFFC00000 + n * 4096`, and the directory itself at
`0xFFFFF000`, since it is the table of slot 1023. Writing to `0xFFF00000` edits
the table that maps the kernel window. `pages` lists the three tables this
kernel keeps live and nothing else.

**Page fault** — the exception the CPU raises when translation fails or the
rights refuse the access: vector 14. CR2 holds the address, an error code says
what was attempted, and the faulting instruction has not run. It is the
mechanism behind every "segmentation fault" you have seen, from the other side:
here it is our handler that decides what happens next.

**Error code** — the word the CPU pushes for a page fault, describing the
attempt rather than the address: bit 0 the page was present, bit 1 the access
was a write, bit 2 it came from user code. `0x00000003` is a write to a page
that was present, so the rights refused it; `0x00000002` is a write to a page
that was not there. Both appear in the reports of `fault ro` and
`fault kernel`.

**Demand paging** — mapping a page only when someone first touches it, instead
of up front. It is how a real system promises more memory than it commits.
Here: `0x00800000..0x008fffff` has no pages at boot; a fault inside that range
makes `paging::handle_fault` take a frame, zero it, map it as a user page and
let the instruction run again. `fault demand` reads the first address of the
range and gets `0x00000000` back from a page that did not exist a moment
earlier.

**Multiboot memory map** — the list GRUB leaves behind describing RAM: for each
region a base, a length and a type, where type 1 means usable. A kernel cannot
guess this, because RAM has holes. Here: `kernel/src/multiboot.rs` reads up to
12 usable regions, clamps them to 32 bits, and falls back to the older
`mem_lower`/`mem_upper` pair when no map is present. `mem` prints what it
found: `0x00000000..0x0009fc00` and `0x00100000..0x07fe0000`, 130 559 kb in two
regions on QEMU's default machine.

**Reserved memory** — memory that exists but must not be handed out: anything
the loader did not call usable, plus whatever the kernel is already using.
Here: `pmm::reserve` marks the whole first 1 MB, which holds the BIOS area, the
descriptor table and the VGA cells, and then the kernel image at physical
`0x00101000..0x0013a000`. Everything left is fair game, which is why the
reservations have to happen before the first allocation.

**Frame allocator** — the bottom of the memory stack: the code that hands out
physical frames and takes them back, with nothing underneath it to ask. Here:
`kernel/src/pmm.rs`, with `pmm::alloc_frame` for one frame, `pmm::alloc_range`
for consecutive frames, `pmm::free_frame` to give one back and `pmm::carve` for
a block kept aside.

**Bitmap allocator** — a frame allocator that keeps one bit per frame. Simple,
fixed in size, and cheap to search a machine word at a time. Here: 1 << 20 bits
for the whole 4 GB a 32-bit address can name, 128 KB in `.bss`, where a set bit
means free. Zero-initialised therefore reads as "nothing is free", so the map
is safe before any code runs and costs no bytes in the image.

**Carved block** — one long run of consecutive frames taken from the allocator
at boot and kept for a single purpose. Here: `pmm::carve` asks for a quarter of
usable RAM capped at 32 MB, halving its request until a run fits, and gives the
result to `kmalloc`. In this build the block is physical
`0x0013a000..0x02119000`, 32 636 kb, and it is what makes every kmalloc pointer
physically contiguous.

**Physically contiguous / virtually contiguous** — consecutive in RAM, versus
consecutive only in the address space. Paging makes the second cheap and the
first scarce, and hardware that reads memory on its own needs the first. Here:
this is the whole difference between the two allocators. `kmalloc(5000)`
crosses a page boundary inside the carved block, and its two halves are at
physical `0x0013a050` and `0x0013b050`; `vmalloc(12288)` is one run of
addresses over frames `0x0211a000`, `0x0211c000` and `0x0211d000`, in that
order, and its caller cannot tell.

**Out of memory** — the case every allocator has to answer for. There is no
swapping here and nothing to reclaim, so the answer is a refusal, and the
refusal must not be a crash. Here: `pmm::alloc_frame` returns `None`, `kmalloc`
and `vmalloc` return a null pointer, the reason is reported with `koops!`, and
the kernel carries on. Failing to carve the kmalloc block at boot is the one
memory failure that is fatal, because nothing sensible can follow it.

**Heap** — a region of memory an allocator hands out in pieces of whatever size
the caller asks for, rather than in whole pages. `malloc` gives you one; a
kernel has to build its own. Here: two of them, `0xd0000000..0xd1ffffff` for
`kmalloc` and `0xe0000000..0xefffffff` for `vmalloc`, both empty at boot and
both growing a page at a time ([MEMORY.md](MEMORY.md)).

**Break (`brk` / `sbrk`)** — the top of a heap: the first address past the part
that has memory behind it. Moving the break is how a heap grows and shrinks.
Unix's `sbrk` returns the *old* break, so the caller knows where the space it
just gained begins, and `heap::kbrk` and `heap::vbrk` keep that: `kbrk(+4096)`
reports "break was 0xd0003000, now 0xd0004000". The delta is rounded up to
whole pages, since a page is the smallest thing that can be mapped, and a
refused move returns a null pointer.

**Block header** — the few bytes an allocator keeps in front of every block to
remember what it handed out. Here: 8 bytes, a size and a used flag, so the
pointer a caller receives is always its block plus 8, and the first `kmalloc`
of a boot returns `0xd0000008`. It is also what makes `ksize` possible: the
size is one read backwards from the pointer.

**Boundary tag** — the classic name for that arrangement: every block carries
its own tag at its boundary, so the heap is a walkable list needing no separate
index, and `kfree` needs nothing but the pointer. Textbook boundary tags repeat
the tag as a footer, which lets a block merge with the block *before* it in
constant time. This kernel keeps the header only, so merging works forwards,
and `kfree` validates a pointer by walking the list from the base rather than
trusting it.

**First fit** — take the first free block big enough, instead of looking for
the best one. It is the cheapest policy that works, and it is what
`heap::Heap::first_fit` does, absorbing free neighbours as it walks. Best fit
wastes less and pays a full scan every time.

**Splitting** — cutting the tail off an oversized free block so the remainder
stays usable. Here: a block is split only when what is left can hold a header
and the smallest payload, 16 bytes together; below that the caller quietly gets
the few extra bytes.

**Coalescing** — merging free blocks that touch, so that freeing two
neighbours yields one block big enough for something. Without it a heap decays
into unusable crumbs. Here: done while searching rather than while freeing —
`first_fit` folds each following free block into the one it is looking at.

**Fragmentation** — free memory that cannot be used because of its shape rather
than its total. It is the reason an allocator with plenty free can still refuse
a request.

**Internal fragmentation** — the waste inside what was handed out: rounding,
alignment and the header itself. Here every request is rounded up to 8 bytes
and pays an 8-byte header, so `kmalloc(1)` costs 16 bytes of heap and `ksize`
reports 8.

**Double free** — freeing the same pointer twice. In userland it corrupts the
allocator and the crash arrives later somewhere else, which is what makes it
such a hard bug. Here the header says whether the block is in use, so the
second `kfree` is caught, reported as an oops, and ignored.

**Dangling pointer** — a pointer to memory that has been freed or unmapped. A
freed heap pointer still points at mapped memory here, so reading through it
returns stale bytes instead of faulting, which is exactly why `kfree` and
`ksize` check the header rather than trusting the address. An unmapped pointer
is louder: after the `user` command unmaps its page, `paging::translate` says
there is nothing there and touching it would be a page fault.

---

## Booting

Between power-on and your `main` sits a chain of programs you have never had to
meet. This section names the links. Only two matter here: the firmware in the
machine, and a bootloader whose only job is to find a kernel, load it into
memory, and jump to it.

**Firmware** — software stored in the machine itself, running before any disk
has been read.

**BIOS** — the traditional PC firmware. It initialises hardware, runs POST, then
loads the first sector of the chosen boot device and jumps into it. QEMU provides
one; we boot in BIOS mode, not UEFI.

**POST** — power-on self test, the firmware's own hardware check at startup.

**UEFI** — the modern replacement for the BIOS, with a different boot protocol.
`grub-mkrescue` happens to produce an image that works with both; our path is
BIOS.

**Boot sector / MBR** — the first 512-byte sector of a bootable device, holding
the first-stage bootloader; it is the only code the BIOS will load sight unseen.
Here: `grub-mkrescue` embeds `boot_hybrid.img` in the ISO's system area so the
image also boots as if it were a disk.

**Bootloader** — a small program whose only job is to find a kernel, load it
into memory, and jump to it. Writing one yourself means talking to disks in real
mode; using GRUB is what the subject asks for.

**GRUB (GRand Unified Bootloader)** — the standard Linux bootloader, and the one
this project uses. It reads a config file, loads a kernel image, prepares the CPU
(protected mode, flat GDT) and jumps to the kernel's entry point. Here:
`grub.cfg`, and `grub-mkrescue` to build the ISO.

**GRUB module** — a piece of GRUB itself that GRUB loads on demand: filesystem
drivers, video drivers. `grub-mkrescue` copies its whole module tree onto the
ISO, which is why a 62 KB kernel yields a 2.7 MB image. The Makefile passes
`--fonts= --locales= --themes=` to leave GRUB's optional data out: on Fedora
the font and the translations alone would push the image past the subject's
limit.

**`grub.cfg`** — GRUB's configuration file, read at boot from
`/boot/grub/grub.cfg` inside the image. Here: the repo's `grub.cfg`, copied
there by the Makefile.

**`menuentry`** — one bootable choice in `grub.cfg`. Ours is named `"kfs"` and
contains only `multiboot /boot/kernel.bin` and `boot`.

**Multiboot specification** — a handshake between bootloaders and kernels: put a
recognisable header near the front of your image and any compliant bootloader
will load it, handing you a machine already in protected mode plus a description
of memory. It saves every hobby kernel from writing a bootloader. We use
multiboot **1**; multiboot 2 is a different header format and a different GRUB
command. The specification is also explicit about what it does *not*
guarantee: the table register may be invalid on entry, so the kernel must not
load any segment register, even with the same value, until it owns a descriptor
table of its own.

**Multiboot header** — the 12 bytes that do the identifying: magic, flags,
checksum. GRUB scans the first 8 KiB of the binary for it, 4-byte aligned; find
it and GRUB loads and jumps, miss it and GRUB refuses the file. Here: `section
.multiboot` in `boot/boot.asm`, forced first by `linker.ld`.

**Magic number** — a fixed, arbitrary value used purely as a signature, so a
reader can tell "this is one of mine". Multiboot 1's is `0x1BADB002`.

**Checksum** — a value chosen so that a sum comes out to a known result. Here
`MAGIC + FLAGS + CHECKSUM` must equal 0 in 32-bit arithmetic, giving
`0xE4524FFB`. A wrong checksum makes GRUB refuse the file.

**Entry point** — the address the first instruction lives at. It is recorded in
the ELF header and chosen by `ENTRY(_start)` in the linker script; `0x100010`
here.

**Load address** — where an image's contents are placed in physical memory. Ours
is 1 MiB (`0x100000`), the conventional lowest address a kernel may claim, since
below it live the BIOS data area, the interrupt vector table, the VGA
framebuffer and mapped ROM. Since kfs-3 it is no longer the address the code
runs at: the kernel is loaded at physical `0x00101000` and runs at `0xC0101000`
(see **VMA / LMA**).

**ISO 9660** — the filesystem format used on CD-ROMs. `kfs.iso` is one.

**El Torito** — the extension to ISO 9660 that makes a CD bootable.

**`grub-mkrescue`** — GRUB's tool for producing a bootable ISO from a directory
tree: it adds GRUB itself, its modules and the boot records. Fedora names the
binary `grub2-mkrescue`, Debian `grub-mkrescue`; the Makefile detects both.

**`grub-file`** — GRUB's inspection tool. `grub-file --is-x86-multiboot`
succeeds only if a valid multiboot header is present, so the build runs it as an
assertion straight after linking.

**`xorriso` / `mtools`** — the ISO-writing and FAT-manipulation utilities
`grub-mkrescue` calls under the hood. You never invoke them; you only notice
them when they are missing.

---

## Assembly

Assembly is here for one reason: some things cannot be said in a high-level
language. Rust has no way to write "set the stack pointer to this address", and
until the stack pointer is set, no function can be called at all. So a dozen
lines of assembly run first and then hand over to Rust for good.

**Assembly language** — a text form of machine instructions, one line per
instruction. Needed wherever no higher-level language can express the operation:
here, setting `ESP` before any function call can happen.

**Assembler** — the program that translates assembly text into an object file.
It is to assembly what a compiler is to C, with almost nothing left to decide.

**NASM (Netwide Assembler)** — the assembler this project uses, in Intel syntax
(`mov dst, src`: destination first, like `dst = src`). Invoked as `nasm -f elf32`
to produce a 32-bit ELF object.

**Mnemonic** — the human-readable name of an instruction: `mov`, `call`, `jmp`.

**Opcode / machine code** — the actual bytes the CPU executes.
`mov esp, 0x107710` is `bc 10 77 10 00`.

**Label** — a name for an address (`_start:`, `stack_top:`). In NASM a label
beginning with `.` belongs to the previous global label, which is why the hang
loop is `_start.hang`.

**Directive** — a line aimed at the assembler rather than the CPU, closer to a
`#define` or a pragma than to an instruction:
- `equ` — define a constant.
- `dd` — emit a 32-bit value ("define doubleword"). Used for the three header
  words.
- `resb` — reserve bytes without storing them in the file. Used for the stack.
- `align` — pad until the address is a multiple of N.
- `section` — start emitting into a named section.
- `global` — export a symbol so the linker can see it (`_start`,
  `stack_bottom`, `stack_top`).
- `extern` — declare a symbol defined elsewhere (`kmain`).

**Calling convention** — the rules a caller and callee agree on: which registers
carry the arguments, where the return value goes, who must restore what.
Assembly and Rust have to agree, and `extern "C"` is how both pick the same set.

**ABI (application binary interface)** — the wider contract between separately
compiled pieces of code: calling convention plus type sizes, alignment, and how
symbol names are spelled. Here: `"rustc-abi": "softfloat"` states that floats
are not passed in SSE registers.

---

## Compiling and linking

Normally your compiler and your OS between them decide what the binary looks
like and what addresses it occupies, and you never see the decision made. Here
nothing decides, so the layout is written out by hand and every address in it is
somebody's choice. These words name the parts of that machinery.

**Compiler** — translates a high-level language into machine code. Here:
`rustc`, driven by `cargo`.

**Cross compilation** — building code for a machine other than the one doing the
build. Every kernel build is one: the host runs Linux or macOS, the target runs
nothing.

**Host vs target** — the machine compiling versus the machine that will run the
output. Here: host `x86_64-unknown-linux-gnu` inside the container, target
`i686-kfs`.

**Target triple** — the conventional name for a target, roughly
`arch-vendor-os-environment`, e.g. `i686-unknown-linux-gnu`. You normally pick
one off a shelf; ours would be `i686-unknown-none`.

**Target spec JSON** — Rust's way to describe a target it does not already ship,
as a JSON file of properties. There is no shelf entry for "32-bit x86, no
operating system at all", so you write the spec sheet yourself. Here:
`kernel/i686-kfs.json`, enabled by the unstable `json-target-spec` flag.

**LLVM** — the compiler backend `rustc` hands its work to for the final step
down to machine code. The target spec's `llvm-target` and `data-layout` are
passed straight to it.

**Data layout** — LLVM's own description of a target: pointer size, and the
alignment of every integer and float type. It must match the LLVM target, or
generated code is subtly wrong with no warning.

**Object file** — the output of assembling or compiling one source file: machine
code plus sections, symbols and relocations, with nothing yet decided about
where in memory it goes. Here: `build/boot.o`.

**ELF (Executable and Linkable Format)** — the Unix binary format for object
files and executables. It describes the same bytes twice: as sections, for the
linker, and as segments, for whoever loads it. GRUB parses the ELF to know where
to put our code.

**Section** — a named region of a binary grouping similar content:
- `.text` — executable code.
- `.rodata` — read-only data: constants and string literals.
- `.data` — writable data whose initial values sit in the file.
- `.bss` — writable data that starts as zeros and occupies no file space.
- `COMMON` — legacy uninitialised symbols, folded into `.bss` here.
- `.got` -- the global offset table: one slot per symbol reached indirectly,
  filled in by the linker.
- `.eh_frame` -- unwind tables, which describe how to walk back out of a
  panic. This kernel aborts instead, so `linker.ld` discards them.
- `.comment` — toolchain version strings; not loaded.
- `.note.GNU-stack` — a marker declaring whether the stack must be executable.
- `.multiboot` — *our* custom section name, so the linker script can place the
  header first.
- `.boot` — *our* second custom section, holding the entry stub that runs
  before paging: `linker.ld` gives it the same virtual and load address,
  `0x00100000`, because at that moment a higher-half address maps to nothing.

**NOBITS** — the ELF flag meaning "occupies memory but has no bytes in the
file". `.bss` is NOBITS: RAM at boot, 0 bytes on disk. It says "reserve this
many zeros in RAM" rather than "ship them inside the file" — closer to `calloc`
than to a literal array in the binary. kfs-3 leans on it: `.bss` now holds the
boot page directory, the boot page table, the 16 KiB stack and the 128 KB frame
bitmap, 176 129 bytes of memory for no image size at all. All-zero also happens
to be the safe state of the frame bitmap, which reads as "no frame is free", so
no code is needed to initialise it.

**Symbol** — a name attached to an address, so other code can refer to it
without knowing the number (`kmain`, `stack_top`).

**Symbol table** — the list of a binary's symbols and their addresses; read it
with `nm`.

**Relocation** — a hole in an object file with a note attached: "patch this
address in once the final layout is known". `call kmain` in `boot.o` is one, and
`ld` fills it with `0x1022f0`.

**Linker** — combines object files and libraries into one image, matching up
symbols and assigning final addresses. Here: GNU `ld`, called with
`-m elf_i386 -n -T linker.ld`.

**Linker script** — the file telling the linker what to place where. Normally
the compiler and the OS decide what address your code lives at; here nothing
does, so you write it out by hand. Here: `linker.ld`.

**Location counter (`.`)** — the linker script's cursor: the address it is
currently handing out. `. = 1M;` moves it to `0x100000`, so the first section
placed after that line starts there.

**Section garbage collection** -- `ld --gc-sections` drops every input section
nothing reaches from the entry point, and `KEEP(...)` in the linker script
exempts one from the sweep. It matters more here than it looks: rustc emits
`compiler_builtins` as a single object file, so needing one helper out of it
pulls the whole 318 KiB member in, soft-float `f128` mathematics included.
Measured: `kernel.bin` is 150 112 bytes without the flag and 17 828 bytes with
it. The multiboot header is the one section that needs `KEEP`, because nothing
in the program refers to it and GRUB finds it by scanning the file.

**VMA / LMA** — a section's virtual address (where the code believes it is) and
its load address (where it is actually put). They were identical until kfs-3,
because paging was off. Now they differ for the whole kernel: `.text` has VMA
`0xc0101000` and LMA `0x00101000`. GRUB copies the bytes to the LMA with paging
off; the code in them is compiled for the VMA and only becomes correct once the
directory maps one onto the other. The entry stub is the exception, and has to
be: `.boot` keeps VMA = LMA = `0x00100000`, because it is what runs before any
mapping exists.

**`AT()`** — the linker-script clause that sets a section's load address
independently of its virtual address. `.text : AT(ADDR(.text) - KERNEL_SPACE)`
places `.text` for a CPU that believes in `0xc0101000` while telling the loader
to write the bytes at `0x00101000`. Without it the linker assumes the two are
equal, and GRUB would try to write 3 GB up in a machine with 128 MB of RAM.

**Segment / program header** — the loader's view of an ELF: which byte ranges to
copy to which addresses with which permissions. Our kernel has two `LOAD`
segments since kfs-3: the entry stub, 97 bytes at `0x00100000` with its virtual
and physical addresses equal, and the kernel proper, 50 540 bytes on disk
expanding to 229 377 in RAM, virtual `0xc0101000` and physical `0x00101000`.

**Static library / archive (`.a`)** — a bundle of object files in one file.
`cargo` emits `libkernel.a` — a bag of parts rather than a finished program — so
*our* `ld` invocation does the final assembly with *our* linker script. Created
and listed with `ar`.

**Name mangling** — the compiler encoding module paths and types into a symbol's
name so same-named functions cannot collide. This kernel's panic handler is
mangled to `_RNvCs9aRK3BLRY2F_7___rustc17rust_begin_unwind`, and the shell makes
a panic reachable, so it is in every kfs-2 build. `#[no_mangle]` switches it
off so assembly can refer to `kmain` by that exact name.

**`nm` / `objdump` / `readelf`** — the inspection tools: list symbols,
disassemble and dump sections, print ELF headers. Every address quoted in
[ARCHITECTURE.md](ARCHITECTURE.md) came from them.

---

## Rust specifics

Rust's standard library assumes an operating system beneath it: files, threads,
a heap, `println!`. None of that exists here, so most of this section is about
switching things off — the standard library, stack unwinding, name mangling,
prebuilt targets. What remains is roughly C with better types.

**Rust** — the systems language this kernel is written in: no runtime, no
garbage collector, memory safety checked at compile time, and `unsafe` blocks
for the places where hardware access cannot be proven safe.

**Cargo** — Rust's build tool and package manager in one, roughly `make` plus
`pip`/`npm`. Reads `Cargo.toml`.

**Crate** — Rust's unit of compilation: like a C translation unit, but
package-sized and compiled in one go. This repo has one, `kernel`.

**Crate type** — what a crate compiles to. `staticlib` produces a C-compatible
`.a` archive, which is what lets our own `ld` command do the final link.
(`rlib` would be Rust-only; `bin` would make cargo link an executable with the
wrong script.)

**`Cargo.toml` / `Cargo.lock`** — the manifest you write and the exact dependency
versions cargo resolved, like `package.json` and its lock file. The lock file is
committed so a rebuild is reproducible.

**Edition** — which dialect of the Rust language a crate is written in (2021
here). A compatibility marker, not a compiler version.

**Profile** — a named set of build settings. `[profile.release]` uses
`opt-level = 2` and `panic = "abort"`.

**`no_std`** — the attribute that unlinks the standard library. Roughly C's
`-ffreestanding` with no libc: necessary because `std` assumes an OS with
threads, files, a heap and syscalls.

**`core`** — the part of Rust's standard library that needs no operating system:
primitive types, `Option`, `Result`, slices, atomics, `PanicInfo`. Always
available, kernels included.

**`alloc`** — the layer between `core` and `std` providing `Box`, `Vec` and
`String`. It needs a working allocator, so it is out of reach until this kernel
has a heap.

**Panic handler** — the `#[panic_handler]` function every `no_std` crate must
provide, called when a panic occurs. Normally `std` supplies one that prints and
kills the process; ours loops forever, because there is nowhere to report to.

**Unwinding vs abort** — on a panic Rust normally unwinds the stack, running
destructors on the way out, which needs runtime support that does not exist here.
`panic = "abort"` in `Cargo.toml` and `"panic-strategy": "abort"` in the target
spec remove it.

**`#[no_mangle]`** — keep a symbol's name exactly as written. It is what
wrapping a declaration in `extern "C"` does to a name in C++.

**`extern "C"`** — use the C calling convention for a function, so non-Rust code
can call it correctly. Here: `kmain`, which `boot.asm` calls.

**Never type (`-> !`)** — the return type of a function that never returns.
`kmain() -> !` is why an infinite loop is required and why the code after
`call kmain` is unreachable.

**Nightly** — Rust's unstable release channel. Required here because rebuilding
`core` for a custom target and using JSON target files are both unstable
features. Pinned in `kernel/rust-toolchain.toml`.

**`rust-src`** — the rustup component shipping the standard library's *source*,
without which there is nothing to rebuild `core` from. Listed in
`kernel/rust-toolchain.toml`.

**`build-std`** — the unstable cargo feature that compiles `core` (and
`compiler_builtins`) from source for a target that has no prebuilt copy. Here:
`kernel/.cargo/config.toml`.

**`compiler_builtins`** — Rust's implementations of the low-level helper
routines LLVM assumes someone provides: 64-bit division on a 32-bit CPU, and,
with the `compiler-builtins-mem` feature, `memcpy`/`memset`/`memcmp`. On a
normal system libc supplies those; we have no libc, so we opt in.

**`volatile`** — a memory access the compiler may not optimise away, reorder or
merge. Mandatory for MMIO such as the VGA buffer, where the *act* of writing is
the point and the value stored is beside it. Here: specified for the planned
`vga.rs`.

**`unsafe`** — the keyword marking operations the compiler cannot verify, such
as dereferencing a raw pointer to `0xb8000`. It disables no checks; it moves the
burden of proof to the author.

**`core::hint::spin_loop()`** — a portable way to say "this loop is
spin-waiting"; on x86 it emits `pause`. Here: the whole body of `kmain`, and of
the panic handler.

---

## Build and test tooling

You cannot run this program. There is no `./kernel` to type, no test harness
inside it, and no `assert` that could tell you anything, because the kernel has
nowhere to print. So `make check` is a unit test for a whole machine: build a CD
image, boot it in an emulator, and ask the emulator from the outside where the
CPU ended up.

**Make** — the build automation tool that runs commands when files are out of
date. Its vocabulary: a **target** is a thing to build, its **prerequisites**
are what it depends on, and its **recipe** is the tab-indented commands.

**Phony target** — a Make target that names an action, not a file (`all`,
`clean`, `cargo`). Declared in `.PHONY` so Make never mistakes a same-named file
for it.

**Pattern rule** — a Make rule with a `%` wildcard. Here `docker-%:` matches
`docker-all` and `docker-check`, and passes `$*` — the part matched by `%` — to
`make` inside the container.

**QEMU** — the machine emulator used to run the kernel. `qemu-system-i386`
emulates a 32-bit PC: BIOS, CD-ROM, VGA, the works. Emulation rather than
virtualisation means it can run x86 code on an arm64 Mac, slowly but faithfully.

**`-cdrom kfs.iso`** — attach the ISO as the boot CD. Here: `make run` and
`make check`.

**`-display none`** — run with no window: headless, for scripted checks. Here:
`make check`.

**QEMU monitor** — QEMU's own control console, separate from anything the guest
can see or touch. It reports CPU state, dumps the screen, pauses and resets the
machine — a debugger attached to the emulated hardware rather than to a process.
Exposed here on a unix socket with `-monitor unix:/tmp/kfs-mon,server,nowait`.
Its text form is the **human monitor**, and `tools/check.sh` speaks it: all
thirty-six assertions of `make check` are monitor round trips, and nothing
inside the guest takes part in any of them.

**`info registers`** — the monitor command printing the guest's CPU registers.
The `EIP` it reports is the proof that the kernel is executing. It prints the
six segment registers too, the table register as `GDT=base limit`, `ESP`, and
the `HLT` flag, which is how the check proves the kernel's own descriptors are
in use and that `halt` really stopped the processor.

**`xp`** -- the monitor command that reads guest **physical** memory.
`xp/14wx 0x800` prints the descriptor table as fourteen words, and
`xp/2000hx 0xb8000` prints the whole VGA text buffer as 2 000 halfwords. The low
byte of each cell is its character code, so decoding those bytes yields the
exact text on screen.

**`sendkey`** -- the monitor command that injects one key into the guest. It has
no string form, so the check types a command one key per round trip. The key
lands in the 8042 controller's queue, the status bit on port `0x64` goes high
from queue occupancy alone, and the polled driver reads it with no interrupt
involved.

**`screendump file.ppm`** — the monitor command writing the current guest screen
to an image. kfs-1's check used it, and grepped the image for the white "42"
glyphs and for the absence of GRUB's grey. kfs-2 reads the text out of the VGA
buffer with `xp` instead, which compares characters rather than pixel colours,
so the old constraint that kept dim palette colours off the boot screen is gone
with it.

**Headless** — running with no display attached.

**`socat`** — a utility that pipes data between arbitrary endpoints. Here it
connects stdin/stdout to the monitor's unix socket, which is how `make check`
talks to a running QEMU without a terminal.

**Unix domain socket** — an inter-process channel addressed by a filesystem path
instead of a port. `/tmp/kfs-mon` is one.

**Docker** — container tooling, used only to build on a machine with no x86
toolchain. An **image** is a filesystem template built from the `Dockerfile`; a
**container** is a running instance; a **bind mount** (`-v $(PWD):/kfs`) exposes
the host's repo inside it so build artifacts land in the working tree.
`--platform=linux/amd64` requests the x86 image on an arm64 host, which Docker
runs under emulation. The evaluation machine uses none of this.
