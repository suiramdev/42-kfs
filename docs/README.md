# kfs documentation

`kfs` is a kernel: the piece of software that would normally be *underneath*
your program. Every `printf`, `malloc` and `open` you have ever written was a
request to one. This project has nothing underneath it at all — no files, no
heap, no threads, and no one to kill your process when you get a pointer wrong.
The machine just reboots.

These docs assume you write C, C++, Python or TypeScript and have never touched
assembly, a linker script, or a bootloader.

| Document | What it covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the whole thing works: power-on to the kernel's idle loop, every file's job, the memory map, and how the build is proven correct |
| [GLOSSARY.md](GLOSSARY.md) | Every technical term used here, defined in plain language with comparisons to code you already know — kernel, GRUB, multiboot, linker script, `no_std`, and about a hundred more |
| [VGA.md](VGA.md) | How characters get on screen: the VGA text buffer, the driver module, what the optimiser made of it, and how the check proves the "42" is lit |
| `kfs-1.subject.pdf` | The 42 assignment this kernel already answers: boot, screen, "42" |
| `kfs-2.subject.pdf` | The next 42 assignment: a Global Descriptor Table at `0x00000800` and a printable kernel stack |

## Where the project stands

Working and measured: a 32-bit x86 kernel that GRUB loads from a 5 MB ISO and
runs at address 1 MiB — an assembly boot stub plus a `no_std` Rust kernel,
linked with our own linker script — which clears the screen and displays the
mandatory "42" ([VGA.md](VGA.md)), computed through the first pieces of a
kernel library (`utoa`, plus C-string helpers for the multiboot data to come).

All of kfs-1's bonuses are in too: colours, a tracked cursor with wrapping
and scrolling, the blinking hardware cursor driven through I/O ports,
`printk!` — `core`'s formatting engine hooked onto the screen, which also lets
panics print themselves in red — a polled PS/2 keyboard that echoes what you
type, and three virtual screens on F1/F2/F3.

Not started: kfs-2, whose mandatory part is our own Global Descriptor Table —
GRUB's flat segments replaced by six of our own at a fixed address — and a
tool that prints the kernel stack. Interrupts (what would make the keyboard
event-driven instead of polled) and paging come after that.

## Reading order

**New to this?** Read the first three sections of [GLOSSARY.md](GLOSSARY.md)
(operating systems, the x86 processor, booting), then
[ARCHITECTURE.md](ARCHITECTURE.md) top to bottom — it leans on glossary terms as
it goes.

**Just want to build and run it?** That is the [root README](../README.md).

**Wondering what the screen should show?** A black screen, a white "42", a
green "kfs-1", the blinking cursor parked right after — and your keystrokes
echoed once you type.
[ARCHITECTURE.md §5](ARCHITECTURE.md#5-what-the-screen-shows) explains each
part, with measurements.
