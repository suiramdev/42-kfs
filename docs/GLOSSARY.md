# Glossary

Every technical term used by this project, defined once. Grouped by subject
rather than alphabetically, because the terms only make sense in clusters —
use your reader's search to jump to a word.

One idea underpins all of them. Every program you have ever written ran *on top
of* an operating system. `printf`, `malloc`, `open`, `import`, `Promise` — all
of those are, sooner or later, requests to a kernel. This project *is* the thing
that would have answered them. There is nothing underneath it: no files, no
heap, no threads, no `println!`. Dereference a bad pointer and no one kills your
process; the machine reboots.

"**Here:**" marks where the concept shows up in this repository.

## Start here

If the whole vocabulary is new, read these eight in this order and the rest will
have somewhere to attach:

**Kernel**, **Bare metal / freestanding**, **Bootloader**, **Multiboot
specification**, **Entry point**, **Linker script**, **Triple fault**,
**`no_std`**.

---

## Operating systems and kernels

These words describe your own programs from the outside. You have always been
the guest: something else owned the CPU, handed you memory, and answered your
system calls. Here there is no host — this code is the host, and the words below
name the job it has inherited.

**Kernel** — the one program on a machine allowed to touch hardware directly;
everything else runs on top of it and asks it for favours. In kfs-1 there is no
"everything else": the kernel is the only software running. Here:
`kernel/src/lib.rs` plus `boot/boot.asm`.

**Operating system** — a kernel plus the programs around it that you actually
interact with: a shell, libraries, services. kfs-1 is a kernel, not an OS.

**Bare metal / freestanding** — code with no operating system underneath it.
No files, no processes, no `malloc`, no `printf`, no `println!` — only the CPU,
RAM, and whatever hardware you program yourself. Here: `"os": "none"` in
`kernel/i686-kfs.json`.

**Kernel space / user space** — the privileged half of a running system versus
the sandboxed half where applications live. kfs-1 is entirely kernel space;
there is no sandbox to be inside.

**Ring 0** — x86's name for its most privileged level, the one a kernel runs in;
ordinary applications get ring 3. Our code is in ring 0 from its very first
instruction, because GRUB left the CPU there.

**System call** — the single legal doorway a user-space program has for asking
the kernel to do something. Every `open`, `write` and `mmap` you have called was
one. Not implemented here: there is no user space to knock on the door.

**Driver** — kernel code that knows one specific device's private protocol.
The VGA text output ([VGA.md](VGA.md)) is this kernel's first driver.

**Panic** — a bug the program has decided it cannot survive. In userland
something catches it and kills your process; here nobody is listening, so a
panic can only stop the machine. Here: the `#[panic_handler]` in
`kernel/src/lib.rs` prints the panic in bright red, then spins forever.

---

## The x86 processor

You have never had to care what mode the CPU is in, how many registers it has,
or whether floating point is switched on, because something arranged all of that
long before `main` ran. Here nobody did. Beyond the little GRUB sets up, the
CPU's state is our problem, and these words name the pieces of it.

**x86** — the family of instruction sets Intel began with the 8086 and PCs still
use. Backwards-compatible to a fault, which is the entire reason booting walks
through several CPU modes.

**i386 / i686** — generation names inside the 32-bit x86 family. i386 is the
first 32-bit member; i686 is the Pentium-Pro-era baseline, and the name inside
our build target. The subject requires a 32-bit kernel.

**Register** — a storage slot inside the CPU itself, and the fastest memory that
exists. Registers are the CPU's local variables: 32-bit x86 has eight
general-purpose ones (`EAX`, `EBX`, `ECX`, `EDX`, `ESI`, `EDI`, `EBP`, `ESP`)
plus a few special ones.

