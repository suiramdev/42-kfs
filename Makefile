NAME          := kfs.iso
BUILD         := build
ISODIR        := $(BUILD)/iso

NASM          ?= nasm
NASMFLAGS      = -f elf32
LD            ?= ld
LDFLAGS        = -m elf_i386 -n -nostdlib --gc-sections -T linker.ld
RUSTFLAGS     ?= -C panic=abort -C no-redzone=y -Z stack-protector=none
GRUB_MKRESCUE ?= $(shell command -v grub-mkrescue || command -v grub2-mkrescue)
GRUB_FILE     ?= $(shell command -v grub-file || command -v grub2-file)
GRUBFLAGS      = --fonts= --locales= --themes=
QEMU          ?= qemu-system-i386
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

cargo:
	cd kernel && RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build --release

$(KERNEL_BIN): $(BUILD)/boot.o linker.ld cargo
	$(LD) $(LDFLAGS) -o $@ $(BUILD)/boot.o $(KERNEL_LIB)
	$(GRUB_FILE) --is-x86-multiboot $@
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

check: $(NAME)
	ISO=$(NAME) QEMU=$(QEMU) tools/check.sh

clean:
	rm -rf $(BUILD)
	cd kernel && $(CARGO) clean

fclean: clean
	rm -f $(NAME)

re: fclean all

docker-%:
	docker build --platform=linux/amd64 -t kfs-builder .
	docker run --rm -v $(PWD):/kfs kfs-builder make $*
