NAME          := kfs.iso
BUILD         := build
ISODIR        := $(BUILD)/iso

NASM          ?= nasm
NASMFLAGS      = -f elf32
LD            ?= ld
# --gc-sections drops every section nothing reaches from `_start`. rustc builds
# compiler_builtins as one 318 KiB object file, so needing a single helper from
# it makes `ld` pull the whole member in, soft-float f128 maths included.
# Measured: kernel.bin was 150112 bytes and held 61 float symbols; it is now
# 17828 bytes and holds none. `linker.ld` KEEPs the multiboot header, which
# nothing references and would otherwise be collected.
LDFLAGS        = -m elf_i386 -n -nostdlib --gc-sections -T linker.ld
RUSTFLAGS     ?= -C panic=abort -C no-redzone=y -Z stack-protector=none
# Fedora ships grub2-*, Debian ships grub-*.
GRUB_MKRESCUE ?= $(shell command -v grub-mkrescue || command -v grub2-mkrescue)
GRUB_FILE     ?= $(shell command -v grub-file || command -v grub2-file)
# grub-mkrescue copies whatever GRUB's data directory holds into the image.
# On Fedora that is a 2.4 MiB Unicode font plus ~6 MiB of translations, which
# alone push the ISO past MAX_ISO_SIZE. A single English menuentry uses none of
# them, so ask for none: the empty lists are what disable each set.
GRUBFLAGS      = --fonts= --locales= --themes=
QEMU          ?= qemu-system-i386
# rustup installs into $HOME but only puts itself on PATH from a shell rc file,
# which a login shell or a bare `make` may never have read. Fall back to the
# known install path rather than fail with "cargo: command not found".
CARGO         ?= $(shell command -v cargo || echo $(or $(CARGO_HOME),$(HOME)/.cargo)/bin/cargo)
NM            ?= nm

KERNEL_LIB    := kernel/target/i686-kfs/release/libkernel.a
KERNEL_BIN    := $(BUILD)/kernel.bin
MAX_ISO_SIZE  := 10485760

.PHONY: all cargo run check clean fclean re docker-%

all: $(NAME)

$(BUILD):
	mkdir -p $(BUILD)

$(BUILD)/boot.o: boot/boot.asm | $(BUILD)
	$(NASM) $(NASMFLAGS) $< -o $@

# Phony: cargo does its own change detection.
cargo:
	cd kernel && RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build --release

$(KERNEL_BIN): $(BUILD)/boot.o linker.ld cargo
	$(LD) $(LDFLAGS) -o $@ $(BUILD)/boot.o $(KERNEL_LIB)
	$(GRUB_FILE) --is-x86-multiboot $@
# -nostdlib is a promise about the *result*, so check the result: an undefined
# symbol here is a host library the kernel expects and GRUB cannot provide.
	@if $(NM) -u $@ | grep -q .; then \
		echo "FAIL: $@ needs host libraries:"; $(NM) -u $@; exit 1; \
	fi
	@echo "OK: $@ is freestanding (no undefined symbols)"

$(NAME): $(KERNEL_BIN) grub.cfg
	mkdir -p $(ISODIR)/boot/grub
	cp $(KERNEL_BIN) $(ISODIR)/boot/kernel.bin
	cp grub.cfg $(ISODIR)/boot/grub/grub.cfg
	$(GRUB_MKRESCUE) $(GRUBFLAGS) -o $@ $(ISODIR)
	@size=$$(stat -c %s $@ 2>/dev/null || stat -f %z $@); \
	test $$size -le $(MAX_ISO_SIZE) \
		|| { echo "FAIL: $@ is $$size bytes, over $(MAX_ISO_SIZE)"; exit 1; }; \
	echo "OK: $@ is $$size bytes (limit $(MAX_ISO_SIZE))"

run: $(NAME)
	$(QEMU) -cdrom $(NAME)

# Headless end-to-end proof, asked from outside via the qemu monitor.
# `tools/check.sh` holds it: the assertions read the guest's segment
# registers, its physical memory at 0x800, and the text in its VGA
# buffer, then drive the shell with injected keystrokes. See the script
# for what each one proves.
check: $(NAME)
	ISO=$(NAME) QEMU=$(QEMU) tools/check.sh

clean:
	rm -rf $(BUILD)
	cd kernel && $(CARGO) clean

fclean: clean
	rm -f $(NAME)

re: fclean all

# Local dev convenience on hosts without an x86 toolchain (e.g. macOS/arm64).
# Runs the exact same native rules inside a Debian amd64 container.
# The image itself is amd64 (pinned in the Dockerfile), so `docker run` needs
# no --platform: passing it trips the containerd image store on arm64 hosts.
docker-%:
	docker build --platform=linux/amd64 -t kfs-builder .
	docker run --rm -v $(PWD):/kfs kfs-builder make $*