**EIP** — the register holding the address of the instruction being executed. It
answers a debugger's most basic question, "which line am I on". Here: `make
check` passes only when `EIP` is inside the kernel.

**ESP** — the register holding the address of the top of the stack (see
**Stack**). Here: `_start`'s first instruction loads it with `stack_top`.

**EFLAGS** — the register holding one-bit facts about the CPU: was the last
comparison equal, are interrupts enabled, did the last addition overflow.

**Real mode** — the 16-bit mode the CPU wakes up in: 1 MiB of addressable
memory, no memory protection, 1978 rules. The BIOS and GRUB's first stage run
here.

**Protected mode** — the 32-bit mode with memory protection and privilege
levels, and what a modern 32-bit kernel expects to find itself in. GRUB switches
into it before jumping to us, which is why this repo contains no mode-switching
code.

**Long mode** — 64-bit x86 mode. Out of scope; the subject mandates 32-bit.

**Segment / GDT** — x86 can slice memory into segments, each described by an
entry in a table called the Global Descriptor Table. GRUB installs a flat,
permissive GDT before handing over, so the kernel works without touching it;
replacing it with our own is a later KFS step.

**Interrupt** — an event that makes the CPU drop what it is doing and jump to a
handler: a key press, a timer tick, a division by zero. It is a callback the
hardware invokes whether your code was ready or not. Interrupts arrive disabled
at boot and stay disabled here, because we have installed no handlers.

**IDT** — the Interrupt Descriptor Table, the array mapping interrupt numbers to
handler addresses. Not implemented yet, so there is nothing for an interrupt to
call.

**Exception** — an interrupt the CPU raises about itself: invalid opcode, page
fault, division by zero. Same idea as a hardware-level thrown error, except
there is no `catch` anywhere.

**Triple fault** — the CPU faults, faults again while handling that fault,
faults a third time, gives up and resets. With no IDT this is what *any* CPU
exception does to us, and from outside it looks like the machine rebooting in a
loop. It is the kernel-land equivalent of a segfault, except nothing catches it.
Here: `make check` rules it out by proving the guest settles in `kmain` and
stays there.

**Paging / MMU** — the hardware that translates the addresses your code uses
into real addresses in RAM, and so lets every process pretend it owns memory
from zero upwards. Not enabled here: every address in this kernel is a physical
address.

**Red zone** — an optimisation where a function scribbles in the 128 bytes just
below the stack pointer without formally reserving them. Fine in userland, fatal
in a kernel: an interrupt pushes its own data there and silently overwrites it.
Here: `"disable-redzone": true`.

**FPU / MMX / SSE** — the floating-point and vector units on an x86 chip. They
must be switched on explicitly after boot, so a kernel that has not switched
them on must not contain a single one of their instructions — banning them is
cheaper than initialising hardware the kernel does not use. Here: `"features":
"-mmx,-sse,+soft-float"`.

**Soft float** — doing floating-point arithmetic in software with ordinary
integer instructions instead of hardware float instructions. Slow, but it works
on a chip whose FPU nobody turned on.

**MMIO (memory-mapped I/O)** — a memory address that is really a device: writing
to it changes hardware instead of storing a value. The VGA text screen at
`0xb8000` is MMIO, which is why writes to it must be `volatile` — the compiler
must not optimise away a write whose whole point is the side effect.

**Port I/O** — x86's other way of reaching devices, using `in` and `out`
instructions on a separate, small address space of its own. Here: the hardware
cursor (ports `0x3D4`/`0x3D5`) and the PS/2 keyboard (status `0x64`, data
`0x60`).

**PS/2 / 8042** — the classic PC keyboard interface and the controller chip
behind it. QEMU emulates one regardless of your real keyboard. It hands out
scancodes, never characters.

**Scancode** — the number a keyboard sends for a physical key *position*:
press and release are separate codes (release = press | 0x80), and "which
character is that" is a layout decision the kernel makes with a lookup table.
Here: `kernel/src/keyboard.rs`, US QWERTY, set 1.

**Polling** — asking a device "anything new?" in a loop, as opposed to the
device raising an interrupt when something happens. Wasteful but simple, and
the only option before an IDT exists. Here: `kmain`'s keyboard loop.

**VGA text mode** — the 80×25 grid of characters a PC starts up in. Each cell is
two bytes at `0xb8000`: a character code and a colour attribute. Here: driven
by `kernel/src/vga.rs`, the subject of [VGA.md](VGA.md).

**Framebuffer** — the region of memory a display continuously reads its pixels
or characters out of. Change the memory and the screen changes.

**`hlt`** — the instruction that stops the CPU until the next interrupt arrives.
The correct way for a kernel to idle: a bare `while (true) {}` would burn a core
for nothing. Here: the hang loop after `call kmain` in `_start`.

**`cli`** — "clear interrupt flag": tells the CPU to stop accepting maskable
interrupts. Here: immediately before `hlt` in `_start`. Non-maskable events can
still wake the CPU, which is why the `jmp` after it halts again.

**`pause`** — an instruction meaning "this loop is only waiting", letting the
CPU save power and avoid a pipeline penalty. Here: what
`core::hint::spin_loop()` compiles to inside `kmain` (bytes `f3 90`).

**Little-endian** — x86 stores multi-byte values least-significant byte first,
which is why the multiboot magic `0x1BADB002` appears in the binary as
`02 b0 ad 1b`.

**Stack** — the memory holding return addresses and local variables, tracked
through `ESP`. Nobody hands a kernel one; it reserves memory and points `ESP` at
it. On x86 the stack grows *downwards*, so the initial pointer is the *highest*
address of the reserved region. Here: 16 KiB in `.bss`, with `esp` starting at
`stack_top` = `0x104690`.

---

## Booting

Between power-on and your `main` sits a chain of programs you have never had to
meet. This section names the links. Only two matter here: the firmware in the
machine, and a bootloader whose only job is to find a kernel, load it into
memory, and jump to it.

**Firmware** — software stored in the machine itself, running before any disk
has been read.

**BIOS** — the traditional PC firmware. It initialises hardware, runs POST, then
loads the first sector of the chosen boot device and jumps into it. QEMU provides
one; we boot in BIOS mode, not UEFI.

**POST** — power-on self test, the firmware's own hardware check at startup.

**UEFI** — the modern replacement for the BIOS, with a different boot protocol.
`grub-mkrescue` happens to produce an image that works with both; our path is
BIOS.

**Boot sector / MBR** — the first 512-byte sector of a bootable device, holding
the first-stage bootloader; it is the only code the BIOS will load sight unseen.
Here: `grub-mkrescue` embeds `boot_hybrid.img` in the ISO's system area so the
image also boots as if it were a disk.

**Bootloader** — a small program whose only job is to find a kernel, load it
into memory, and jump to it. Writing one yourself means talking to disks in real
mode; using GRUB is what the subject asks for.

**GRUB (GRand Unified Bootloader)** — the standard Linux bootloader, and the one
this project uses. It reads a config file, loads a kernel image, prepares the CPU
(protected mode, flat GDT) and jumps to the kernel's entry point. Here:
`grub.cfg`, and `grub-mkrescue` to build the ISO.

**GRUB module** — a piece of GRUB itself that GRUB loads on demand: filesystem
drivers, video drivers. `grub-mkrescue` copies its whole module tree onto the
ISO — 294 files in this build — which is why a 1 KB kernel yields a 5 MB image.

**`grub.cfg`** — GRUB's configuration file, read at boot from
`/boot/grub/grub.cfg` inside the image. Here: the repo's `grub.cfg`, copied
there by the Makefile.

**`menuentry`** — one bootable choice in `grub.cfg`. Ours is named `"kfs-1"` and
contains only `multiboot /boot/kernel.bin` and `boot`.

**Multiboot specification** — a handshake between bootloaders and kernels: put a
recognisable header near the front of your image and any compliant bootloader
will load it, handing you a machine already in protected mode plus a description
of memory. It saves every hobby kernel from writing a bootloader. We use
multiboot **1**; multiboot 2 is a different header format and a different GRUB
command.

**Multiboot header** — the 12 bytes that do the identifying: magic, flags,
checksum. GRUB scans the first 8 KiB of the binary for it, 4-byte aligned; find
it and GRUB loads and jumps, miss it and GRUB refuses the file. Here: `section
.multiboot` in `boot/boot.asm`, forced first by `linker.ld`.

**Magic number** — a fixed, arbitrary value used purely as a signature, so a
reader can tell "this is one of mine". Multiboot 1's is `0x1BADB002`.

**Checksum** — a value chosen so that a sum comes out to a known result. Here
`MAGIC + FLAGS + CHECKSUM` must equal 0 in 32-bit arithmetic, giving
`0xE4524FFB`. A wrong checksum makes GRUB refuse the file.

**Entry point** — the address the first instruction lives at. It is recorded in
the ELF header and chosen by `ENTRY(_start)` in the linker script; `0x100010`
here.

**Load address** — where an image's contents are placed in physical memory. Ours
is 1 MiB (`0x100000`), the conventional lowest address a kernel may claim, since
below it live the BIOS data area, the interrupt vector table, the VGA
framebuffer and mapped ROM.

**ISO 9660** — the filesystem format used on CD-ROMs. `kfs.iso` is one.

**El Torito** — the extension to ISO 9660 that makes a CD bootable.

**`grub-mkrescue`** — GRUB's tool for producing a bootable ISO from a directory
tree: it adds GRUB itself, its modules and the boot records. Fedora names the
binary `grub2-mkrescue`, Debian `grub-mkrescue`; the Makefile detects both.

**`grub-file`** — GRUB's inspection tool. `grub-file --is-x86-multiboot`
succeeds only if a valid multiboot header is present, so the build runs it as an
assertion straight after linking.

**`xorriso` / `mtools`** — the ISO-writing and FAT-manipulation utilities
`grub-mkrescue` calls under the hood. You never invoke them; you only notice
them when they are missing.

---

## Assembly

Assembly is here for one reason: some things cannot be said in a high-level
language. Rust has no way to write "set the stack pointer to this address", and
until the stack pointer is set, no function can be called at all. So a dozen
lines of assembly run first and then hand over to Rust for good.

**Assembly language** — a text form of machine instructions, one line per
instruction. Needed wherever no higher-level language can express the operation:
here, setting `ESP` before any function call can happen.

**Assembler** — the program that translates assembly text into an object file.
It is to assembly what a compiler is to C, with almost nothing left to decide.

**NASM (Netwide Assembler)** — the assembler this project uses, in Intel syntax
(`mov dst, src`: destination first, like `dst = src`). Invoked as `nasm -f elf32`
to produce a 32-bit ELF object.

**Mnemonic** — the human-readable name of an instruction: `mov`, `call`, `jmp`.

**Opcode / machine code** — the actual bytes the CPU executes.
`mov esp, 0x104690` is `bc 90 46 10 00`.

**Label** — a name for an address (`_start:`, `stack_top:`). In NASM a label
beginning with `.` belongs to the previous global label, which is why the hang
loop is `_start.hang`.

**Directive** — a line aimed at the assembler rather than the CPU, closer to a
`#define` or a pragma than to an instruction:
- `equ` — define a constant.
- `dd` — emit a 32-bit value ("define doubleword"). Used for the three header
  words.
