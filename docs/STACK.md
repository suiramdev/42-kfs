# The kernel stack and the stack printer

The `stack` command of the shell prints a hex dump of the live kernel stack.
This file explains what the kernel stack is on this machine, how
`kernel/src/stack.rs` reads it, what the dump means, who else calls the
printer, and how `make check` proves the dump. The shell itself is in
[SHELL.md](SHELL.md), the screen under `printk!` is in [VGA.md](VGA.md), and
the fatal path that prints the second dump is in [PANIC.md](PANIC.md).

## What the kernel stack is here

There is no thread and no process. The kernel stack is one fixed region of
memory. `boot/boot.asm` reserves 16384 bytes of it in `.bss`, and the
higher-half entry loads `stack_top` into the stack pointer before it calls
`kmain`. Every call in this kernel runs on that region.

`boot.asm` exports both bounds, after the two page frames that kfs-3 added
ahead of them:

```asm
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
```

Only the linker knows where `.bss` lands, so Rust cannot compute these two
addresses. `stack.rs` declares them as `extern "C"` statics and reads their
addresses with `addr_of!`. The two functions `top()` and `bottom()` return the
real bounds of the real build.

kfs-3 moved those bounds. The whole kernel image runs in the higher half now
([PAGING.md](PAGING.md)), so `.bss` moved with it, and the stack inside `.bss`
moved too. In the measured build `stack_bottom` is 0xc0110000 and `stack_top`
is 0xc0114000. The size did not change.

The stack grows down. The part in use is the range from the live stack pointer
up to `stack_top`. Everything below the stack pointer is free memory, or the
remains of calls that already returned:

```
  low addresses
  0xc0110000   stack_bottom  +--------------------------+
                             |                          |
                             |          free            |   the stack
                             |                          |   grows down
     the stack pointer  -->  +--------------------------+   in this
                             |                          |   direction
                             |         in use           |        |
                             |    (the dump window)     |        v
  0xc0114000   stack_top     +--------------------------+
  high addresses
```

One byte below `stack_bottom` overflows the stack. Nothing in this kernel
detects that today, and kfs-3 gave the overflow something worth damaging. The
two reservations directly below the stack are the boot page directory and the
boot low page table, physical 0x0010e000 and 0x0010f000, and CR3 still holds
the first of them. A write one byte below the stack lands in a live page table.

`.bss` is 176129 bytes in this build, and the frame bitmap of the physical
allocator is 128 KiB of that ([MEMORY.md](MEMORY.md)). The stack is unaffected
by that growth. `boot/boot.asm` reserves it explicitly, so it keeps its own
16384 bytes wherever `.bss` ends up, and the bitmap and the interrupt
descriptor table sit above `stack_top` rather than inside the stack.

## The named data shape

A dump has two parts. The first part is a copy of the bytes between the live
stack pointer and `stack_top`. The second part is the address that the copy
came from. Those two parts are the whole shape. `snapshot(from, buf)` produces
the first part, and `from` is the second part.

`snapshot` stops at `stack_top`, or when the buffer is full, and returns the
part that it filled. The buffer is one `WINDOW` of 256 bytes. A longer dump
costs another pass and not a bigger frame, because `print` calls `snapshot`
again with the same buffer.

A row is not part of the shape. The value 16 is a screen width, not a property
of the stack. `render` calls `chunks(PER_ROW)` and decides the row at the
moment that it prints. Move the dump to a serial port of 132 columns, and only
`render` changes.

`render` also names no stack concept and reads no raw pointer. It takes a base
address and a byte slice. That boundary keeps every unsafe read on the other
side, inside `snapshot`.

## Why the dump is a copy and not a view

A `&[u8]` over live stack memory is a false claim. A shared slice tells the
compiler that nobody writes those bytes for the whole life of the slice. The
printer writes there on its very next call, because `printk!` and `render` both
run on the same stack. The compiler is then free to cache a byte, to reorder a
read, or to delete a read, and the dump becomes a fiction.

So `snapshot` copies. It reads every byte with `read_volatile`:

