# kfs-1 documentation

`kfs-1` is a kernel: the piece of software that would normally be *underneath*
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
| [VGA.md](VGA.md) | The one thing deliberately **not** built yet: putting characters on screen, and the design for it |
| `en.subject.pdf` | The original 42 assignment |

## Where the project stands

Working and measured: a 32-bit x86 kernel that GRUB loads from a 5 MB ISO and
runs at address 1 MiB — an assembly boot stub plus a `no_std` Rust kernel,
linked with our own linker script.

Not built yet: anything visible. `kmain` idles in a two-instruction loop, so the
screen still shows GRUB's last message. The mandatory "42" is the next step
([VGA.md](VGA.md)); interrupts, our own segment table and paging come after.

## Reading order

**New to this?** Read the first three sections of [GLOSSARY.md](GLOSSARY.md)
(operating systems, the x86 processor, booting), then
[ARCHITECTURE.md](ARCHITECTURE.md) top to bottom — it leans on glossary terms as
it goes.

**Just want to build and run it?** That is the [root README](../README.md).

**Wondering why the screen looks frozen?** It is supposed to.
[ARCHITECTURE.md §5](ARCHITECTURE.md#5-why-the-screen-looks-frozen) explains
what the machine is doing, with measurements.