- `resb` — reserve bytes without storing them in the file. Used for the stack.
- `align` — pad until the address is a multiple of N.
- `section` — start emitting into a named section.
- `global` — export a symbol so the linker can see it (`_start`).
- `extern` — declare a symbol defined elsewhere (`kmain`).

**Calling convention** — the rules a caller and callee agree on: which registers
carry the arguments, where the return value goes, who must restore what.
Assembly and Rust have to agree, and `extern "C"` is how both pick the same set.

**ABI (application binary interface)** — the wider contract between separately
compiled pieces of code: calling convention plus type sizes, alignment, and how
symbol names are spelled. Here: `"rustc-abi": "softfloat"` states that floats
are not passed in SSE registers.

---

## Compiling and linking

Normally your compiler and your OS between them decide what the binary looks
like and what addresses it occupies, and you never see the decision made. Here
nothing decides, so the layout is written out by hand and every address in it is
somebody's choice. These words name the parts of that machinery.

**Compiler** — translates a high-level language into machine code. Here:
`rustc`, driven by `cargo`.

**Cross compilation** — building code for a machine other than the one doing the
build. Every kernel build is one: the host runs Linux or macOS, the target runs
nothing.

**Host vs target** — the machine compiling versus the machine that will run the
output. Here: host `x86_64-unknown-linux-gnu` inside the container, target
`i686-kfs`.

