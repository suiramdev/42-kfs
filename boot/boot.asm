MBALIGN  equ 1 << 0
MEMINFO  equ 1 << 1
FLAGS    equ MBALIGN | MEMINFO
MAGIC    equ 0x1BADB002
CHECKSUM equ -(MAGIC + FLAGS)

KERNEL_SPACE  equ 0xC0000000
KERNEL_DIR    equ KERNEL_SPACE >> 22
PAGE_PRESENT  equ 1 << 0
PAGE_WRITE    equ 1 << 1
PAGE_ENABLE   equ 1 << 31
ENTRIES       equ 1024
PAGE_SIZE     equ 4096

section .multiboot
align 4
    dd MAGIC
    dd FLAGS
    dd CHECKSUM

section .bss
alignb 4096
global boot_directory
boot_directory:
    resb 4096
global boot_low_table
boot_low_table:
    resb 4096
alignb 16
global stack_bottom
stack_bottom:
    resb 16384
global stack_top
stack_top:

section .boot progbits alloc exec nowrite align=16
global _start
_start:
    mov esi, eax
    mov edi, ebx

    mov edx, boot_low_table - KERNEL_SPACE
    mov eax, PAGE_PRESENT | PAGE_WRITE
    mov ecx, ENTRIES
.identity:
    mov [edx], eax
    add eax, PAGE_SIZE
    add edx, 4
    loop .identity

    mov edx, boot_directory - KERNEL_SPACE
    mov eax, (boot_low_table - KERNEL_SPACE) + PAGE_PRESENT + PAGE_WRITE
    mov [edx], eax
    mov [edx + KERNEL_DIR * 4], eax
    mov eax, (boot_directory - KERNEL_SPACE) + PAGE_PRESENT + PAGE_WRITE
    mov [edx + (ENTRIES - 1) * 4], eax

    mov cr3, edx

    mov eax, cr0
    or eax, PAGE_ENABLE
    mov cr0, eax

    mov eax, higher_half
    jmp eax

section .text
extern kmain
higher_half:
    mov esp, stack_top
    mov ebp, esp
    push edi
    push esi
    call kmain
.hang:
    cli
    hlt
    jmp .hang

section .note.GNU-stack noalloc noexec nowrite progbits
