# kfs-1 documentation

| Document | What it covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | What is implemented and how it works: the boot chain, every file's role, the memory map, and how the build is verified |
| [GLOSSARY.md](GLOSSARY.md) | Every technical term used here, defined — kernel, GRUB, assembly, multiboot, linker script, `no_std`, and ~100 more |
| [VGA.md](VGA.md) | The one thing deliberately **not** implemented yet: screen output, and the design for it |
| `en.subject.pdf` | The original 42 assignment |

## Where the project stands

Implemented and verified: a 32-bit x86 kernel that GRUB loads from a 5 MB ISO
and executes at 1 MiB, written as an assembly boot stub plus a `no_std` Rust
kernel, linked with a custom linker script.

Not implemented: everything visible. `kmain` idles in a two-instruction loop,
so the screen keeps showing GRUB's last message. The mandatory "42" is the next
step ([VGA.md](VGA.md)); interrupts, GDT, paging and the rest come after.

## Reading order

New to kernel work? Read [GLOSSARY.md](GLOSSARY.md)'s first three sections
(operating systems, the x86 processor, booting), then
[ARCHITECTURE.md](ARCHITECTURE.md) top to bottom — it references glossary terms
as it goes.

Just want to build and run it? That's the [root README](../README.md).

Wondering why the screen is frozen on `Booting "kfs-1"`? That is expected, and
[ARCHITECTURE.md §4](ARCHITECTURE.md#4-what-the-machine-is-doing-right-now)
explains what the machine is doing, with measurements.