```rust
for (i, slot) in buf[..n].iter_mut().enumerate() {
    *slot = unsafe { read_volatile((from as *const u8).add(i)) };
}
```

`read_volatile` is the honest read for an address that comes from an integer
cast. Such an address carries no provenance that a Rust memory model
recognises, so a plain read has no defined meaning. Volatile also stops the
optimiser. The optimiser cannot fold these reads against the caller's own
locals, and those locals live in the same region.

## How the kernel reads the stack pointer

`esp()` is four lines of inline assembly:

```rust
#[inline(always)]
pub fn esp() -> u32 {
    let esp: u32;
    unsafe { asm!("mov {}, esp", out(reg) esp, options(nomem, nostack, preserves_flags)) };
    esp
}
```

The `#[inline(always)]` carries the whole contract. A real call pushes a return
address first, and it moves the stack pointer down. The answer then describes
`esp()` and not the code that asked for it. With the attribute, the `mov` runs
in the caller's frame and reports the caller's stack pointer.

For the same reason, `print` takes the stack pointer as a parameter. A read
inside `print` reports the frame of `print`, and the caller's saved registers
then sit below the window instead of inside it.

## The alias defect and the fix

Read this section before any change to the attributes on `print`.

`print` must own a frame of its own. The copy buffer is a local of `print`, and
a local sits inside the frame that holds it. The buffer must sit below the
captured stack pointer, because the window starts at that pointer and runs
upward. A separate frame puts the buffer below the window, and the two never
touch.

An inlined `print` breaks that rule. The buffer then belongs to the caller's
frame, which is inside the window. `snapshot` writes byte 0 of the buffer, and
some later read of the window reads that same byte back. Every read past the
start of the buffer returns a byte that the copy already wrote, so the output
repeats with a period equal to the distance from the captured stack pointer up
to the buffer. That distance was 64 bytes in the build that showed the defect.

The shape of the failure looks like this. The addresses advance, and the bytes
repeat:

```
  addr+0x00   .. 16 bytes: block A ..                   |....|
  addr+0x10   .. 16 bytes: block B ..                   |....|
  addr+0x20   .. 16 bytes: block C ..                   |....|
  addr+0x30   .. 16 bytes: block D ..                   |....|
  addr+0x40   .. block A again, byte for byte ..        |....|
  addr+0x50   .. block B again, byte for byte ..        |....|
```

That output looks like real stack content. Structure repeats on a real stack
too, because a loop makes similar frames. The failure hides in plain sight, and
that is the reason for the attribute and for this section.

The correct output has no period. Each of these four rows differs from the
other three:

