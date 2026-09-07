# kfs-3

A kernel that boots on a bare 32-bit x86 PC, prints "42", replaces GRUB's
segmentation with its own descriptor table, and answers your keyboard from a
debug shell that can dump its own stack: an assembly boot stub, a `no_std` Rust
kernel with VGA and PS/2 drivers, our own linker script, packaged as a bootable
GRUB ISO.

It now pages memory in 4 kb pages and runs in the higher half, at 3 GB, with
its own code mapped read only. Kernel space and user space are a fact of the
address space rather than a comment, a frame allocator hands out physical
memory, `kmalloc` and `vmalloc` hand out bytes of it, and a fault the kernel
did not plan for prints a kernel panic and stops the processor instead of
rebooting the machine in silence.

"Kernel" means the software that would normally be *underneath* your program:
the thing `printf` and `malloc` are asking for help from. There is nothing
underneath this one.

How the screen works (colours, scrolling, and why the blinking cursor lives in
I/O ports rather than memory) is in [docs/VGA.md](docs/VGA.md). The kfs-2 parts
have one document each: [docs/GDT.md](docs/GDT.md) for the descriptor table,
[docs/STACK.md](docs/STACK.md) for the stack dump, and
[docs/SHELL.md](docs/SHELL.md) for the shell. So do the kfs-3 parts:
[docs/PAGING.md](docs/PAGING.md) for the page tables, the rights they carry and
the address space they define, [docs/MEMORY.md](docs/MEMORY.md) for the frame
allocator and the two heaps, and [docs/PANIC.md](docs/PANIC.md) for the fatal
and recovered panics and the exception table behind them.

**Never worked on a kernel before?** Start with
[docs/](docs/): [ARCHITECTURE.md](docs/ARCHITECTURE.md) explains how every piece
works for a reader who knows C or Python but not assembly, and
[GLOSSARY.md](docs/GLOSSARY.md) defines every term used. This file is only build
and run instructions.

## Layout

```
boot/boot.asm     multiboot header, boot page tables, paging on, 16 KiB stack,
                  _start -> the higher half -> kmain
kernel/           no_std Rust staticlib (custom i686 bare-metal target)
  lib.rs          kmain: gdt, clear, "42", banner, idt, mem::init, shell
  gdt.rs          our own descriptor table, copied to 0x00000800
  idt.rs          256 slots, 32 exception gates, the assembly stubs behind them
  multiboot.rs    GRUB's memory map: which physical RAM exists
  pmm.rs          physical frame allocator, one bit per 4 kb frame
  paging.rs       directory, tables, rights, translation, the fault handler
  mem.rs          the address space layout, and the boot-time memory setup
  heap.rs         kmalloc/kfree/ksize/kbrk and vmalloc/vfree/vsize/vbrk
  panic.rs        kpanic! fatal, koops! recovered, the machine dump
  stack.rs        hex dump of the live kernel stack
  shell.rs        the debug shell, its command table and its arguments
  vga.rs          screen driver; keyboard.rs polled PS/2; port.rs inb/outb
  printk.rs       printk! on top of core's formatter; klib.rs utoa, parse_u32
linker.ld         two parts: .boot at 1 MiB where GRUB jumps, kernel at 3 GB
grub.cfg          single "kfs" menuentry, multiboot /boot/kernel.bin
Makefile          native build: nasm -> cargo -> ld -> grub-mkrescue
tools/check.sh    the proof: 36 assertions asked through the qemu monitor
Dockerfile        dev-only container for hosts without an x86 toolchain
```

## Build natively (Fedora / Debian)

Fedora dependencies:

```sh
sudo dnf install nasm binutils gcc glibc-devel grub2-tools grub2-tools-extra \
                 grub2-pc-modules xorriso mtools qemu-system-x86 socat rustup
rustup-init -y --default-toolchain nightly-2026-08-12 --component rust-src
```

Debian/Ubuntu equivalents: `nasm binutils gcc libc6-dev grub-pc-bin
grub-common xorriso mtools qemu-system-x86 socat`, plus nightly Rust with
`rust-src` from <https://rustup.rs>.

(`gcc` and the libc headers are only needed to build one dependency's *host*
build script. Nothing links against libc: the kernel gets `core` and nothing
else.)

Then:

