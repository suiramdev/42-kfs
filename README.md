# kfs-1

Minimal bootable x86 (i386) kernel: NASM multiboot 1 boot code, a `no_std`
Rust kernel, a custom linker script, packaged as a GRUB rescue ISO.

Screen output ("42") is not part of this bring-up — see [docs/VGA.md](docs/VGA.md).

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

`make check` boots the ISO with no display, waits 5 s, then asks the qemu
monitor for `info registers`. It passes only if qemu is still alive and `EIP`
is at or above 1 MiB — i.e. GRUB accepted the multiboot binary and `kmain` is
spinning instead of the machine triple-faulting.

## Build on a host without an x86 toolchain (e.g. macOS/arm64)

Any target can be run inside the dev container with a `docker-` prefix:

```sh
make docker-all
make docker-check
make docker-re
```

These mount the repo into a Debian amd64 image and run the same native rules.
