# kfs documentation

`kfs` is a kernel: the piece of software that would normally be *underneath*
your program. Every `printf`, `malloc` and `open` you have ever written was a
request to one. This project has nothing underneath it at all: no files, no
heap, no threads, and no one to kill your process when you get a pointer wrong.
The machine just reboots.

These docs assume you write C, C++, Python or TypeScript and have never touched
assembly, a linker script, or a bootloader.

| Document | What it covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the whole thing works: power-on to the kernel's idle loop, every file's job, the memory map, and how the build is proven correct |
| [GLOSSARY.md](GLOSSARY.md) | Every technical term used here, defined in plain language with comparisons to code you already know: kernel, GRUB, multiboot, linker script, `no_std`, and about a hundred more |
| [VGA.md](VGA.md) | How characters get on screen: the VGA text buffer, the driver module, what the optimiser made of it, and how the check proves the "42" is lit |
| [GDT.md](GDT.md) | The kernel's own descriptor table: why it needs one of its own, the descriptor layout, the six segments, the copy to `0x00000800`, the install sequence, and the proof |
| [STACK.md](STACK.md) | The stack printer: what the kernel stack is, the window the dump shows, the copy that avoids aliasing, the output format, and why there is no frame-pointer backtrace |
| [SHELL.md](SHELL.md) | The debug shell: the command table, the line editor, each of the six commands, and how the check drives the shell from outside the guest |
| `kfs-1.subject.pdf` | The 42 assignment this kernel already answers: boot, screen, "42" |
| `kfs-2.subject.pdf` | The 42 assignment this kernel answers now: a Global Descriptor Table at `0x00000800` and a printable kernel stack |

The three kfs-2 documents (GDT.md, STACK.md, SHELL.md) are written in
Simplified Technical English (ASD-STE100): short sentences, active voice, and
one word per idea.

## Where the project stands

Working and measured: a 32-bit x86 kernel that GRUB loads from a 2.7 MB ISO and
runs at address 1 MiB (an assembly boot stub plus a `no_std` Rust kernel,
linked with our own linker script), which clears the screen and displays the
mandatory "42" ([VGA.md](VGA.md)), computed through the first pieces of a
kernel library (`utoa`, plus C-string helpers for the multiboot data to come).

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
`0x10` in `ds`, `es`, `fs` and `gs` ([GDT.md](GDT.md)). Next to it, a stack
printer dumps the live kernel stack the way a debugger shows memory: an address
column, the bytes in hex, and an ASCII column ([STACK.md](STACK.md)).

The kfs-2 bonus is in as well: a debug shell on a `kfs> ` prompt, with six
commands (`stack`, `gdt`, `clear`, `reboot`, `halt`, `help`)
([SHELL.md](SHELL.md)).

Not started: interrupts and an Interrupt Descriptor Table, which is what would
make the keyboard event-driven instead of polled. Paging comes after that.

## Reading order

**New to this?** Read the first three sections of [GLOSSARY.md](GLOSSARY.md)
(operating systems, the x86 processor, booting), then
[ARCHITECTURE.md](ARCHITECTURE.md) top to bottom: it leans on glossary terms as
it goes.

**Just want to build and run it?** That is the [root README](../README.md).

**Wondering what the screen should show?** A black screen, a white "42", a
green "kfs-2", the hint line "type \`help\` for the command list", then a `kfs> `
prompt with the blinking cursor parked right after. Type a command and the
shell runs it, rather than only echoing it.
[ARCHITECTURE.md §5](ARCHITECTURE.md#5-what-the-screen-shows) explains each
part, with measurements.
