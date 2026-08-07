# kfs-1

A kernel that boots on a bare 32-bit x86 PC, displays "42" and echoes your
keyboard across three virtual screens: an assembly boot stub, a `no_std` Rust
kernel with VGA and PS/2 drivers, our own linker script, packaged as a bootable
GRUB ISO.

"Kernel" means the software that would normally be *underneath* your program —
the thing `printf` and `malloc` are asking for help from. There is nothing
underneath this one.

How the screen works — colours, scrolling, and why the blinking cursor lives
in I/O ports rather than memory — is in [docs/VGA.md](docs/VGA.md).

**Never worked on a kernel before?** Start with
[docs/](docs/): [ARCHITECTURE.md](docs/ARCHITECTURE.md) explains how every piece
works for a reader who knows C or Python but not assembly, and
[GLOSSARY.md](docs/GLOSSARY.md) defines every term used. This file is only build
and run instructions.

## Layout

```
boot/boot.asm     multiboot header, stack, _start -> kmain
kernel/           no_std Rust staticlib (custom i686 bare-metal target)
linker.ld         kernel load layout (1 MiB, .multiboot first)
grub.cfg          single "kfs-1" menuentry, multiboot /boot/kernel.bin
Makefile          native build: nasm -> cargo -> ld -> grub-mkrescue
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
build script. Nothing links against libc — the kernel gets `core` and nothing
else.)

Then:

```sh
make          # build kfs.iso (fails if over 10 MB)
make run      # boot it in qemu
make check    # headless boot proof (needs socat)
make clean    # remove build/ and cargo artifacts
make fclean   # clean + remove kfs.iso
make re       # fclean + all
```

## Running it

`make run` opens a qemu window showing a black screen with a white "42" on the
first row, a bright-green "kfs-1" on the second, and the blinking hardware
cursor parked right after — the driver moves it through I/O ports after every
print ([docs/VGA.md](docs/VGA.md)). Click into the window and type: characters
echo at the cursor (Shift, Enter, Backspace all work) and F1/F2/F3 switch
between three independent screens.

The eyeball test is not the proof, though — `make check` is. You cannot
`assert` from inside a kernel — no test harness, no exit status, nowhere to
print — so the check asks the emulator from the outside what state the machine
ended up in. It boots the ISO with no display, then over a unix socket polls
the qemu monitor for `info registers` until the kernel is reached (up to 60 s
— boot is slow inside the emulated dev container) and takes a framebuffer
dump:

```
OK: kfs.iso is 5085184 bytes (limit 10485760)
OK: guest alive, EIP=001002fa inside kernel
OK: screen cleared and "42" glyphs lit
```

Three facts have to hold: qemu is still alive to answer, the instruction
pointer is inside the kernel (within [1 MiB, 2 MiB) — GRUB executes both below
1 MiB and, relocated, near the top of RAM, so a lower bound alone is not
enough), and the dumped frame shows white pixels (the "42" glyphs) with none
of GRUB's grey `#a8a8a8` leftovers (so our clear really overwrote the screen).
`kmain` links at `0x100020` (`nm build/kernel.bin`) and spends its life in the
keyboard-polling loop, which is where `EIP` lands. That means GRUB accepted
our binary, `_start` set up a stack, and Rust ran the screen writes to
completion. An unreachable monitor, or an `EIP` outside the kernel, means the
guest died or never arrived.

## Build on a host without an x86 toolchain (e.g. macOS/arm64)

Any target runs inside the dev container with a `docker-` prefix. The repo is
bind-mounted, so artifacts land in the working tree as usual:

```sh
make docker-all      # build kfs.iso
make docker-check    # headless boot proof
make docker-re       # clean rebuild
```

For an actual qemu window on macOS, boot the ISO from the host — building needs
Linux, booting does not:

```sh
brew install qemu
qemu-system-i386 -cdrom kfs.iso
```

Call qemu directly rather than `make run`: `run` depends on the build chain
(`nasm`, `cargo`, `ld`), which is absent on macOS, so make would fail trying to
relink before it ever booted anything.

https://claude.ai/code/artifact/e734108f-81fa-48f8-92d7-2cd84eb65413
