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
QEMU          ?= qemu-system-i386
CARGO         ?= cargo

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
	$(GRUB_MKRESCUE) -o $@ $(ISODIR)
	@size=$$(stat -c %s $@ 2>/dev/null || stat -f %z $@); \
	test $$size -le $(MAX_ISO_SIZE) \
		|| { echo "FAIL: $@ is $$size bytes, over $(MAX_ISO_SIZE)"; exit 1; }; \
	echo "OK: $@ is $$size bytes (limit $(MAX_ISO_SIZE))"

run: $(NAME)
	$(QEMU) -cdrom $(NAME)

# Headless boot proof: the guest must still be alive after 5 s with EIP inside
# the kernel (>= 1 MiB), i.e. GRUB loaded the multiboot binary and kmain spins
# instead of the machine triple-faulting and rebooting.
check: $(NAME)
	@rm -f /tmp/kfs-mon $(BUILD)/regs.txt
	@$(QEMU) -cdrom $(NAME) -display none \
		-monitor unix:/tmp/kfs-mon,server,nowait & echo $$! > $(BUILD)/qemu.pid
	@sleep 5
	@printf 'info registers\nquit\n' \
		| socat - unix-connect:/tmp/kfs-mon > $(BUILD)/regs.txt \
		|| { echo "FAIL: qemu monitor unreachable (guest died)"; \
		     kill $$(cat $(BUILD)/qemu.pid) 2>/dev/null; exit 1; }
	@kill $$(cat $(BUILD)/qemu.pid) 2>/dev/null || true
	@eip=$$(sed -n 's/.*EIP=\([0-9a-fA-F]*\).*/\1/p' $(BUILD)/regs.txt | head -1); \
	test -n "$$eip" || { echo "FAIL: no EIP in monitor output"; exit 1; }; \
	test $$((0x$$eip)) -ge $$((0x100000)) \
		|| { echo "FAIL: EIP=$$eip is below 1 MiB, kernel not running"; exit 1; }; \
	echo "OK: guest alive, EIP=$$eip inside kernel"

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