```
c0113f30  60 d5 10 c0 a1 36 10 c0  01 00 00 00 00 00 00 00 |`....6..........|
c0113f40  05 00 00 00 05 00 00 00  01 00 00 00 00 00 00 00 |................|
c0113f50  72 3f 11 c0 00 00 00 00  00 00 10 00 76 3f 11 c0 |r?..........v?..|
c0113f60  72 3f 11 c0 05 00 00 00  00 00 00 00 00 00 00 00 |r?..............|
```

The fix is one attribute:

```rust
#[inline(never)]
pub fn print(esp: u32, bytes: usize) {
```

`#[inline(never)]` is the counterpart of the `#[inline(always)]` on `esp()`.
One attribute keeps the read in the caller. The other keeps the buffer out of
the caller. Both attributes are necessary, and neither is a hint about speed.

The check asserts that the first dword of the dump lies between 0xc0100000 and
0xc0200000. Read that assertion for what it is. The range covers the whole
kernel image, and in the higher half it also covers the kernel stack at
0xc0110000, so the test no longer tells a stack address from an image address.
It still rejects a dword that is neither, which is what a dump of the wrong
region gives. The reliable signature of the alias defect is the period in the
output, and the four rows above are the reference for a dump that has none.

## The output format

`render` prints one row per 16 bytes, and a full row is 77 columns:

| Columns | Content |
| --- | --- |
| 1 to 8 | the address of the first byte of the row, 8 hex digits |
| 9 to 10 | two spaces |
| 11 to 34 | bytes 0 to 7, each one as `xx` and a space |
| 35 | one extra space, the half-row mark |
| 36 to 59 | bytes 8 to 15, each one as `xx` and a space |
| 60 | a vertical bar |
| 61 to 76 | the 16 characters of the text column |
| 77 | a vertical bar |

The address column holds a real address, not an offset. The text column shows
one character per byte. A byte in the range 0x20 to 0x7e prints as itself, and
every other byte prints as a full stop.

Bytes appear in memory order. x86 is little-endian, so the lowest byte of a
value comes first. The address 0xc01036a1 appears in the hex column as
`a1 36 10 c0`. A return address therefore reads backwards, and the reader
assembles it from right to left.

The last row is partial when the window is not a multiple of 16. The hex column
pads. `chunk.get(i)` returns `None` for a byte that does not exist, and `render`
prints three spaces in its place, so the columns stay aligned. The text column
does not pad. It ends after the real bytes, so a partial row is shorter than 77
columns. No captured dump in this build shows one: every measured stack pointer
is 16-byte aligned, `stack_top` is aligned too, and the panic path asks for
exactly 64 bytes.

## How to read a real dump

This is the measured output of one run. The header names the captured stack
pointer, the top, the bytes in use, and the size of the region. 208 bytes give
13 rows of 16, with no partial row. All thirteen appear below:

```
kfs> stack
stack: esp 0xc0113f30 -> top 0xc0114000, 208 of 16384 bytes in use
c0113f30  60 d5 10 c0 a1 36 10 c0  01 00 00 00 00 00 00 00 |`....6..........|
c0113f40  05 00 00 00 05 00 00 00  01 00 00 00 00 00 00 00 |................|
c0113f50  72 3f 11 c0 00 00 00 00  00 00 10 00 76 3f 11 c0 |r?..........v?..|
c0113f60  72 3f 11 c0 05 00 00 00  00 00 00 00 00 00 00 00 |r?..............|
c0113f70  00 00 73 74 61 63 6b 00  00 00 00 00 00 00 00 00 |..stack.........|
c0113f80  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
c0113f90  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
c0113fa0  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
c0113fb0  00 00 00 00 00 00 00 00  00 00 00 00 02 b0 ad 2b |...............+|
c0113fc0  00 00 01 00 60 d5 10 c0  f0 3f 11 c0 f1 77 10 c0 |....`....?...w..|
c0113fd0  00 00 00 00 00 00 00 00  34 32 00 00 00 00 00 00 |........42......|
c0113fe0  00 00 00 00 02 b0 ad 2b  00 00 01 00 00 00 01 00 |.......+........|
c0113ff0  00 40 11 c0 0e 10 10 c0  02 b0 ad 2b 00 00 01 00 |.@.........+....|
kfs>
```

Four landmarks in that dump:

- **`60 d5 10 c0` at 0xc0113f30.** These are the first four bytes of the
  window, and they hold 0xc010d560. `.data` runs from 0xc010d000 to
  0xc010d56c, so this is a data address that the frame saved, and not a return
  address; the fatal reports in [PANIC.md](PANIC.md) show the same value in
  `ebx`. The next four bytes hold 0xc01036a1, which is inside `.text`
  (0xc0101000 to 0xc010949b), and that is the shape of a return address into
  the shell. Either way the first row is the shortest proof that the dump reads
  real stack.
- **`73 74 61 63 6b 00` at 0xc0113f72.** The text column shows `stack.` The
  line buffer of the shell still holds the word that the operator typed.
  `read_line` writes into a local `[u8; LINE_MAX]` of `run`, and that local
  lives inside the window, above the frame that captured the stack pointer. The
  eight bytes `72 3f 11 c0 05 00 00 00` at 0xc0113f60 are the `&str` that
  `read_line` returned: the pointer 0xc0113f72 to that buffer, then the length
  5 for the five letters of `stack`.
- **`34 32` at 0xc0113fd8.** The text column shows `42`. `kmain` built the
  mandatory `42` with `klib::utoa` into a local buffer, and the bytes remain
  where that dead frame left them. Nothing overwrote them, because the shell
  runs above `kmain` and never returns.
- **The last four dwords, 0xc0113ff0 to 0xc0113ffc.** They are the deepest
  frame in the kernel, and they decode as `boot/boot.asm` wrote them.
  0xc0113ffc holds 0x00010000, the pointer to the multiboot information
  structure that GRUB left in low memory. `_start` moved it from `ebx` into
  `edi`, and `higher_half` pushed it first, so it sits highest. 0xc0113ff8
  holds 0x2badb002, the multiboot magic that arrived in `eax`, went to `esi`
  and was pushed second. 0xc0113ff4 holds 0xc010100e, the return address that
  `call kmain` pushed. 0xc0113ff0 holds 0xc0114000, the `ebp` that `kmain`
  saved on entry, and `higher_half` had just set `ebp` to `stack_top`. The
  stack pointer was at `stack_top` when the first push happened.

The other kinds of value are ordinary. A value inside the range 0xc0110000 to
0xc0114000 is a saved stack pointer, a saved frame register, or a pointer to a
local, as 0xc0113f72 is. A value between 0xc0100000 and 0xc0110000 is a code
address or a data address inside the image. A small value such as 0x00010000 is
a physical address in low memory, which the low window still maps. A run of
zeros is untouched `.bss`, or a local that the kernel never wrote.

## The second caller: the fatal panic

Until kfs-3 the shell was the only caller of `print`. The fatal panic path is
the second one. `panic::machine` prints CR0, CR2, CR3 and the count of
recovered panics, and then asks for a window of the live stack:

```rust
let esp = stack::esp();
if esp >= stack::bottom() && esp < stack::top() {
    stack::print(esp, 64);
}
```

Two decisions sit in those four lines.

The window is 64 bytes and not `WINDOW`. A fatal report has to fit on one
screen beside five lines of registers, and four rows are enough to show the
frame that faulted.

The guard is the reason this caller needs any code of its own. `print` reads
every byte of the window with `read_volatile` through a pointer built from an
integer. That read is safe on the kernel stack and nowhere else. A panic can
arrive with a stack pointer that points anywhere: a wild `esp`, a frame that
overflowed past `stack_bottom`, or a fault taken before the entry stub set `esp`
at all. A read there can fault again, and a second fault inside the fatal path
has nowhere to go, because the report is the last thing the machine will ever
print. So the panic prints the window only when `esp` is inside
`stack_bottom..stack_top`, and prints nothing at all otherwise.
[PANIC.md](PANIC.md) covers the rest of the report.

These are the four rows of the captured `fault ro` panic:

```
stack: esp 0xc0113e10 -> top 0xc0114000, 496 of 16384 bytes in use
c0113e10  11 00 01 80 00 10 10 c0  00 e0 10 00 00 00 00 00 |................|
c0113e20  10 3e 11 c0 50 92 10 c0  14 3e 11 c0 50 92 10 c0 |.>..P....>..P...|
c0113e30  18 3e 11 c0 50 92 10 c0  1c 3e 11 c0 50 8f 10 c0 |.>..P....>..P...|
c0113e40  f4 3e 11 c0 50 92 10 c0  60 d5 10 c0 70 29 10 c0 |.>..P...`...p)..|
```

The header still says 496 bytes in use, because that is `stack_top` minus the
live stack pointer. The dump shows 64 of them, because that is what this caller
asked for.

The first four dwords are the values that `machine()` was printing one line
earlier: 0x80010011 is CR0, 0xc0101000 is CR2, 0x0010e000 is CR3, and
0x00000000 is the count of recovered panics. The eight dwords after them are
four pairs, one per value: a pointer to the value, then the address of the
function that formats it. The three registers share the formatter 0xc0109250,
and the count uses 0xc0108f50 instead, because a count prints as a decimal
number where the registers print as `{:#010x}`. That array of pairs is how
`format_args!` hands its arguments to `printk!`, and this window is the
plainest view of it that the kernel offers. The last row belongs to the frame
above, and it starts the same shape again.

## Why there is no frame-pointer backtrace

A backtrace needs two things that this kernel does not have.

The first is a frame pointer. `kernel/i686-kfs.json` names no frame-pointer
option, so rustc omits the frame pointer where it can. `ebp` is an ordinary
register in this kernel, and it holds whatever the optimiser put there. A walk
through `[ebp]` therefore reads an arbitrary value as a link, follows it, and
prints structured nonsense. Structured nonsense is worse than a hex dump,
because a reader trusts it.

The second is a symbol table. The image carries no symbol table at run time.
`linker.ld` keeps `.boot` with the multiboot header inside it, then `.text`,
`.rodata`, `.data` with `.got`, and `.bss`. It discards `.eh_frame` and
`.comment`. Nothing maps an address to a name.

So an honest backtrace prints bare addresses, and nothing more. A reader must
resolve every one of them by hand against `nm` or `objdump` on the host. The
hex dump already shows those same addresses, and it shows the data next to
them.

Two costs make a real backtrace. First, the target file must force the frame
pointer, which costs a register and a prologue in every function. Second, the
image must carry a symbol table, which costs bytes in `.rodata` and a build
step that generates it.

kfs-3 brought the fault handler that would spend those costs, and it still does
not need to. The trap frame carries `eip`, `ebp` and `esp` as they were at the
fault, so the fatal report names the faulting instruction exactly
([PANIC.md](PANIC.md)). A backtrace would add the chain of callers above that
instruction, and nothing in the three subjects so far has needed it.

Bytes are true whatever the optimiser did. That is the reason the dump comes
first.

## The proof

`tools/check.sh` drives the shell from outside the guest, over the QEMU human
monitor. It types `stack` with `sendkey`, reads the text buffer of the screen
at 0xb8000, and decodes the low byte of every cell. `make check` holds 36
assertions in total, and four of them cover the dump:

```
OK: header is self-consistent: 208 of 16384 bytes below 0xc0114000
OK: first row starts at 0xc0113f30, the address the header names
OK: the first dword is 0xc010d560, a kernel pointer from outside the stack
```

Those three carry the numbers of the captured dump. The fourth prints two stack
pointers and the distance between them, both read at run time: the `esp` from
the dump header, and the live `ESP` that `info registers` reports after the
dump. The captured screens hold no register read, so its numbers are not
repeated here. What each assertion rules out:

1. **The header is self-consistent.** The check parses the four numbers out of
   the header. It requires a size of exactly 16384, a stack pointer below the
   top, and a difference that equals the reported bytes in use. This rules out
   a header with a stale bound, a swapped pair, or arithmetic that overflows.
   The size is the one number in the header that the check knows in advance,
   because `boot/boot.asm` reserves 16384 bytes and nothing else may change
   that.
2. **The first row starts at the address the header names.** The check compares
   the first eight columns of the first row against the `esp` from the header.
   This rules out an address column that prints an offset from zero, or an
   address that comes from the buffer instead of the source.
3. **The first dword is inside the kernel image.** The check reverses the four
   bytes and requires a value between 0xc0100000 and 0xc0200000. The message
   calls that value a return address; 0xc010d560 is in `.data`, so the honest
   reading is narrower. What the assertion proves is that the first dword is an
   address in the higher half and not a byte offset, a zero, or a value from
   some other region of memory. The range includes the kernel stack itself, so
   it does not by itself exclude the alias defect. The period in the output does
   that, and the four rows in the section above are the reference.
4. **The captured stack pointer is deeper than the live one.** The check reads
   `ESP` with `info registers` after the dump. The guest is then one frame
   shallower, and x86 stacks grow down, so the captured value must sit below the
   live value and within 1024 bytes of it. This ties the dump to the CPU, and it
   rules out a dump of some other region of memory.

Two assertions that kfs-2 had are gone. The row count and the text column of
the line buffer are no longer asserted; `make check` spends its assertions on
paging, the heaps and the three fatal panics instead, and those are listed in
[PAGING.md](PAGING.md), [MEMORY.md](MEMORY.md) and [PANIC.md](PANIC.md).
