; Multiboot 1 entry point for kfs-1.
; GRUB2 loads this via the `multiboot` command in grub.cfg.

MBALIGN  equ 1 << 0             ; align loaded modules on page boundaries
MEMINFO  equ 1 << 1             ; provide memory map
FLAGS    equ MBALIGN | MEMINFO
MAGIC    equ 0x1BADB002
CHECKSUM equ -(MAGIC + FLAGS)

section .multiboot
align 4
    dd MAGIC
    dd FLAGS
    dd CHECKSUM

; The kernel stack. Exported so `kernel::stack` can print it: the dump
; needs the real bounds, and only the linker knows where .bss landed.
section .bss
align 16
global stack_bottom
stack_bottom:
    resb 16384
global stack_top
stack_top:

section .text
global _start
extern kmain
_start:
    mov esp, stack_top
    call kmain
.hang:
    cli
    hlt
    jmp .hang

; Mark the stack non-executable so `ld` does not warn about an implicitly
; executable stack.
section .note.GNU-stack noalloc noexec nowrite progbits
