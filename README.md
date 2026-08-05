# kfs-1

Minimal bootable x86 (i386) kernel: NASM multiboot 1 boot code, a `no_std`
Rust kernel, a custom linker script, packaged as a GRUB rescue ISO.

Screen output ("42") is not part of this bring-up — see [docs/VGA.md](docs/VGA.md).

**Documentation:** [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) explains how
every piece works, [docs/GLOSSARY.md](docs/GLOSSARY.md) defines every technical
term used, and [docs/](docs/) indexes both. This file is just build and run
instructions.

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

(`gcc` + the libc headers are only needed to build `compiler_builtins`' host
build script; nothing links against libc — the kernel is `core`-only.)

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

`make run` opens a qemu window. Expect a blank screen with three lines of grey
text in the top-left corner — that is the correct result today. The text is
GRUB's residual loading output: `kmain` only spins, nothing writes to the VGA
text buffer at `0xb8000` yet, so nothing clears or overwrites what GRUB left
behind. A `screendump` of the running guest confirms it: 720x400 frame, 376 lit
pixels, all `#a8a8a8`, confined to text cells rows 0-2 / columns 0-16. The
mandatory "42" arrives with the VGA work described in
[docs/VGA.md](docs/VGA.md).

Because there is nothing to look at, the proof of life is `make check`. It
boots the ISO with no display, waits 5 s, then asks the qemu monitor for
`info registers` over a unix socket:

```
OK: kfs.iso is 5083136 bytes (limit 10485760)
OK: guest alive, EIP=00100032 inside kernel
```

It passes only if qemu is still alive to answer and `EIP` is at or above 1 MiB.
`kmain` links at `0x100030` (`nm build/kernel.bin`), so `EIP=0x100032` is two
bytes into its spin loop: GRUB accepted the multiboot binary, `_start` set
`esp` and called into Rust, and the machine is not triple-faulting and
rebooting. An unreachable monitor, or an `EIP` below 1 MiB, means the guest
died.

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
relink before it ever boots.
