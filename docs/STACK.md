# The kernel stack and the stack printer

The `stack` command of the shell prints a hex dump of the live kernel stack.
This file explains what the kernel stack is on this machine, how
`kernel/src/stack.rs` reads it, what the dump means, and how `make check`
proves the dump. The shell itself is in [SHELL.md](SHELL.md), and the screen
under `printk!` is in [VGA.md](VGA.md).

## What the kernel stack is here

There is no thread and no process. The kernel stack is one fixed region of
memory. `boot/boot.asm` reserves 16384 bytes of it in `.bss`, and `_start`
loads `stack_top` into the stack pointer before it calls `kmain`. Every call in
this kernel runs on that region.

`boot.asm` exports both bounds:

```asm
section .bss
align 16
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

The stack grows down. The part in use is the range from the live stack pointer
up to `stack_top`. Everything below the stack pointer is free memory, or the
remains of calls that already returned. In the measured build, `stack_bottom`
is 0x00103710 and `stack_top` is 0x00107710:

```
  low addresses
  0x00103710   stack_bottom  +--------------------------+
                             |                          |
                             |          free            |   the stack
                             |                          |   grows down
     the stack pointer  -->  +--------------------------+   in this
                             |                          |   direction
                             |         in use           |        |
                             |    (the dump window)     |        v
  0x00107710   stack_top     +--------------------------+
  high addresses
```

One byte below `stack_bottom` overflows the stack. Nothing in this kernel
detects that today.

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
00107668  3c 06 10 00 00 00 00 00  97 76 10 00 97 76 10 00 |<........v...v..|
00107678  00 00 00 00 93 76 10 00  92 76 10 00 05 00 00 00 |.....v...v......|
00107688  00 00 00 00 00 00 00 00  00 00 73 74 61 63 6b 00 |..........stack.|
00107698  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
```

The fix is one attribute:

```rust
#[inline(never)]
pub fn print(esp: u32, bytes: usize) {
```

`#[inline(never)]` is the counterpart of the `#[inline(always)]` on `esp()`.
One attribute keeps the read in the caller. The other keeps the buffer out of
the caller. Both attributes are necessary, and neither is a hint about speed.

The check now asserts that the first dword of the dump is a return address
inside the kernel. That single assertion catches this class of defect. A buffer
inside the window overwrites the return address with a stack address, and a
stack address fails the range test at once.

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
value comes first. The saved address 0x0010063c appears in the hex column as
`3c 06 10 00`. A return address therefore reads backwards, and the reader
assembles it from right to left.

The last row is often partial, because the window is rarely a multiple of 16.
The hex column pads. `chunk.get(i)` returns `None` for a byte that does not
exist, and `render` prints three spaces in its place, so the columns stay
aligned. The text column does not pad. It ends after the real bytes, so the
partial row is shorter than 77 columns:

```
00107708  00 00 00 00 1a 00 10 00                          |........|
```

## How to read a real dump

This is the measured output of one run. The header names the captured stack
pointer, the top, the bytes in use, and the size of the region. 168 bytes give
11 rows: ten full rows and one row of 8 bytes. All eleven rows appear below:

```
kfs> stack
stack: esp 0x00107668 -> top 0x00107710, 168 of 16384 bytes in use
00107668  3c 06 10 00 00 00 00 00  97 76 10 00 97 76 10 00 |<........v...v..|
00107678  00 00 00 00 93 76 10 00  92 76 10 00 05 00 00 00 |.....v...v......|
00107688  00 00 00 00 00 00 00 00  00 00 73 74 61 63 6b 00 |..........stack.|
00107698  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
001076a8  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
001076b8  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
001076c8  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00 |................|
001076d8  00 00 00 00 00 00 00 00  00 00 00 00 fc 36 10 00 |.............6..|
001076e8  08 77 10 00 6b 23 10 00  00 00 00 00 00 00 00 00 |.w..k#..........|
001076f8  34 32 00 00 00 00 00 00  00 00 00 00 00 00 01 00 |42..............|
00107708  00 00 00 00 1a 00 10 00                          |........|
kfs>
```

Four landmarks in that dump:

- **`3c 06 10 00` at 0x00107668.** These are the first four bytes of the
  window, and they hold 0x0010063c. That is the return address into
  `dispatch`. `cmd_stack` calls `stack::esp()` in its own frame, so the window
  starts in `cmd_stack` and the return address to its caller sits at the
  bottom. This row is the shortest proof that the dump reads real stack.
