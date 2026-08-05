# VGA text output — deferred

Status: NOT implemented in the initial bring-up. `kmain` currently halts in a
spin loop. The subject's mandatory "Display 42 on the screen" is delivered by
a follow-up task using the design below.

## Planned implementation

New module `kernel/src/vga.rs`, declared as `mod vga;` in `kernel/src/lib.rs`.

- Hardware: VGA text mode buffer at physical `0xb8000`, 80×25 cells, one
  `u16` per cell: low byte = ASCII code point, high byte = attribute
  (low nibble foreground, high nibble background). White on black = `0x0F`.
- Access: `0xb8000 as *mut u16`, all writes through
  `core::ptr::write_volatile` (the buffer is MMIO — the compiler must not
  elide or reorder writes). No allocation, no `core::fmt`.
- API (first iteration):
  - `pub fn clear()` — fill all 80*25 cells with `0x0F20` (white-on-black space).
  - `pub fn print(s: &str)` — for each byte `b` of `s`, write
    `0x0F00 | b as u16` starting at offset 0, incrementing a cell index.
    Non-ASCII bytes are written as-is (code page 437 glyphs); no wrapping
    or scrolling needed while output fits one line.
- `kmain` change: replace the bare spin loop body with
  `vga::clear(); vga::print("42");` before the `loop {}`.

## Later extensions (bonus part, not this design's scope)

Color parameter on `print`, cursor tracking + scrolling (move rows up with
`copy` when past row 24), hardware cursor via ports `0x3D4`/`0x3D5`, and a
`printk`-style formatter implementing `core::fmt::Write`.

## Verification when implemented

`make check` regains a screen assertion: qemu monitor `screendump
build/screen.ppm`, then confirm the "42" glyphs in the frame.