```sh
make          # build kfs.iso (fails if over 10 MB)
make run      # boot it in qemu
make check    # headless end-to-end proof, 36 assertions (needs socat)
make clean    # remove build/ and cargo artifacts
make fclean   # clean + remove kfs.iso
make re       # fclean + all
```

The image is built without GRUB's fonts, locales and themes (`--fonts=
--locales= --themes=`, `GRUBFLAGS` in the Makefile). Fedora's
`grub2-tools-extra` ships a 2.4 MiB Unicode font and ~6 MiB of translations
that `grub-mkrescue` copies in by default, enough on their own to push the ISO
past the 10 MB limit. A single English menuentry uses none of them.

## Build on a machine that has none of this (school machines)

```sh
./setup.sh
```

That is the whole procedure on a freshly cloned repo, including a school
machine where you have no root, which is what the script is for. It installs
the build tools, installs Rust, then runs `make check` so you see the proof
rather than take its word. Run it twice and the second run installs nothing.

What it works around:

*No root.* `toolbox` gives you a Fedora container you own, so `sudo` inside it
needs no password. The script creates one only if the host is missing tools
(~500 MB the first time). Afterwards, build with `toolbox enter` then `make`.

*No Rust.* `rustup` installs into `$HOME`, no root either; `rust-toolchain.toml`
pins the nightly and `rust-src` the build needs. Budget ~1.6 GB, enough to
matter on the 4.7 GB home partition of a school machine, so the script measures
the free space and refuses early rather than dying half-installed. `RUSTUP_HOME`
and `CARGO_HOME` can move it, but only to a path under `$HOME`: a toolbox
shares that and no other disk (`/goinfre` is invisible from inside).

*No `PATH` entry for cargo.* `rustup` only edits shell rc files, which a login
shell or a bare `make` may never read; the Makefile falls back to
`$CARGO_HOME/bin/cargo` on its own.

The failure the script exists to prevent is a quiet one. Without the BIOS boot
modules (`grub2-pc-modules` on Fedora, `grub-pc-bin` on Debian),
`grub-mkrescue` still writes an ISO and says nothing: a ~380 KB one with no
boot sector, on which the guest never leaves the BIOS. The build refuses that
image now — the ISO rule asks `xorriso` for the El Torito boot record and fails
when there is none — and `make check` catches it too (`EIP=0x0000b78c`,
real-mode SeaBIOS, outside the kernel), which is exactly why the script ends by
running it.

## Running it

`make run` opens a qemu window. This is the screen it shows, captured through
the qemu monitor from the shipped image:

```
42
kfs-3
faults: 256 vectors at 0xc0114004, 32 exceptions wired, interrupts off
memory: 130559 kb usable in 2 regions, 32639 frames of 4 kb
paging: directory at 0x0010e000, 32636 kb carved at 0x0013a000 for kmalloc
type `help` for the command list
kfs>
```

The "42" is white, `kfs-3` is bright green, the three report lines and the hint
are white, and the `kfs> ` prompt is bright cyan with the blinking hardware
cursor parked right after it (the driver moves it through I/O ports after every
print, [docs/VGA.md](docs/VGA.md)). The memory figures are qemu's defaults and
change with `-m`; the address of the exception table moves when the kernel is
rebuilt.

Click into the window and type: characters echo at the cursor, Backspace
erases, Enter runs the line as a command, and F1/F2/F3 switch between three
independent screens. `help` lists the fourteen commands:

```
  mem     physical memory, frames and both heaps
  space   the kernel and user address space layout
  pages   every mapping in the page directory
  virt    translate an address: virt [addr]
  alloc   exercise kmalloc, vmalloc, kbrk and vbrk
  user    map, use and drop a user space page
  fault   make a page fault: fault [demand|ro|kernel]
  panic   panic on purpose: panic [oops|fatal]
  stack   hex dump of the live kernel stack
  gdt     the descriptor table, read back from the CPU
  clear   blank the screen
  reboot  restart the machine through the 8042
  halt    stop this processor for good
  help    this list
