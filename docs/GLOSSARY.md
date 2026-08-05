# Glossary

Every technical term used by this project, defined once. Grouped by subject
rather than alphabetically, because the terms only make sense in clusters —
use your reader's search to jump to a word.

"**Here:**" marks where the concept shows up in this repository.

---

## Operating systems and kernels

**Kernel** — the part of an operating system that runs with full hardware
privileges. It owns the CPU, memory and devices, and everything else runs on
top of it. In this project the kernel is the *only* software running: there is
no OS around it. Here: `kernel/src/lib.rs` plus `boot/boot.asm`.

**Operating system** — a kernel plus the surrounding user-space programs
(shell, libraries, services). kfs-1 is a kernel, not an OS.

**Bare metal / freestanding** — code that runs with no operating system
underneath it. No files, no processes, no `malloc`, no `printf`; the only
things that exist are the CPU, RAM and hardware you program yourself. Here:
the target spec's `"os": "none"`.

**Kernel space / user space** — the privileged half of a system versus the
sandboxed half where applications run. kfs-1 is entirely kernel space.

**Ring 0** — x86's most privileged execution level, where a kernel runs
(applications typically use ring 3). Our code runs in ring 0 from the first
instruction because GRUB left it there.

**System call** — the controlled doorway a user-space program uses to ask the
kernel for something. Not implemented; there is no user space to call in.

**Driver** — kernel code that speaks a specific device's protocol. The planned
VGA text output ([VGA.md](VGA.md)) will be this kernel's first driver.

**Panic** — an unrecoverable programming error. With no OS to report to, a
kernel panic can only stop the machine. Here: `#[panic_handler]` halts.

---

## The x86 processor

**x86** — the instruction set architecture Intel introduced with the 8086 and
which PCs still use. Backwards-compatible to a fault, which is why booting
involves several CPU modes.

**i386 / i686** — generation names inside the 32-bit x86 family. i386 is the
first 32-bit member; i686 is the Pentium-Pro-era baseline, and the name of our
build target. The subject requires a 32-bit kernel.

**Register** — a storage slot inside the CPU, the fastest memory that exists.
32-bit x86 has eight general-purpose ones (`EAX`, `EBX`, `ECX`, `EDX`, `ESI`,
`EDI`, `EBP`, `ESP`) plus special ones.

**EIP** — the instruction pointer: the address of the instruction being
executed. Reading it tells you exactly where a machine is. Here: `make check`
passes only when `EIP` is inside the kernel.

**ESP** — the stack pointer register, holding the address of the top of the
stack. Here: `_start`'s first instruction loads it.

**EFLAGS** — the status register: comparison results, the interrupt-enable
flag, and similar CPU state bits.

**Real mode** — the 16-bit mode the CPU starts in, with 1 MiB of addressable
memory and no memory protection, for compatibility with 1978. The BIOS and
GRUB's first stage run here.

**Protected mode** — 32-bit x86 mode with memory protection and privilege
levels: what a modern 32-bit kernel runs in. GRUB switches to it before jumping
to our entry point, so we never write mode-switching code.

**Long mode** — 64-bit x86 mode. Out of scope; the subject mandates 32-bit.

**Segment / GDT** — x86 divides memory into segments described by the Global
Descriptor Table. GRUB installs a flat, permissive GDT before handing over, so
the kernel works without touching it; replacing it with our own is a later
KFS step.

**Interrupt** — a hardware or software event that makes the CPU stop and jump
to a handler: a key press, a timer tick, a division by zero. Interrupts arrive
disabled at boot and stay disabled here, because we have installed no handlers.

**IDT** — the Interrupt Descriptor Table, mapping interrupt numbers to handler
addresses. Not implemented yet.

**Exception** — a CPU-generated interrupt signalling a fault: invalid opcode,
page fault, division by zero.

**Triple fault** — a fault occurs, faults while dispatching, and faults again;
the CPU gives up and resets. With no IDT this is what *any* CPU exception does
to us, and it looks like the machine rebooting in a loop. Here: `make check`
rules it out by proving the guest is still in `kmain` after five seconds.

**Paging / MMU** — the hardware translation from virtual to physical addresses,
and the unit that performs it. Not enabled; every address in this kernel is a
physical address.

