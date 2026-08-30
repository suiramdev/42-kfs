# kfs documentation

`kfs` is a kernel: the piece of software that would normally be *underneath*
your program. Every `printf`, `malloc` and `open` you have ever written was a
request to one. This project has nothing underneath it at all: no files, no
threads, and no one to kill your process when you get a pointer wrong. There is
a `kmalloc`, but it is a file in this repo rather than a favour from an
operating system. When a pointer is wrong, the kernel names the fault, dumps
the machine, and halts the processor.

These docs assume you write C, C++, Python or TypeScript and have never touched
assembly, a linker script, or a bootloader.

| Document | What it covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the whole thing works: power-on to the kernel's idle loop, every file's job, the memory map, and how the build is proven correct |
| [GLOSSARY.md](GLOSSARY.md) | Every technical term used here, defined in plain language with comparisons to code you already know: kernel, GRUB, multiboot, linker script, `no_std`, and about a hundred more |
| [VGA.md](VGA.md) | How characters get on screen: the VGA text buffer, the driver module, what the optimiser made of it, and how the check proves the "42" is lit |
| [GDT.md](GDT.md) | The kernel's own descriptor table: why it needs one of its own, the descriptor layout, the six segments, the copy to `0x00000800`, the install sequence, and the proof |
| [STACK.md](STACK.md) | The stack printer: what the kernel stack is, the window the dump shows, the copy that avoids aliasing, the output format, and why there is no frame-pointer backtrace |
| [SHELL.md](SHELL.md) | The debug shell: the command table, the line editor, how a command reads its argument, each of the fourteen commands, and how the check drives the shell from outside the guest |
| [PAGING.md](PAGING.md) | Memory paging: what the hardware translates, the directory and its tables, the higher-half kernel, kernel and user space, page rights, the recursive entry, and what a page fault means |
| [MEMORY.md](MEMORY.md) | Where memory comes from and how it is handed out: GRUB's memory map, the frame bitmap, the boot-time layout, and the two heaps behind `kmalloc` and `vmalloc` |
| [PANIC.md](PANIC.md) | Kernel panics: the fatal and the recovered kind, the exception table and its stubs, every field of the report, and the three fatal paths the check proves |
| `kfs-1.subject.pdf` | The 42 assignment this kernel already answers: boot, screen, "42" |
| `kfs-2.subject.pdf` | The 42 assignment this kernel already answers: a Global Descriptor Table at `0x00000800` and a printable kernel stack |
| `kfs-3.subject.pdf` | The 42 assignment this kernel answers now: memory paging, kernel and user space, memory rights, the allocators, and kernel panics |

The three kfs-2 documents (GDT.md, STACK.md, SHELL.md) and the three kfs-3 ones
(PAGING.md, MEMORY.md, PANIC.md) are written in Simplified Technical English
(ASD-STE100): short sentences, active voice, and one word per idea.

## Where the project stands

Working and measured: a 32-bit x86 kernel that GRUB loads from a 2 752 512-byte
ISO. The image is loaded at physical `0x00101000` and runs at virtual
`0xc0101000`, three gigabytes up (an assembly boot stub plus a `no_std` Rust
kernel, linked with our own linker script), and it clears the screen and
displays the mandatory "42" ([VGA.md](VGA.md)), computed through the first
pieces of a kernel library (`utoa`, C-string helpers, and a text-to-number
parser the `virt` command uses).

All of kfs-1's bonuses are in too: colours, a tracked cursor with wrapping
and scrolling, the blinking hardware cursor driven through I/O ports,
`printk!` (`core`'s formatting engine hooked onto the screen, which also lets
panics print themselves in red), a polled PS/2 keyboard that echoes what you
type, and three virtual screens on F1/F2/F3.

kfs-2's mandatory part is done. The kernel installs its own Global Descriptor
Table at `0x00000800`: kernel code, kernel data, kernel stack, user code, user
data and user stack, after a null descriptor, each one flat over the whole
4 GiB. They are not merely declared but in force, and GRUB hands over other
values, so these cannot be inherited: the CPU reports `cs=0x08`, `ss=0x18`, and
`0x10` in `ds`, `es`, `fs` and `gs` ([GDT.md](GDT.md)). The table stays at
physical and linear `0x800`, reached through the low 4 MB, which is identity
mapped because `lgdt` takes a linear address. Next to it, a stack printer dumps
the live kernel stack the way a debugger shows memory: an address column, the
bytes in hex, and an ASCII column ([STACK.md](STACK.md)). That stack is
`0xc0110000..0xc0114000`, 16 KiB reserved in `.bss` by the boot stub, and
`stack_top` is `0xc0114000`.