- **`73 74 61 63 6b 00` at 0x00107692.** The text column shows `stack.` The
  line buffer of the shell still holds the word that the operator typed.
  `read_line` writes into a local `[u8; LINE_MAX]` of `run`, and that local
  lives inside the window, above `cmd_stack`.
- **`34 32` at 0x001076f8.** The text column shows `42`. `kmain` built the
  mandatory `42` with `klib::utoa` into a local buffer, and the bytes remain
  where that dead frame left them. Nothing overwrote them, because the shell
  runs above `kmain` and never returns.
- **`1a 00 10 00` at 0x0010770c.** These are the last four bytes below
  `stack_top`, and they hold 0x0010001a. `_start` pushed that address when it
  ran `call kmain`. It is the deepest return address in the kernel, and the
  stack pointer was at `stack_top` at that moment.

The other three kinds of value are ordinary. A value inside the range
0x00103710 to 0x00107710 is a saved stack pointer, or a saved frame register.
A value below 0x00103710 is a code address or a data address inside the image.
A run of zeros is untouched `.bss`, or a local that the kernel never wrote.

## Why there is no frame-pointer backtrace

A backtrace needs two things that this kernel does not have.

The first is a frame pointer. `kernel/i686-kfs.json` names no frame-pointer
option, so rustc omits the frame pointer where it can. `ebp` is an ordinary
register in this kernel, and it holds whatever the optimiser put there. A walk
through `[ebp]` therefore reads an arbitrary value as a link, follows it, and
prints structured nonsense. Structured nonsense is worse than a hex dump,
because a reader trusts it.

The second is a symbol table. The image carries no symbol table at run time.
`linker.ld` keeps `.multiboot`, `.text`, `.rodata`, `.data`, `.got` and `.bss`,
and it discards `.eh_frame` and `.comment`. Nothing maps an address to a name.

So an honest backtrace prints bare addresses, and nothing more. A reader must
resolve every one of them by hand against `nm` or `objdump` on the host. The
hex dump already shows those same addresses, and it shows the data next to
them.

Two costs make a real backtrace. First, the target file must force the frame
pointer, which costs a register and a prologue in every function. Second, the
image must carry a symbol table, which costs bytes in `.rodata` and a build
step that generates it. Both costs buy nothing until a fault handler needs to
report where the fault came from. That handler arrives with the interrupt
descriptor table, so this work belongs with the interrupt work and not here.

Bytes are true whatever the optimiser did. That is the reason the dump comes
first.

## The proof

`tools/check.sh` drives the shell from outside the guest, over the QEMU human
monitor. It types `stack` with `sendkey`, reads the text buffer of the screen
at 0xb8000, and decodes the low byte of every cell. Six of its 20 assertions
cover the dump:

```
OK: header is self-consistent: 168 of 16384 bytes below 0x00107710
OK: 168 bytes rendered as 11 rows of 16
OK: first row starts at 0x00107668, the address the header names
OK: the first dword is 0x0010063c, a return address inside the kernel
OK: the ASCII column shows the typed command in the line buffer
OK: dump esp 0x00107668 is 4 bytes deeper than ESP=0x0010766c
```

What each one rules out:

1. **The header is self-consistent.** The check parses the four numbers out of
   the header. It requires a size of exactly 16384, a stack pointer below the
   top, and a difference that equals the reported bytes in use. This rules out
   a header with a stale bound, a swapped pair, or arithmetic that overflows.
2. **168 bytes came out as 11 rows of 16.** The check counts the rows and
   compares against `(used + 15) / 16`. This rules out a dump that drops the
   partial last row, prints an extra row, or stops early on a short pass.
3. **The first row starts at the address the header names.** This rules out an
   address column that prints an offset from zero, or an address that comes
   from the buffer instead of the source.
4. **The first dword is a return address inside the kernel.** The check
   reverses the four bytes and requires a value between 0x00100000 and
   0x00200000. This rules out the alias defect. A buffer inside the window puts
   a stack address here, and a stack address is far outside that range.
5. **The text column shows the typed command.** The check searches the dump rows
   for the word `stack`. This rules out a text column that prints the hex
   digits again. It also proves that the bytes are the real line buffer of
   the shell.
6. **The captured stack pointer is 4 bytes deeper than the live one.** The
   check reads `ESP` with `info registers` after the dump. The guest is then
   one frame shallower, and x86 stacks grow down. So the captured value must
   sit below the live value, and within 1024 bytes of it. This ties the dump to the
   CPU, and it rules out a dump of some other region of memory.