**Target triple** — the conventional name for a target, roughly
`arch-vendor-os-environment`, e.g. `i686-unknown-linux-gnu`. You normally pick
one off a shelf; ours would be `i686-unknown-none`.

**Target spec JSON** — Rust's way to describe a target it does not already ship,
as a JSON file of properties. There is no shelf entry for "32-bit x86, no
operating system at all", so you write the spec sheet yourself. Here:
`kernel/i686-kfs.json`, enabled by the unstable `json-target-spec` flag.

**LLVM** — the compiler backend `rustc` hands its work to for the final step
down to machine code. The target spec's `llvm-target` and `data-layout` are
passed straight to it.

**Data layout** — LLVM's own description of a target: pointer size, and the
alignment of every integer and float type. It must match the LLVM target, or
generated code is subtly wrong with no warning.

**Object file** — the output of assembling or compiling one source file: machine
code plus sections, symbols and relocations, with nothing yet decided about
where in memory it goes. Here: `build/boot.o`.

**ELF (Executable and Linkable Format)** — the Unix binary format for object
files and executables. It describes the same bytes twice: as sections, for the
linker, and as segments, for whoever loads it. GRUB parses the ELF to know where
to put our code.

**Section** — a named region of a binary grouping similar content:
- `.text` — executable code.
- `.rodata` — read-only data: constants and string literals.
- `.data` — writable data whose initial values sit in the file.
- `.bss` — writable data that starts as zeros and occupies no file space.
- `COMMON` — legacy uninitialised symbols, folded into `.bss` here.
- `.comment` — toolchain version strings; not loaded.
- `.note.GNU-stack` — a marker declaring whether the stack must be executable.
- `.multiboot` — *our* custom section name, so the linker script can place the
  header first.

