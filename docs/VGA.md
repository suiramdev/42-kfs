# Screen output

The subject requires the kernel to display "42". It does: `kmain` clears the
screen and prints "42" through `kernel/src/vga.rs` before settling into its
idle loop. This file explains what "the screen" actually is on this machine,
how the module drives it, and how the result is verified.

## What "the screen" actually is here

There is no console. No `printf`, no file descriptor, no driver stack. What the
PC gives you is a rectangle of memory that the display hardware reads on its
own, sixty times a second, and paints.

That rectangle lives at physical address `0xb8000`. It is 80 columns by 25 rows,
two bytes per cell:

```
   high byte              low byte
 ┌───────┬───────┐   ┌───────────────┐
 │  bg   │  fg   │   │  ASCII code   │
 └───────┴───────┘   └───────────────┘
   4 bits   4 bits         8 bits
```

So the entire "driver" is: write the number `0x0F34` to the right address and a
white-on-black `4` appears. `0x0F` is the colour (foreground 15 = white,
background 0 = black); `0x34` is ASCII `'4'`.

This is **memory-mapped I/O**: an address that is really a device. Writing to it
changes hardware instead of storing a value. That distinction matters to the
compiler — see `volatile` below.

## The implementation

`kernel/src/vga.rs`, declared with `mod vga;` in `kernel/src/lib.rs`.

- **Access:** `0xb8000 as *mut u16`, every write through
  `core::ptr::write_volatile`. `volatile` tells the compiler it may not delete,
  merge or reorder these writes. Without it the optimiser is entitled to throw
  away a store whose result is never read back — which is every store here,
  because the *act* of writing is the whole point. Touching a raw pointer needs
  `unsafe`; that keyword does not switch off any checks, it just moves the
  burden of proof from the compiler to us. The one condition that makes the
  pointer arithmetic sound — the cell index stays below 80x25 — is asserted in
  the single helper every write funnels through.
- **No allocation, no `core::fmt`.** There is no heap, and formatting machinery
  is far more than "put two characters on screen" needs.
- **API, first iteration:**
  - `pub fn clear()` — fill all 80x25 cells with `0x0F20`, a white-on-black
    space.
  - `pub fn print(s: &str)` — for each byte `b` of `s`, write `0x0F00 | b as u16`
    starting at cell 0, incrementing the index. Non-ASCII bytes go through
    as-is and render as code page 437 glyphs. Output past cell 1999 is dropped;
    no wrapping or scrolling exists yet.
- **`kmain`:** `vga::clear(); vga::print("42");` before the idle `loop`.

### What the compiler made of it

The whole module optimises down to 37 bytes of code inside `kmain` (52 with
its idle loop and alignment padding):

```
00100030 <kmain>:
  100030: b8 60 f0 ff ff    mov  $0xfffff060,%eax
  ...
  100040: 66 c7 80 a0 8f 0b 00 20 0f   movw $0xf20,0xb8fa0(%eax)
  100049: 83 c0 02                     add  $0x2,%eax
  10004c: 75 f2                        jne  100040
  10004e: 66 c7 05 00 80 0b 00 34 0f   movw $0xf34,0xb8000
  100057: 66 c7 05 02 80 0b 00 32 0f   movw $0xf32,0xb8002
  100060: f3 90                        pause
  100062: eb fc                        jmp  100060
```

`clear()` became a counted loop storing `0x0F20` (white-on-black space) into
all 2 000 cells — the negative starting index is just the optimiser's way of
making the loop end when `eax` hits zero. `print("42")` was unrolled completely:
two immediate stores of `0x0F34` and `0x0F32` straight into `0xb8000` and
`0xb8002`. The string `"42"` never even reached `.rodata`; the section stayed
empty. And the `volatile` contract held: every store is present, in order,
none merged — the optimiser reshaped the *loop*, never the *writes*.

## Later, for the bonus part

A colour parameter on `print`; cursor tracking and scrolling (shift rows up with
a `copy` once output passes row 24); moving the blinking hardware cursor through
I/O ports `0x3D4` and `0x3D5`; and a `printk`-style formatter implementing
`core::fmt::Write`, so `write!` works.

## How it is verified

`make check` gained a screen assertion next to the `EIP` one — same principle,
ask the emulator from outside, because the kernel has no way to report on
itself. The QEMU monitor dumps the guest's framebuffer with
`screendump build/screen.ppm`, and the check requires two things of that frame:

1. **Some pure-white pixels exist** — the "42" glyphs, drawn with attribute
   `0x0F`, render `#ffffff`.
2. **No `#a8a8a8` pixels remain** — that grey is the colour of GRUB's leftover
   boot text, so its absence proves `clear()` really overwrote the buffer.

After `clear()` the frame contains only black and white, so grepping the raw
P6 byte stream for the two colour triples cannot false-positive across pixel
boundaries. A successful run prints:

```
OK: guest alive, EIP=00100062 inside kernel
OK: screen cleared and "42" glyphs lit
```

The other white thing in the frame is the hardware cursor, still blinking at
row 2 where GRUB parked it — nothing moves it yet (that is the bonus's I/O
port work). It blinks in the attribute of the cell it sits on, which `clear()`
set to white-on-black, so it does not disturb either assertion.