**Red zone** — an ABI optimisation where a function may use 128 bytes below the
stack pointer without reserving them. Illegal in a kernel: an interrupt pushes
data there and silently corrupts it. Here: `"disable-redzone": true`.

**FPU / MMX / SSE** — floating-point and vector units on x86. They require
explicit enabling after boot, so a kernel that has not enabled them must not
contain their instructions. Here: `"features": "-mmx,-sse,+soft-float"`.

**Soft float** — performing floating-point arithmetic in software with integer
instructions instead of hardware float instructions. Slow but always available.

**MMIO (memory-mapped I/O)** — hardware exposed as memory addresses: writing to
an address changes device state rather than storing a value. The VGA text screen
at `0xb8000` is MMIO, which is why writes to it must be `volatile`.

**Port I/O** — the other x86 device mechanism, using `in`/`out` instructions on
a separate address space. Needed later for the hardware cursor (ports `0x3D4`/
`0x3D5`).

**VGA text mode** — the 80×25 character display the PC starts in. Each cell is
two bytes at `0xb8000`: a character code and a colour attribute. Here: the
subject of [VGA.md](VGA.md), not yet implemented.

**Framebuffer** — the memory region a display reads pixels or characters from.

**`hlt`** — the instruction that halts the CPU until the next interrupt. The
correct way for a kernel to idle: it stops burning power. Here: the hang loop
in `_start`.

**`cli`** — clear interrupt flag: disables maskable interrupts.

**`pause`** — a hint that the current loop is a spin-wait, letting the CPU
save power and avoid a pipeline penalty. Here: what `core::hint::spin_loop()`
compiles to inside `kmain` (bytes `f3 90`).

**Little-endian** — x86 stores multi-byte values least-significant byte first,
which is why the multiboot magic `0x1BADB002` appears in the binary as
`02 b0 ad 1b`.

**Stack** — the region of memory holding return addresses and local variables,
managed through `ESP`. On x86 it grows *downwards*, so the initial pointer is
the highest address of the reserved region. Here: 16 KiB in `.bss`, with `esp`
starting at `stack_top` = `0x104040`.

---

## Booting

**Firmware** — software stored in the machine itself, running before any disk
is read.

**BIOS** — the traditional PC firmware. It initialises hardware, runs POST,
then loads the first sector of the chosen boot device and jumps into it. QEMU
provides one; we boot in BIOS mode, not UEFI.

**POST** — power-on self test, the firmware's hardware check at startup.

**UEFI** — the modern replacement for the BIOS, with a different boot protocol.
`grub-mkrescue` happens to produce an image that works with both; our path is
BIOS.

**Boot sector / MBR** — the first 512-byte sector of a bootable device, holding
the first-stage bootloader. Here: `grub-mkrescue` embeds
`boot_hybrid.img` in the ISO's system area so the image also boots as if it
were a disk.

**Bootloader** — the program that loads a kernel into memory and starts it.
Writing one from scratch means talking to disks in real mode; using GRUB is
what the subject asks for.

**GRUB (GRand Unified Bootloader)** — the standard Linux bootloader, and the
one this project uses. It reads a config file, loads a kernel image, prepares
the CPU (protected mode, flat GDT) and jumps to the kernel's entry point.
Here: `grub.cfg`, and `grub-mkrescue` to build the ISO.

**GRUB module** — a loadable piece of GRUB itself (filesystem drivers, video
drivers). `grub-mkrescue` copies its whole module tree onto the ISO — 294 files
in this build — which is why a 1 KB kernel yields a 5 MB image.

**`grub.cfg`** — GRUB's configuration file, read at boot from
`/boot/grub/grub.cfg` inside the image.

**`menuentry`** — one bootable choice in `grub.cfg`. Ours contains
`multiboot /boot/kernel.bin` and `boot`.

**Multiboot specification** — an agreement between bootloaders and kernels: if
your image begins with a recognisable header, any compliant bootloader can load
it, and it will hand you a machine already in protected mode plus a description
of memory. It saves every hobby kernel from writing a bootloader. We use
multiboot **1**; multiboot 2 is a different header format and a different GRUB
command.

