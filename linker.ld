ENTRY(_start)

KERNEL_SPACE = 0xC0000000;
KERNEL_LOAD  = 0x00100000;

SECTIONS
{
    . = KERNEL_LOAD;

    boot_start = .;

    .boot ALIGN(4K) :
    {
        KEEP(*(.multiboot))
        *(.boot)
    }

    boot_end = .;

    . += KERNEL_SPACE;
    . = ALIGN(4K);

    kernel_start = .;

    .text : AT(ADDR(.text) - KERNEL_SPACE)
    {
        *(.text*)
    }

    kernel_code_end = .;

    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_SPACE)
    {
        *(.rodata*)
    }

    kernel_readonly_end = ALIGN(4K);

    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_SPACE)
    {
        *(.data*)
        *(.got*)
    }

    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_SPACE)
    {
        *(COMMON)
        *(.bss*)
    }

    kernel_end = ALIGN(4K);

    /DISCARD/ : { *(.eh_frame*) *(.comment) }
}
