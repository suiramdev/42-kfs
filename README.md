# kfs-2

A kernel that boots on a bare 32-bit x86 PC, prints "42", replaces GRUB's
segmentation with its own descriptor table, and answers your keyboard from a
debug shell that can dump its own stack: an assembly boot stub, a `no_std` Rust
kernel with VGA and PS/2 drivers, our own linker script, packaged as a bootable
GRUB ISO.

"Kernel" means the software that would normally be *underneath* your program:
the thing `printf` and `malloc` are asking for help from. There is nothing
underneath this one.

How the screen works (colours, scrolling, and why the blinking cursor lives in
I/O ports rather than memory) is in [docs/VGA.md](docs/VGA.md). The kfs-2 parts
have one document each: [docs/GDT.md](docs/GDT.md) for the descriptor table,
[docs/STACK.md](docs/STACK.md) for the stack dump, and
[docs/SHELL.md](docs/SHELL.md) for the shell.

**Never worked on a kernel before?** Start with
[docs/](docs/): [ARCHITECTURE.md](docs/ARCHITECTURE.md) explains how every piece
works for a reader who knows C or Python but not assembly, and
[GLOSSARY.md](docs/GLOSSARY.md) defines every term used. This file is only build
and run instructions.

## Layout

```
boot/boot.asm     multiboot header, 16 KiB stack, _start -> kmain
kernel/           no_std Rust staticlib (custom i686 bare-metal target)
  lib.rs          kmain: install the table, clear, "42", banner, shell
  gdt.rs          our own descriptor table, copied to 0x00000800
  stack.rs        hex dump of the live kernel stack
  shell.rs        the debug shell and its command table
  vga.rs          screen driver; keyboard.rs polled PS/2; port.rs inb/outb
  printk.rs       printk! on top of core's formatter; klib.rs utoa, strlen
linker.ld         kernel load layout (1 MiB, .multiboot first and kept)
grub.cfg          single "kfs" menuentry, multiboot /boot/kernel.bin
Makefile          native build: nasm -> cargo -> ld -> grub-mkrescue
tools/check.sh    the proof: 20 assertions asked through the qemu monitor
Dockerfile        dev-only container for hosts without an x86 toolchain
```

## Build natively (Fedora / Debian)

Fedora dependencies:

```sh
sudo dnf install nasm binutils gcc glibc-devel grub2-tools grub2-tools-extra \
                 grub2-pc-modules xorriso mtools qemu-system-x86 socat rustup
rustup-init -y --default-toolchain nightly --component rust-src
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
make check    # headless end-to-end proof, 20 assertions (needs socat)
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
boot sector, on which the guest never leaves the BIOS. `make check` catches it
(`EIP=0x0000b78c`, real-mode SeaBIOS, outside the kernel), which is exactly why
the script ends by running it.

## Running it

`make run` opens a qemu window showing a black screen with a white "42" on the
first row, a bright-green "kfs-2" on the second, the hint "type \`help\` for the
command list" on the third, and a `kfs> ` prompt on the fourth with the
blinking hardware cursor parked right after (the driver moves it
through I/O ports after every print, [docs/VGA.md](docs/VGA.md)). Click into the
window and type: characters echo at the cursor, Backspace erases, Enter runs
the line as a command, and F1/F2/F3 switch between three independent screens.
`help` lists the six commands, `gdt` reads the descriptor table back out of the
CPU, and `stack` hex-dumps the stack the shell is standing on
([docs/SHELL.md](docs/SHELL.md)).

The eyeball test is not the proof, though: `make check` is. You cannot `assert`
from inside a kernel (no test harness, no exit status, nowhere to print), so the
check asks the emulator from the outside what state the machine ended up in.
`tools/check.sh` boots the ISO with no display and talks to the qemu monitor
over a unix socket. It polls `info registers` until the kernel is reached (up to
60 s, because boot is slow inside the emulated dev container), reads guest
physical memory with `xp`, reads the VGA text buffer at `0xb8000` and decodes
the character byte of every cell, and types into the guest with `sendkey`, one
key per call, because `sendkey` has no string form. The twenty assertions of a
full `make check`, the first two from the build rules themselves:

```
OK: build/kernel.bin is freestanding (no undefined symbols)
OK: kfs.iso is 2705408 bytes (limit 10485760)
OK: guest alive, EIP=001008a7 inside the kernel
OK: GDT at 0x00000800, limit 0x00000037 (7 descriptors)
OK: cs=0008 ss=0018 ds=es=fs=gs=0010, all from the new table
OK: null + kernel code/data/stack + user code/data/stack at 0x800
OK: row 0 shows the mandatory "42"
OK: row 1 shows the kfs-2 banner
OK: shell prompt is on screen
OK: `help` lists the commands
OK: `gdt` reads the table back from the CPU and agrees with the source
OK: `clear` blanked the screen
OK: header is self-consistent: 168 of 16384 bytes below 0x00107710
OK: 168 bytes rendered as 11 rows of 16
OK: first row starts at 0x00107668, the address the header names
OK: the first dword is 0x0010063c, a return address inside the kernel
OK: the ASCII column shows the typed command in the line buffer
OK: dump esp 0x00107668 is 4 bytes deeper than ESP=0x0010766c
OK: `reboot` restarted the machine, and the table came back at 0x00000800
OK: `halt` stopped the processor (HLT=1)
```

Nothing inside the guest takes part, which is the whole point: a kernel that
only claims to work cannot pass. What the assertions cover:

*The build.* No undefined symbol in `kernel.bin` (so nothing expects a host
library GRUB cannot provide), and the ISO under the 10 MB limit.

*The boot.* qemu is still alive to answer, and the instruction pointer is inside
the kernel, within [1 MiB, 2 MiB): GRUB executes both below 1 MiB and,
relocated, near the top of RAM, so a lower bound alone is not enough. That means
GRUB accepted our binary, `_start` set up a stack, and Rust ran to the shell's
poll loop, which is where `EIP` lands. An unreachable monitor, or an `EIP`
outside the kernel, means the guest died or never arrived.

*The descriptor table.* `GDTR` names `0x00000800` with limit `0x37`, the seven
descriptors read back byte for byte out of guest physical memory, and the
segment registers hold the new selectors. GRUB hands over `cs=0x10` and the data
registers at `0x18`, so `cs=0x08`, `ss=0x18` and `0x10` in `ds`, `es`, `fs` and
`gs` cannot be inherited ([docs/GDT.md](docs/GDT.md)).

*The screen.* Exact text, not pixel colour: the check decodes the character byte
of each cell, so it can assert the "42" on row 0, the `kfs-2` banner on row 1,
and the prompt.

*The shell and the dump.* `help`, `gdt`, `clear`, `stack`, `reboot` and `halt`,
each typed in as keystrokes. The dump has to agree with itself and with the CPU:
the header's byte count matches its own addresses, every byte in the window is
rendered 16 to a row, the first row starts where the header says, the dword at
the bottom of the window is a return address into the kernel rather than a stack
address (which is what a dump that overwrote its own source would show,
[docs/STACK.md](docs/STACK.md)), and the captured `esp` sits just below the live
one. `reboot` has to bring the machine back with the table installed again at
the same address, and `halt` has to leave the processor with `HLT=1`.

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