**Multiboot header** — the 12 bytes that identify the image: magic, flags,
checksum. Must appear within the first 8 KiB of the file, 4-byte aligned.
Here: `section .multiboot` in `boot/boot.asm`, forced first by `linker.ld`.

**Magic number** — a fixed value used as a signature. Multiboot 1's is
`0x1BADB002`.

**Checksum** — a value chosen so a sum comes out to a known result; here
`MAGIC + FLAGS + CHECKSUM` must equal 0 in 32-bit arithmetic, giving
`0xE4524FFB`. A wrong checksum makes GRUB refuse the file.

**Entry point** — the address the first instruction lives at. Recorded in the
ELF header and set by `ENTRY(_start)` in the linker script; `0x100010` here.

**Load address** — where an image's contents are placed in physical memory.
Ours is 1 MiB (`0x100000`), the conventional lowest address a kernel may claim,
since below it live the BIOS data area, the interrupt vector table, the VGA
framebuffer and mapped ROM.

**ISO 9660** — the CD-ROM filesystem format. `kfs.iso` is one.

**El Torito** — the extension to ISO 9660 that makes a CD bootable.

**`grub-mkrescue`** — GRUB's tool for producing a bootable ISO from a directory
tree: it adds GRUB itself, its modules and the boot records. Fedora names the
binary `grub2-mkrescue`, Debian `grub-mkrescue`; the Makefile detects both.

**`grub-file`** — GRUB's inspection tool. `grub-file --is-x86-multiboot`
succeeds only if a valid multiboot header is present; the build runs it as an
assertion.

**`xorriso` / `mtools`** — the ISO-writing and FAT-manipulation utilities
`grub-mkrescue` calls under the hood.

---

## Assembly

**Assembly language** — a textual, one-to-one representation of machine
instructions. Needed wherever no higher-level language can express the
operation: here, setting `ESP` before any function call can happen.

**Assembler** — the program translating assembly text into an object file.

**NASM (Netwide Assembler)** — the assembler this project uses, in Intel
syntax (`mov dst, src`). Invoked as `nasm -f elf32` to produce a 32-bit ELF
object.

**Mnemonic** — the human-readable name of an instruction (`mov`, `call`,
`jmp`).

**Opcode / machine code** — the actual bytes the CPU executes.
`mov esp, 0x104040` is `bc 40 40 10 00`.

**Label** — a name for an address (`_start:`, `stack_top:`). Labels beginning
with `.` in NASM are local to the previous global label, hence `_start.hang`.

**Directive** — an instruction to the assembler rather than the CPU:
- `equ` — define a constant.
- `dd` — emit a 32-bit value ("define doubleword"). Used for the three header
  words.
- `resb` — reserve bytes without storing them in the file. Used for the stack.
- `align` — pad until the address is a multiple of N.
- `section` — start emitting into a named section.
- `global` — export a symbol so the linker can see it (`_start`).
- `extern` — declare a symbol defined elsewhere (`kmain`).

**Calling convention** — the rules for how a call passes arguments, returns
values and divides register-preservation duties. Assembly and Rust must agree,
which is what `extern "C"` selects.

**ABI (application binary interface)** — the wider contract: calling
convention, type sizes, alignment, symbol naming. Here: `"rustc-abi":
"softfloat"` states that floats are not passed in SSE registers.

---

## Compiling and linking

**Compiler** — translates a high-level language into machine code. Here:
`rustc`, driven by `cargo`.

**Cross compilation** — building code for a machine other than the one doing
the build. Every kernel build is a cross compilation: the host runs Linux or
macOS, the target runs nothing.

**Host vs target** — the machine compiling versus the machine that will run the
output. Here: host `x86_64-unknown-linux-gnu` inside the container, target
`i686-kfs`.

**Target triple** — the conventional name for a target, roughly
`arch-vendor-os-environment`, e.g. `i686-unknown-linux-gnu`. Ours would be
`i686-unknown-none`.

**Target spec JSON** — Rust's way to describe a target that isn't built in, as
a JSON file of properties. Here: `kernel/i686-kfs.json`, enabled by the
unstable `json-target-spec` flag.

**LLVM** — the compiler backend `rustc` uses to turn its IR into machine code.
The target spec's `llvm-target` and `data-layout` are handed straight to it.