**NOBITS** — the ELF flag meaning "occupies memory but has no bytes in the
file". `.bss` is NOBITS: 16 KiB of RAM, 0 bytes on disk. It says "reserve 16 KiB
of zeros in RAM" rather than "ship 16 KiB of zeros inside the file" — closer to
`calloc` than to a literal array in the binary.

**Symbol** — a name attached to an address, so other code can refer to it
without knowing the number (`kmain`, `stack_top`).

**Symbol table** — the list of a binary's symbols and their addresses; read it
with `nm`.

**Relocation** — a hole in an object file with a note attached: "patch this
address in once the final layout is known". `call kmain` in `boot.o` is one, and
`ld` fills it with `0x100030`.

**Linker** — combines object files and libraries into one image, matching up
symbols and assigning final addresses. Here: GNU `ld`, called with
`-m elf_i386 -n -T linker.ld`.

**Linker script** — the file telling the linker what to place where. Normally
the compiler and the OS decide what address your code lives at; here nothing
does, so you write it out by hand. Here: `linker.ld`.

**Location counter (`.`)** — the linker script's cursor: the address it is
currently handing out. `. = 1M;` moves it to `0x100000`, so the first section
placed after that line starts there.

**VMA / LMA** — a section's virtual address (where the code believes it is) and
its load address (where it is actually put). Identical here, because paging is
off.

**Segment / program header** — the loader's view of an ELF: which byte ranges to
copy to which addresses with which permissions. Our kernel has one `LOAD`
segment, 1 680 bytes on disk expanding to 30 101 in RAM.

