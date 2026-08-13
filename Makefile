NAME          := kfs.iso
BUILD         := build
ISODIR        := $(BUILD)/iso

NASM          ?= nasm
NASMFLAGS      = -f elf32
LD            ?= ld
LDFLAGS        = -m elf_i386 -n -T linker.ld
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
	cd kernel && $(CARGO) build --release

$(KERNEL_BIN): $(BUILD)/boot.o linker.ld cargo
	$(LD) $(LDFLAGS) -o $@ $(BUILD)/boot.o $(KERNEL_LIB)
	$(GRUB_FILE) --is-x86-multiboot $@

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

# Headless boot proof, asked from outside via the qemu monitor:
#   1. the guest must reach EIP inside the kernel — within [1 MiB, 2 MiB):
#      GRUB loaded the multiboot binary and kmain runs instead of the
#      machine triple-faulting and rebooting. Both bounds matter: GRUB
#      itself executes below 1 MiB *and* relocated near the top of RAM
#      (EIP=0x07f7d106 was observed mid-boot), so ">= 1 MiB" alone can
#      pass while GRUB is still running. Polled rather than a fixed
#      sleep: boot takes ~4 s natively but can take >5 s inside an
#      emulated container, so we retry every 2 s for up to 60 s.
#   2. a frame dump must contain white pixels (the "42" glyphs, attribute
#      0x0F) and none of GRUB's grey #a8a8a8 leftovers (vga::clear ran).
# The colour greps scan the raw P6 byte stream, so they rely on the kernel
# only drawing colours that render without 0xa8 or spurious 0xff bytes:
# black, white and the bright half of the palette. The dim half (#00a800
# green, #a80000 red, ...) could reassemble GRUB's grey across adjacent
# pixel boundaries — keep it off the boot screen.
check: $(NAME)
	@rm -f /tmp/kfs-mon $(BUILD)/screen.ppm
	@$(QEMU) -cdrom $(NAME) -display none \
		-monitor unix:/tmp/kfs-mon,server,nowait & echo $$! > $(BUILD)/qemu.pid
	@eip=; for i in $$(seq 1 30); do \
		sleep 2; \
		eip=$$(printf 'info registers\n' \
			| socat - unix-connect:/tmp/kfs-mon 2>/dev/null \
			| sed -n 's/.*EIP=\([0-9a-fA-F]*\).*/\1/p' | head -1); \
		test -n "$$eip" && test $$((0x$$eip)) -ge $$((0x100000)) \
			&& test $$((0x$$eip)) -lt $$((0x200000)) && break; \
	done; \
	test -n "$$eip" \
		|| { echo "FAIL: qemu monitor unreachable (guest died)"; \
		     kill $$(cat $(BUILD)/qemu.pid) 2>/dev/null; exit 1; }; \
	{ test $$((0x$$eip)) -ge $$((0x100000)) && test $$((0x$$eip)) -lt $$((0x200000)); } \
		|| { echo "FAIL: EIP=$$eip still outside the kernel after 60 s"; \
		     kill $$(cat $(BUILD)/qemu.pid) 2>/dev/null; exit 1; }; \
	echo "OK: guest alive, EIP=$$eip inside kernel"
	@state=none; for i in $$(seq 1 10); do \
		printf 'screendump $(BUILD)/screen.ppm\n' \
			| socat - unix-connect:/tmp/kfs-mon > /dev/null 2>&1; \
		pixels=$$(od -An -v -tx1 $(BUILD)/screen.ppm 2>/dev/null | tr -d ' \n'); \
		case $$pixels in \
		*a8a8a8*) state=grey;; \
		*ffffff*) state=ok; break;; \
		'') state=none;; \
		*) state=black;; \
		esac; \
		sleep 1; \
	done; \
	printf 'quit\n' | socat - unix-connect:/tmp/kfs-mon > /dev/null 2>&1; \
	kill $$(cat $(BUILD)/qemu.pid) 2>/dev/null || true; \
	case $$state in \
	ok) echo "OK: screen cleared and \"42\" glyphs lit";; \
	grey) echo "FAIL: GRUB's grey text still on screen, vga::clear did not run"; exit 1;; \
	black) echo "FAIL: no white pixels on screen, \"42\" not displayed"; exit 1;; \
	*) echo "FAIL: no screen dump produced"; exit 1;; \
	esac

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