**Data layout** — LLVM's description of pointer sizes, integer and float
alignments for a target. It must match the LLVM target or code generation is
subtly wrong.

**Object file** — the output of assembling or compiling one translation unit:
machine code plus sections, symbols and relocations, not yet arranged in
memory. Here: `build/boot.o`.

**ELF (Executable and Linkable Format)** — the Unix binary format used for
object files and executables. It stores sections (for linking) and segments
(for loading). GRUB parses ELF to know where to put our code.

**Section** — a named region of a binary grouping similar content:
- `.text` — executable code.
- `.rodata` — read-only data (constants, string literals).
- `.data` — initialised writable data.
- `.bss` — zero-initialised writable data, occupying no file space.
- `COMMON` — legacy uninitialised symbols, folded into `.bss` here.
- `.comment` — toolchain version strings; not loaded.
- `.note.GNU-stack` — a marker declaring whether the stack must be executable.
- `.multiboot` — *our* custom section name, so the linker script can place the
  header first.

**NOBITS** — the ELF flag meaning "occupies memory but has no bytes in the
file". `.bss` is NOBITS: 16 KiB of RAM, 0 bytes on disk.

**Symbol** — a name attached to an address (`kmain`, `stack_top`).

**Symbol table** — the list of a binary's symbols; read with `nm`.

**Relocation** — a placeholder in an object file saying "patch this address
once the final layout is known". `call kmain` in `boot.o` is a relocation that
`ld` resolves to `0x100030`.

**Linker** — combines object files and libraries into one image, resolving
symbols and assigning final addresses. Here: GNU `ld`, called with
`-m elf_i386 -n -T linker.ld`.

**Linker script** — the file telling the linker what to place where. Writing
one is mandatory for a kernel: default scripts target userland programs on a
specific OS. Here: `linker.ld`.

**Location counter (`.`)** — the linker script variable holding the address
currently being assigned. `. = 1M;` moves it to `0x100000`.

**VMA / LMA** — virtual and load address of a section. Identical here, because
paging is off.

**Segment / program header** — the loader's view of an ELF: which byte ranges
to copy to which addresses with which permissions. Our kernel has one `LOAD`
segment, 52 bytes on disk expanding to 16 448 in RAM.

**Static library / archive (`.a`)** — a bundle of object files. `cargo` emits
`libkernel.a`; `ld` pulls out the objects it needs. Created and listed with
`ar`.

**Name mangling** — the compiler encoding module paths and types into symbol
names, e.g. this kernel's panic handler appears as
`_RNvCsj5li9sZI3iI_7___rustc17rust_begin_unwind`. `#[no_mangle]` switches it
off so assembly can refer to `kmain` by that exact name.

**`nm` / `objdump` / `readelf`** — the inspection tools: list symbols,
disassemble and dump sections, print ELF headers. Every address quoted in
[ARCHITECTURE.md](ARCHITECTURE.md) came from them.

---

## Rust specifics

**Rust** — the systems language this kernel is written in: no runtime, no
garbage collector, compile-time memory safety, and `unsafe` blocks for the
places where hardware access cannot be proven safe.

**Cargo** — Rust's build tool and package manager. Reads `Cargo.toml`.

**Crate** — a Rust compilation unit / package. This repo has one, `kernel`.

**Crate type** — what a crate compiles to. `staticlib` produces a C-compatible
`.a` archive, which is what lets our own `ld` command do the final link.
(`rlib` would be Rust-only; `bin` would make cargo link an executable with the
wrong script.)

**`Cargo.toml` / `Cargo.lock`** — the manifest and the resolved dependency
versions. The lock file is committed so a rebuild is reproducible.

**Edition** — the Rust language edition (2021 here); a compatibility marker,
not a compiler version.

**Profile** — a named set of build settings. `[profile.release]` uses
`opt-level = 2` and `panic = "abort"`.

**`no_std`** — the attribute that unlinks the standard library. Necessary
because `std` assumes an OS: threads, files, a heap, syscalls.

**`core`** — the OS-independent subset of Rust's standard library: primitive
types, `Option`, `Result`, slices, atomics, `PanicInfo`. Always available,
including in a kernel.

**`alloc`** — the layer between `core` and `std` providing `Box`, `Vec` and
`String`. Requires an allocator, so it is unavailable until this kernel has a
heap.

