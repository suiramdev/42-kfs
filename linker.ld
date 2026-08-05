ENTRY(_start)

SECTIONS
{
    /* GRUB loads the kernel at 1 MiB; below that is BIOS/legacy territory. */
    . = 1M;

    .multiboot : { *(.multiboot) }
    .text      : { *(.text*) }
    .rodata    : { *(.rodata*) }
    .data      : { *(.data*) }
    .bss       : { *(COMMON) *(.bss*) }
}