**Static library / archive (`.a`)** — a bundle of object files in one file.
`cargo` emits `libkernel.a` — a bag of parts rather than a finished program — so
*our* `ld` invocation does the final assembly with *our* linker script. Created
and listed with `ar`.

**Name mangling** — the compiler encoding module paths and types into a symbol's
name so same-named functions cannot collide. This kernel's panic handler is
mangled to `_RNvCschKVOpqoY1I_7___rustc17rust_begin_unwind` (visible whenever a
panic site exists — see ARCHITECTURE on LTO). `#[no_mangle]` switches it off so
assembly can refer to `kmain` by that exact name.

**`nm` / `objdump` / `readelf`** — the inspection tools: list symbols,
disassemble and dump sections, print ELF headers. Every address quoted in
[ARCHITECTURE.md](ARCHITECTURE.md) came from them.

---

## Rust specifics

Rust's standard library assumes an operating system beneath it: files, threads,
a heap, `println!`. None of that exists here, so most of this section is about
switching things off — the standard library, stack unwinding, name mangling,
prebuilt targets. What remains is roughly C with better types.

**Rust** — the systems language this kernel is written in: no runtime, no
garbage collector, memory safety checked at compile time, and `unsafe` blocks
for the places where hardware access cannot be proven safe.

**Cargo** — Rust's build tool and package manager in one, roughly `make` plus
`pip`/`npm`. Reads `Cargo.toml`.

**Crate** — Rust's unit of compilation: like a C translation unit, but
package-sized and compiled in one go. This repo has one, `kernel`.

**Crate type** — what a crate compiles to. `staticlib` produces a C-compatible
`.a` archive, which is what lets our own `ld` command do the final link.
(`rlib` would be Rust-only; `bin` would make cargo link an executable with the
wrong script.)

**`Cargo.toml` / `Cargo.lock`** — the manifest you write and the exact dependency
versions cargo resolved, like `package.json` and its lock file. The lock file is
committed so a rebuild is reproducible.

**Edition** — which dialect of the Rust language a crate is written in (2021
here). A compatibility marker, not a compiler version.

**Profile** — a named set of build settings. `[profile.release]` uses
`opt-level = 2` and `panic = "abort"`.

**`no_std`** — the attribute that unlinks the standard library. Roughly C's
`-ffreestanding` with no libc: necessary because `std` assumes an OS with
threads, files, a heap and syscalls.

**`core`** — the part of Rust's standard library that needs no operating system:
primitive types, `Option`, `Result`, slices, atomics, `PanicInfo`. Always
available, kernels included.

**`alloc`** — the layer between `core` and `std` providing `Box`, `Vec` and
`String`. It needs a working allocator, so it is out of reach until this kernel
has a heap.

**Panic handler** — the `#[panic_handler]` function every `no_std` crate must
provide, called when a panic occurs. Normally `std` supplies one that prints and
kills the process; ours loops forever, because there is nowhere to report to.

**Unwinding vs abort** — on a panic Rust normally unwinds the stack, running
destructors on the way out, which needs runtime support that does not exist here.
`panic = "abort"` in `Cargo.toml` and `"panic-strategy": "abort"` in the target
spec remove it.

**`#[no_mangle]`** — keep a symbol's name exactly as written. It is what
wrapping a declaration in `extern "C"` does to a name in C++.

**`extern "C"`** — use the C calling convention for a function, so non-Rust code
can call it correctly. Here: `kmain`, which `boot.asm` calls.

**Never type (`-> !`)** — the return type of a function that never returns.
`kmain() -> !` is why an infinite loop is required and why the code after
`call kmain` is unreachable.

**Nightly** — Rust's unstable release channel. Required here because rebuilding
`core` for a custom target and using JSON target files are both unstable
features. Pinned in `kernel/rust-toolchain.toml`.