**Panic handler** — the `#[panic_handler]` function `no_std` crates must
provide, called when a panic occurs. Ours halts forever; there is nowhere to
report to.

**Unwinding vs abort** — on panic, Rust normally unwinds the stack running
destructors, which needs runtime support that does not exist here.
`panic = "abort"` and `"panic-strategy": "abort"` remove it.

**`#[no_mangle]`** — keep a symbol's name exactly as written.

**`extern "C"`** — use the C calling convention for a function, so non-Rust
code can call it correctly.

**Never type (`-> !`)** — the return type of a function that never returns.
`kmain() -> !` is why an infinite loop is required and why the code after
`call kmain` is unreachable.

**Nightly** — Rust's unstable release channel. Required here because building
`core` for a custom target and using JSON target files are both unstable
features. Pinned in `rust-toolchain.toml`.

**`rust-src`** — the rustup component shipping the standard library's source,
without which `core` cannot be rebuilt for our target.

**`build-std`** — the unstable cargo feature that compiles `core` (and
`compiler_builtins`) from source for a target that has no prebuilt copy.

**`compiler_builtins`** — Rust's implementation of the low-level helper
routines LLVM assumes exist: 64-bit division on a 32-bit CPU, and with the
`compiler-builtins-mem` feature, `memcpy`/`memset`/`memcmp`. On a normal system
libc supplies those; we have no libc, so we opt in.

**`volatile`** — a memory access the compiler may not optimise away, reorder or
merge. Mandatory for MMIO such as the VGA buffer, where the *act* of writing is
the point. Here: specified for the planned `vga.rs`.

**`unsafe`** — the keyword marking operations the compiler cannot verify, such
as dereferencing a raw pointer to `0xb8000`. It does not disable checks; it
moves the burden of proof to the author.

**`core::hint::spin_loop()`** — a portable hint that the current loop is
spin-waiting; on x86 it emits `pause`. Here: the body of `kmain`.

---

## Build and test tooling

**Make** — the build automation tool that runs commands when files are out of
date. Terms: a **target** is a thing to build, its **prerequisites** are what
it depends on, and its **recipe** is the tab-indented commands.

**Phony target** — a Make target that names an action, not a file (`all`,
`clean`, `cargo`). Declared in `.PHONY` so Make never mistakes a same-named
file for it.

**Pattern rule** — a Make rule with a `%` wildcard. Here `docker-%:` matches
`docker-all`, `docker-check`, and passes `$*` (the part matched by `%`) to
`make` inside the container.

**QEMU** — the machine emulator used to run the kernel. `qemu-system-i386`
emulates a 32-bit PC: BIOS, CD-ROM, VGA, the works. Emulation (rather than
virtualisation) means it can run x86 code on an arm64 Mac, slowly but
faithfully.

**`-cdrom kfs.iso`** — attach the ISO as the boot CD.

**`-display none`** — run with no window: headless, for scripted checks.

**QEMU monitor** — QEMU's own control console, independent of the guest. It can
report CPU state, dump the screen, pause and reset the machine. Exposed here on
a unix socket with `-monitor unix:/tmp/kfs-mon,server,nowait`.

**`info registers`** — the monitor command printing the guest's CPU registers.
The `EIP` it reports is the proof that the kernel is executing.

**`screendump file.ppm`** — the monitor command writing the current guest
screen to an image. Used to confirm that the only thing on screen is GRUB's
leftover text plus a blinking cursor.

**Headless** — running with no display attached.

**`socat`** — a utility that pipes data between arbitrary endpoints. Here it
connects stdin/stdout to the monitor's unix socket, which is how `make check`
talks to a running QEMU without a terminal.

**Unix domain socket** — an inter-process communication channel that looks like
a filesystem path. `/tmp/kfs-mon` is one.

**Docker** — container tooling used only for local development on a machine
with no x86 toolchain. An **image** is a filesystem template built from the
`Dockerfile`; a **container** is a running instance; a **bind mount**
(`-v $(PWD):/kfs`) exposes the host's repo inside it so build artifacts land in
the working tree. `--platform=linux/amd64` requests the x86 image on an arm64
host, which Docker runs under emulation. The evaluation machine uses none of
this.
