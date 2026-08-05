# kfs-1

A kernel that boots on a bare 32-bit x86 PC: an assembly boot stub, a `no_std`
Rust kernel, our own linker script, packaged as a bootable GRUB ISO.

"Kernel" means the software that would normally be *underneath* your program —
the thing `printf` and `malloc` are asking for help from. There is nothing
underneath this one.

Screen output ("42") is deliberately not part of this bring-up — see
[docs/VGA.md](docs/VGA.md).

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

`make run` opens a qemu window showing a nearly black screen with a few grey
characters in the top-left corner. **That is the correct result today.**

Those characters are GRUB's leftovers. `kmain` only idles; nothing in this
kernel writes to the screen yet, so nothing clears or overwrites what GRUB drew
on its way out. A frame dump confirms it: 720x400, 376 lit pixels, all
`#a8a8a8`, confined to text rows 0-2 and columns 0-16. The mandatory "42"
arrives with the work described in [docs/VGA.md](docs/VGA.md).

Since there is nothing to look at, the proof of life is `make check`. You cannot
`assert` from inside a kernel — no test harness, no exit status, nowhere to
print — so the check asks the emulator from the outside where the CPU ended up.
It boots the ISO with no display, waits 5 s, then requests `info registers` over
a unix socket from the qemu monitor:

```
OK: kfs.iso is 5083136 bytes (limit 10485760)
OK: guest alive, EIP=00100032 inside kernel
```

It passes only if qemu is still alive to answer *and* the instruction pointer is
at or above 1 MiB, where the kernel was loaded. `kmain` links at `0x100030`
(`nm build/kernel.bin`), so `EIP=0x100032` is two bytes in — parked on the `jmp`
of its idle loop. That means GRUB accepted our binary, `_start` set up a stack
and called into Rust, and the machine is not faulting and rebooting in a loop.
An unreachable monitor, or an `EIP` below 1 MiB, means the guest died.

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