kfs-3's mandatory part is done as well. Paging is on before the first Rust
instruction runs: 4 kb pages only, a page directory whose entry 1023 points at
the directory itself, and page tables allocated when a mapping needs one
([PAGING.md](PAGING.md)). Bit 16 of `CR0` is set, so the read-only mapping over
the kernel's own `.text` and `.rodata` binds ring 0 as well, and `fault ro`
proves it. Kernel space is `0xc0000000` upwards, user space is
`0x00400000..0xbfffffff`, the `space` command prints the whole layout, and the
`user` command maps a page in user space, writes through it, drops the write
right and unmaps it. Under all of that, a frame allocator holds one bit per
4 kb frame over the entire 32-bit address space, filled from GRUB's memory map:
on QEMU's default RAM that is 130 559 kb usable in 2 regions and 32 639 frames,
of which 24 263 are still free once low memory and the kernel image are
reserved. `kmalloc`, `kfree`, `ksize` and `kbrk` hand out bytes of one
contiguous physical block carved at boot; `vmalloc`, `vfree`, `vsize` and
`vbrk` hand out virtually contiguous bytes over frames taken one at a time, so
they are physically scattered. The `alloc` command shows the difference in the
addresses it prints ([MEMORY.md](MEMORY.md)). A fault no longer reboots the
machine in silence: a 32-vector exception table inside a 256-slot IDT catches
it, `kpanic!` prints in red, dumps the machine and halts the processor, and
`koops!` prints in yellow, counts, and returns. A page fault in the user demand
zone is one of the recovered ones: the handler maps a page and the instruction
is retried ([PANIC.md](PANIC.md)).

The kfs-2 bonus grew with it. The debug shell on the `kfs> ` prompt now holds
fourteen commands, and commands take arguments: `dispatch` splits the line at
the first space, looks the name up in the table, and passes the rest to the
handler as a string. `virt`, `fault` and `panic` read it; the others ignore it.
Eight commands are new in kfs-3: `mem`, `space`, `pages`, `virt`, `alloc`,
`user`, `fault` and `panic`, beside `stack`, `gdt`, `clear`, `reboot`, `halt`
and `help` ([SHELL.md](SHELL.md)).

`tools/check.sh` holds 36 assertions, and all of them pass. Every one is asked
from outside the guest through the QEMU monitor, so nothing inside the kernel
takes part in its own proof.

Deliberately absent, with the subject that owns each:

- **Interrupts are never enabled.** `sti` appears nowhere in the tree. The IDT
  exists because a page fault with no handler triple-faults, and a triple fault
  is the opposite of "print, stop the kernel". Hardware interrupts are kfs-4.
- **The PIC is not remapped.** The two 8259 controllers sit where the BIOS left
  them, with the first eight IRQs over the CPU's own exception vectors, and the
  keyboard stays polled. kfs-4, with the interrupts that give it a point.
- **Nothing runs in ring 3.** The user code, data and stack descriptors are in
  the GDT and user *pages* are real, but no instruction has executed at
  privilege level 3. Leaving ring 0 needs a way back in, so kfs-4 comes first
  and the ring switch itself belongs to a later subject.
- **No processes and no scheduler.** There is one thread of control: `kmain`,
  then the shell's poll loop for ever. A later subject owns tasks and
  switching.
- **No swapping and no page cache.** A page is either backed by a frame or
  absent, and nothing is ever evicted to a disk this kernel cannot talk to.
  Later still.

## Reading order

**New to this?** Read the first three sections of [GLOSSARY.md](GLOSSARY.md)
(operating systems, the x86 processor, booting), then
[ARCHITECTURE.md](ARCHITECTURE.md) top to bottom: it leans on glossary terms as
it goes.

**Following the subjects in order?** [VGA.md](VGA.md) is kfs-1.
[GDT.md](GDT.md), [STACK.md](STACK.md) and [SHELL.md](SHELL.md) are kfs-2. For
kfs-3, read [PAGING.md](PAGING.md) first, because it defines the address space
everything else sits in, then [MEMORY.md](MEMORY.md) for what fills that space,
then [PANIC.md](PANIC.md) for what happens when an address has no meaning.

**Just want to build and run it?** That is the [root README](../README.md).

**Wondering what the screen should show?** A black screen, a white "42", a
green `kfs-3`, then the exception-table line, the two memory lines, and the
hint line "type \`help\` for the command list", then a `kfs> ` prompt with the
blinking cursor parked right after. Type a command and the shell runs it,
rather than only echoing it.
[ARCHITECTURE.md §5](ARCHITECTURE.md#5-what-the-screen-shows) explains each
part, with measurements.
