ENTRY(_start)

SECTIONS
{
    /* GRUB loads the kernel at 1 MiB; below that is BIOS/legacy territory. */
    . = 1M;

    /* KEEP, because nothing in the program refers to the multiboot header:
       GRUB finds it by scanning the file. Without KEEP, --gc-sections sees an
       unreferenced section and drops it, and the ISO stops booting. */
    .multiboot : { KEEP(*(.multiboot)) }
    .text      : { *(.text*) }
    .rodata    : { *(.rodata*) }
    .data      : { *(.data*) }
    .got       : { *(.got*) }
    .bss       : { *(COMMON) *(.bss*) }

    /* Unwind tables describe how to walk back out of a panic. This kernel
       aborts instead, so nothing ever reads them. */
    /DISCARD/  : { *(.eh_frame*) *(.comment) }
}