**`rust-src`** — the rustup component shipping the standard library's *source*,
without which there is nothing to rebuild `core` from. Listed in
`kernel/rust-toolchain.toml`.

**`build-std`** — the unstable cargo feature that compiles `core` (and
`compiler_builtins`) from source for a target that has no prebuilt copy. Here:
`kernel/.cargo/config.toml`.

**`compiler_builtins`** — Rust's implementations of the low-level helper
routines LLVM assumes someone provides: 64-bit division on a 32-bit CPU, and,
with the `compiler-builtins-mem` feature, `memcpy`/`memset`/`memcmp`. On a
normal system libc supplies those; we have no libc, so we opt in.

**`volatile`** — a memory access the compiler may not optimise away, reorder or
merge. Mandatory for MMIO such as the VGA buffer, where the *act* of writing is
the point and the value stored is beside it. Here: specified for the planned
`vga.rs`.

**`unsafe`** — the keyword marking operations the compiler cannot verify, such
as dereferencing a raw pointer to `0xb8000`. It disables no checks; it moves the
burden of proof to the author.

**`core::hint::spin_loop()`** — a portable way to say "this loop is
spin-waiting"; on x86 it emits `pause`. Here: the whole body of `kmain`, and of
the panic handler.

---

## Build and test tooling

You cannot run this program. There is no `./kernel` to type, no test harness
inside it, and no `assert` that could tell you anything, because the kernel has
nowhere to print. So `make check` is a unit test for a whole machine: build a CD
image, boot it in an emulator, and ask the emulator from the outside where the
CPU ended up.

**Make** — the build automation tool that runs commands when files are out of
date. Its vocabulary: a **target** is a thing to build, its **prerequisites**
are what it depends on, and its **recipe** is the tab-indented commands.

**Phony target** — a Make target that names an action, not a file (`all`,
`clean`, `cargo`). Declared in `.PHONY` so Make never mistakes a same-named file
for it.

**Pattern rule** — a Make rule with a `%` wildcard. Here `docker-%:` matches
`docker-all` and `docker-check`, and passes `$*` — the part matched by `%` — to
`make` inside the container.

**QEMU** — the machine emulator used to run the kernel. `qemu-system-i386`
emulates a 32-bit PC: BIOS, CD-ROM, VGA, the works. Emulation rather than
virtualisation means it can run x86 code on an arm64 Mac, slowly but faithfully.

**`-cdrom kfs.iso`** — attach the ISO as the boot CD. Here: `make run` and
`make check`.

**`-display none`** — run with no window: headless, for scripted checks. Here:
`make check`.

**QEMU monitor** — QEMU's own control console, separate from anything the guest
can see or touch. It reports CPU state, dumps the screen, pauses and resets the
machine — a debugger attached to the emulated hardware rather than to a process.
Exposed here on a unix socket with `-monitor unix:/tmp/kfs-mon,server,nowait`.

**`info registers`** — the monitor command printing the guest's CPU registers.
The `EIP` it reports is the proof that the kernel is executing.

**`screendump file.ppm`** — the monitor command writing the current guest screen
to an image. `make check` uses it to confirm the "42" glyphs are lit in white
and that none of GRUB's grey leftover text survived the kernel's screen clear.

**Headless** — running with no display attached.

**`socat`** — a utility that pipes data between arbitrary endpoints. Here it
connects stdin/stdout to the monitor's unix socket, which is how `make check`
talks to a running QEMU without a terminal.

**Unix domain socket** — an inter-process channel addressed by a filesystem path
instead of a port. `/tmp/kfs-mon` is one.

**Docker** — container tooling, used only to build on a machine with no x86
toolchain. An **image** is a filesystem template built from the `Dockerfile`; a
**container** is a running instance; a **bind mount** (`-v $(PWD):/kfs`) exposes
the host's repo inside it so build artifacts land in the working tree.
`--platform=linux/amd64` requests the x86 image on an arm64 host, which Docker
runs under emulation. The evaluation machine uses none of this.
