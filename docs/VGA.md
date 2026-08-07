# Screen output

The subject requires the kernel to display "42". It does — white on black, with
a bright-green "kfs-1" underneath — and the driver behind it covers all of the
subject's bonuses: colours, a tracked cursor with line wrapping, scrolling, the
`printk!` formatter built on top ([ARCHITECTURE.md](ARCHITECTURE.md) covers that
one), a polled PS/2 keyboard that echoes what you type, and three virtual
screens on F1/F2/F3. This file explains what "the screen" actually is on this machine,
how `kernel/src/vga.rs` drives it, and how the result is verified.

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

## The driver

`kernel/src/vga.rs`, declared with `mod vga;` in `kernel/src/lib.rs`.

- **Access:** `0xb8000 as *mut u16`, every access through
  `core::ptr::write_volatile` / `read_volatile`. `volatile` tells the compiler
  it may not delete, merge or reorder these accesses. Without it the optimiser
  is entitled to throw away a store whose result is never read back — which is
  every store here, because the *act* of writing is the whole point. Touching a
  raw pointer needs `unsafe`; that keyword does not switch off any checks, it
  just moves the burden of proof from the compiler to us. The one condition
  that makes the pointer arithmetic sound — the cell index stays below 80x25 —
  is asserted in the two helpers every access funnels through.
- **State:** two `static mut`s — the next cell to write and the current
  colour. Plain mutable statics are a bug in most programs; here nothing is
  concurrent (one CPU, interrupts never enabled, every call rooted in `kmain`'s
  single call chain), and the module keeps them private behind accessor
  functions.
- **No allocation.** There is no heap; even `printk!`'s formatting runs
  allocation-free, `format_args!` feeding `write_str` calls straight into the
  driver.
- **API:**
  - `pub fn clear()` — fill all 80x25 cells with a space in the current
    colour, cursor back to the top-left.
  - `pub fn set_color(fg: Color, bg: Color)` — colour for everything printed
    from now on. `Color` is the 16-entry VGA palette as a `#[repr(u8)]` enum;
    the attribute byte is `bg << 4 | fg`.
  - `pub fn print(s: &str)` — write at the cursor: `\n` starts a new line,
    other bytes are written as-is (non-ASCII renders as code page 437 glyphs)
    and wrap at column 80. Output past the bottom row scrolls everything up
    one line: rows 1-24 are copied over rows 0-23 — whole u16s, so each
    character keeps its colour — and the bottom row is blanked.
- **`Writer`** — a unit struct implementing `core::fmt::Write` over `print`,
  the three-line hook that gives the `printk!` macro (and panics) the whole
  formatting engine. The macro itself lives in `kernel/src/printk.rs`.
- **`kmain`:** clear, print "42" — computed through `klib::utoa(42, &mut buf)`
  rather than written as a literal, so the kernel library is exercised on the
  mandatory path — then `printk!("kfs-{}\n", 1)` in bright green. After the
  banner it loops polling the keyboard: `put_char` echoes, `backspace()` steps
  the cursor back and blanks the cell, F1-F3 call `switch_screen`.

## Virtual screens

There is one real screen — the 4 000 bytes at `0xb8000` — and three virtual
ones. The active screen lives in the real buffer and *nowhere else*; each
inactive screen is a save slot in `.bss`: the same 2 000 cells plus the cursor
and colour that go with them. `switch_screen(n)` copies the live buffer into
the leaving screen's slot and the entering slot into the buffer, so contents,
colours and cursor position all survive a round trip untouched.

One deliberate detail: the slots are zero-initialised, with a `USED` flag
marking which ever held a real screen (first entry into a fresh screen blanks
it on the spot). Initialising the slots to "blank cells" in the source instead
would move all 12 KiB from `.bss` (free: the loader provides zeroed memory)
into `.data` (stored byte-for-byte in the file) — measured, that mistake cost
+12 KiB of `kernel.bin` for three screens full of spaces.

## The hardware cursor

The blinking underscore is not a character in the buffer: it is the CRT
controller marking a cell index it holds in two registers. Those registers live
in x86's *other* address space — **port I/O** — reached with the `in`/`out`
instructions rather than loads and stores. Write a register index to port
`0x3D4`, then the value to `0x3D5`; registers `0x0E`/`0x0F` are the cursor
position's high and low byte.

`print` and `clear` re-park the cursor after their last write, through a
two-instruction `outb` wrapper around inline `asm!` — the kernel's first and
so far only assembly outside `boot/boot.asm`. Before this, the cursor blinked
wherever GRUB abandoned it; now it tracks the end of our output, which is what
makes the screen read like a terminal.

## Proven behaviour

All frame-dumped over the QEMU monitor; the keyboard runs were driven with its
`sendkey` command, which injects real scancodes into the emulated 8042 —
keystrokes without fingers:

- 30 numbered lines on a 25-row screen leave lines 9-30 on screen: the top
  scrolled away, the bottom kept (throwaway `kmain`, 33 lines printed).
- A 200-character line folds at column 80 across three rows.
- The cursor sits exactly after the last character printed.
- Colours travel with their cells when the screen scrolls.
- Typed `hello World!` — Shift produced `W` and `!`, Enter opened a line,
  Backspace erased: `x` remained of `xy`.
- F2 revealed a fresh blank screen; text typed there stayed there; F1 brought
  back the first screen bit-for-bit, banner colours and cursor included.

## How it is verified

`make check` has a screen assertion next to the `EIP` one — same principle,
ask the emulator from outside, because the kernel has no way to report on
itself. The QEMU monitor dumps the guest's framebuffer with
`screendump build/screen.ppm` (re-dumping for a few seconds if the frame is
stale — qemu repaints its display surface on its own timer), and the check
requires two things of the final frame:

1. **Some pure-white pixels exist** — the "42" glyphs, drawn with attribute
   `0x0F`, render `#ffffff`.
2. **No `#a8a8a8` pixels remain** — that grey is the colour of GRUB's leftover
   boot text, so its absence proves `clear()` really overwrote the buffer.

The greps scan the raw P6 byte stream, which is sound only while the kernel
draws nothing that could counterfeit either triple. That is why the boot
screen sticks to white, black and the *bright* half of the palette (the green
of "kfs-1" renders `#54fc54`): the dim half — green `#00a800`, red `#a80000`,
cyan `#00a8a8`... — contains `a8` bytes that adjacent pixels could reassemble
into GRUB-grey, and light grey (colour 7) *is* `#a8a8a8`. The constraint is
restated in the Makefile next to the check.

A successful run prints:

```
OK: guest alive, EIP=001002fa inside kernel
OK: screen cleared and "42" glyphs lit
```

The other white thing in the frame is the hardware cursor — blinking at row 2,
column 0 because the kernel parked it there after printing its two lines. It
blinks in the attribute of the cell it sits on (white-on-black after
`clear()`), so it disturbs neither assertion.
