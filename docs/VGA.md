# Screen output — not implemented yet

The subject requires the kernel to display "42". It does not, yet. `kmain` idles
in a two-instruction loop and never touches the screen, which is why booting the
ISO leaves GRUB's last few characters sitting there
([ARCHITECTURE.md §5](ARCHITECTURE.md#5-why-the-screen-looks-frozen) explains
that in detail). This file is the design for the follow-up that fixes it.

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

## Planned implementation

A new module `kernel/src/vga.rs`, declared with `mod vga;` in
`kernel/src/lib.rs`.

- **Access:** `0xb8000 as *mut u16`, every write through
  `core::ptr::write_volatile`. `volatile` tells the compiler it may not delete,
  merge or reorder these writes. Without it the optimiser is entitled to throw
  away a store whose result is never read back — which is every store here,
  because the *act* of writing is the whole point. Touching a raw pointer needs
  `unsafe`; that keyword does not switch off any checks, it just moves the
  burden of proof from the compiler to us.
- **No allocation, no `core::fmt`.** There is no heap, and formatting machinery
  is far more than "put two characters on screen" needs.
- **API, first iteration:**
  - `pub fn clear()` — fill all 80x25 cells with `0x0F20`, a white-on-black
    space.
  - `pub fn print(s: &str)` — for each byte `b` of `s`, write `0x0F00 | b as u16`
    starting at cell 0, incrementing the index. Non-ASCII bytes go through
    as-is and render as code page 437 glyphs. No wrapping or scrolling needed
    while the output fits one line.
- **`kmain` change:** replace the bare spin loop body with `vga::clear();
  vga::print("42");` before the `loop {}`.

## Later, for the bonus part

A colour parameter on `print`; cursor tracking and scrolling (shift rows up with
a `copy` once output passes row 24); moving the blinking hardware cursor through
I/O ports `0x3D4` and `0x3D5`; and a `printk`-style formatter implementing
`core::fmt::Write`, so `write!` works.

## How it gets verified

`make check` regains a screen assertion. The QEMU monitor can dump the guest's
framebuffer to an image file with `screendump build/screen.ppm`; the check then
confirms the "42" glyphs are actually lit in that frame. Same principle as the
current `EIP` check — ask the emulator from outside, because the kernel has no
way to report on itself.