```

Eight of them are new in kfs-3, and three take an argument: `virt`, `fault` and
`panic`. `dispatch` splits the typed line at the first space, looks the name up
in the table, and hands the rest to the handler as a string; the other eleven
commands ignore it ([docs/SHELL.md](docs/SHELL.md)). `gdt` reads the descriptor
table back out of the CPU ([docs/GDT.md](docs/GDT.md)), `pages` walks the page
directory the CPU is using ([docs/PAGING.md](docs/PAGING.md)), and `stack`
hex-dumps the stack the shell is standing on
([docs/STACK.md](docs/STACK.md)).

The eyeball test is not the proof, though: `make check` is. You cannot `assert`
from inside a kernel (no test harness, no exit status, nowhere to print), so the
check asks the emulator from the outside what state the machine ended up in.
`tools/check.sh` boots the ISO with no display and talks to the qemu monitor
over a unix socket. It polls `info registers` until the kernel is reached (up to
60 s, because boot is slow inside the emulated dev container), reads guest
physical memory with `xp`, reads the VGA text buffer at `0xb8000` and decodes
the character byte of every cell, walks the guest's real page tables with
`info mem`, types into the guest with `sendkey`, one key per call, because
`sendkey` has no string form — then reads the echoed line back out of video
memory and retypes it if a key went missing — and pulls the reset line with
`system_reset` to get a live machine back after a fatal panic has halted the
processor. Long monitor replies (the screen is 20 KB of `xp` output) are
drained until they go quiet before the connection closes: hanging up on the
monitor mid-reply wedges it for the rest of the run, which on a loaded host
used to fail one random assertion per run. The
assertions of a full `make check`, the first two from the build rules and the
other thirty-six from the script:

```
OK: build/kernel.bin is freestanding (no undefined symbols)
OK: kfs.iso is 2752512 bytes (limit 10485760)
OK: guest alive, EIP=<eip> inside the kernel at 3 gb
OK: GDT still at 0x00000800, limit 0x00000037 (7 descriptors)
OK: cs=0008 ss=0018 ds=es=fs=gs=0010, all from the new table
OK: null + kernel code/data/stack + user code/data/stack at 0x800
OK: CR0=80010011, paging is on
OK: CR0=80010011, write protect is on, so ring 0 obeys the read only bit
OK: CR3=0010e000, a page aligned directory
OK: the low window 0x00000000-0x00400000 is mapped, the GDT lives there
OK: the kernel window at 0xc0000000 is mapped, 4 mb of ram at 3 gb
OK: the kernel's own code is mapped read only: c0101000-c010d000
OK: the page table window is mapped, the directory sees itself
OK: row 0 shows the mandatory "42"
OK: row 1 shows the kfs-3 banner
OK: the boot reports the exception table
OK: the kernel and the cpu agree on cr3 0010e000
OK: shell prompt is on screen
OK: `help` lists the commands
OK: `mem` reports ram, frames, the carved block and both heaps
OK: `space` names every region and its owner
OK: `pages` walks the directory and shows the rights it holds
OK: `virt` walks directory 768, table 256 down to physical 0x00100000
OK: `alloc` passed 14 checks over kmalloc, vmalloc, kbrk and vbrk
OK: kmalloc hands out the start of the kernel heap
OK: `user` maps, writes, protects and drops a page in user space
OK: a page fault in the demand zone is recovered, the kernel keeps running
OK: `panic oops` prints and returns, this panic is not fatal
OK: header is self-consistent: <used> of 16384 bytes below 0xc0114000
OK: first row starts at 0x<esp>, the address the header names
OK: the first dword is 0x<addr>, a kernel pointer from outside the stack
OK: dump esp 0x<esp> is <n> bytes deeper than ESP=0x<esp>
OK: `reboot` restarted the machine, the table and paging came back
OK: a write to read only kernel code panics and stops the processor
OK: the machine boots again for the remaining fatal cases
OK: a write to unmapped kernel space panics and stops the processor
OK: `panic fatal` prints, dumps the machine and stops the processor
OK: `halt` stopped the processor (HLT=1)
OK: every assertion passed
```

The angle brackets are values filled in at run time: an instruction pointer, a
stack address and a byte count, all of which depend on where the shell was when
the script asked. Every other number above is fixed and was measured from this
build.

Nothing inside the guest takes part, which is the whole point: a kernel that
only claims to work cannot pass. What the assertions cover:

*The build.* No undefined symbol in `kernel.bin` (so nothing expects a host
library GRUB cannot provide), and the ISO under the 10 MB limit, at 2 752 512
bytes.

*The boot.* qemu is still alive to answer, and the instruction pointer is inside
the kernel, within `0xC0100000..0xC0200000`. The image is loaded at physical
`0x00101000` and runs at virtual `0xC0101000`, so an `EIP` in that megabyte at
3 GB means GRUB accepted our binary, the stub filled a page directory and
switched paging on, the jump to the higher half landed, `esp` was set, and Rust
ran to the shell's poll loop, which is where `EIP` ends up. GRUB executes both
below 1 MiB and, relocated, near the top of RAM, so a lower bound alone would
not be enough. An unreachable monitor, or an `EIP` outside that range, means
the guest died or never arrived.

*The descriptor table.* `GDTR` names `0x00000800` with limit `0x37`, the seven
descriptors read back byte for byte out of guest physical memory, and the
segment registers hold the new selectors. GRUB hands over `cs=0x10` and the data
registers at `0x18`, so `cs=0x08`, `ss=0x18` and `0x10` in `ds`, `es`, `fs` and
`gs` cannot be inherited ([docs/GDT.md](docs/GDT.md)). `lgdt` takes a linear
address, which is why the low 4 MB stays identity mapped once paging is on:
linear `0x800` has to keep pointing at physical `0x800`.

*Paging.* `CR0` has bit 31, paging, and bit 16, write protect, so ring 0 obeys
the read-only bit instead of ignoring it. `CR3` is page aligned, and the
directory address the kernel printed at boot equals the `CR3` the CPU holds, so
the kernel is describing the live table rather than one it remembers. Then four
ranges out of `info mem`, which is the emulator walking the real tables instead
of believing the kernel: the low identity window, the kernel window at
`0xC0000000`, a read-only range inside the kernel image, and the page-table
window the recursive directory entry creates
([docs/PAGING.md](docs/PAGING.md)).

*The screen.* Exact text, not pixel colour: the check decodes the character byte
of each cell, so it can assert the "42" on row 0, the `kfs-3` banner on row 1,
the exception-table line, and the prompt.

*The shell.* `help`, `mem`, `space`, `pages`, `virt`, `alloc`, `user`, `fault`,
`panic`, `stack`, `reboot` and `halt`, each typed in as keystrokes, with `clear`
between them to keep the screen readable. `virt 0xc0100000` has to split the
address into directory 768, table 256 and offset 0, and come back with physical
`0x00100000`. `alloc` has to report its fourteen checks over `kmalloc`,
`vmalloc`, `kbrk` and `vbrk` passed, the first `kmalloc` landing at
`0xd0000008`, the base of the kernel heap. `user` has to map a page in user
space, write `0x42424242` through it and read it back, drop the write right,
refuse a user page inside kernel space, and unmap it
([docs/MEMORY.md](docs/MEMORY.md)).

*The dump.* It has to agree with itself and with the CPU: the header's byte
count matches its own addresses over the 16 KiB stack, the first row starts
where the header says, the dword at the bottom of the window is a return
address in the higher half rather than a stack address (which is what a dump
that overwrote its own source would show, [docs/STACK.md](docs/STACK.md)), and
the captured `esp` sits just below the live one.

*The panics.* Two assertions for the recovered path, four for the fatal one. A
page fault in the user demand zone has to print a recovered panic, be paged in,
and leave the kernel running; `panic oops` has to print and return to the
prompt. Then `fault ro` (a write to the kernel's own read-only code: the page
is present and its rights refuse), `fault kernel` (a write to kernel space with
no page) and `panic fatal` (software) each have to print a `KERNEL PANIC`
report, name the cause, and leave the processor with `HLT=1`. Between them the
script pulls the reset line, because a halted machine answers nothing else
([docs/PANIC.md](docs/PANIC.md)).

*The restart.* `reboot` has to bring the machine back with the table installed
again at the same address and paging still on, and `halt` has to leave the
processor with `HLT=1`.

## Build on a host without an x86 toolchain (e.g. macOS/arm64)

Any target runs inside the dev container with a `docker-` prefix. The repo is
bind-mounted, so artifacts land in the working tree as usual:

```sh
make docker-all      # build kfs.iso
make docker-check    # headless end-to-end proof
make docker-re       # clean rebuild
```

For an actual qemu window on macOS, boot the ISO from the host (building needs
Linux, booting does not):

```sh
brew install qemu
qemu-system-i386 -cdrom kfs.iso
```

Call qemu directly rather than `make run`: `run` depends on the build chain
(`nasm`, `cargo`, `ld`), which is absent on macOS, so make would fail trying to
relink before it ever booted anything.

