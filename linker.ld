ENTRY(_start)

SECTIONS
{
    . = 1M;

    .multiboot : { KEEP(*(.multiboot)) }
    .text      : { *(.text*) }
    .rodata    : { *(.rodata*) }
    .data      : { *(.data*) }
    .got       : { *(.got*) }
    .bss       : { *(COMMON) *(.bss*) }

    /DISCARD/  : { *(.eh_frame*) *(.comment) }
}
