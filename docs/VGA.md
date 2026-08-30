# Screen output

The subject requires the kernel to display "42". It does — white on black, with
a bright-green "kfs-3" underneath — and the driver behind it covers all of the
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

### Why the kernel writes at 0xc00b8000

The hardware address has not moved. The display controller reads physical
`0xb8000` and knows nothing about page tables. Since kfs-3 the kernel is a
higher-half kernel: its code is loaded at physical `0x00101000` and runs at
virtual `0xc0101000`, and the kernel window maps the first 4 MB of physical
memory a second time at `0xc0000000`. Physical `0xb8000` therefore also answers
at `0xc0000000 + 0xb8000`, which is `0xc00b8000`. Both addresses name the same
4 000 bytes, and `kernel/src/vga.rs` uses the second one. The `pages` command
shows the two windows that make this true:

```
kfs> pages
 virtual                  physical      pages  rights
 0x00000000..0x003fffff 0x00000000   1024  rw kernel
 0xc0000000..0xc0100fff 0x00000000    257  rw kernel
```

The first row is the low identity window, so `0xb8000` would still work today.
The kernel prefers the alias because of what that window is for. It is
supervisor-only legacy space, kept alive because `lgdt` needs linear `0x800` to
reach the descriptor table ([GDT.md](GDT.md)) and because GRUB left the
multiboot information down there. Screen output is not legacy. It belongs in
kernel space with the rest of the kernel, and an address in kernel space
survives if the low window is ever dropped. [PAGING.md](PAGING.md) describes
both windows and the switch that created them.

Every store into the buffer is a write through a page table entry, and that
entry has its R/W bit set: `pages` prints `rw kernel` for the whole kernel
window. kfs-3 also sets bit 16 of CR0, write protect, so ring 0 obeys the
read-only bit like anyone else. If this mapping were read only, a `print` would
raise a page fault instead of dropping the store in silence, and
[PANIC.md](PANIC.md) shows the report that a refused kernel write produces.

## The driver

`kernel/src/vga.rs`, declared with `mod vga;` in `kernel/src/lib.rs`.

- **Access:** `0xc00b8000 as *mut u16`, the kernel-space alias above and the one
  line of the driver that kfs-3 changed. Every access goes through
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
- **No allocation.** The kernel has a heap since kfs-3
  ([MEMORY.md](MEMORY.md)), and the driver uses none of it. That is deliberate.
  The screen has to work before `mem::init` runs, because the banner and the
  exception-table line are printed before it, and the screen is the only way a
  panic can report itself. Even `printk!`'s formatting runs allocation-free,
  `format_args!` feeding `write_str` calls straight into the driver.
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
  mandatory path — then `printk!("kfs-{}\n", 3)` in bright green. `idt::install`
  and `mem::init` print their own lines under it, and `shell::run` takes the
  keyboard from there: `put_char` echoes, `backspace()` steps the cursor back
  and blanks the cell, F1-F3 call `switch_screen`. See [SHELL.md](SHELL.md).

## Virtual screens

There is one real screen — the 4 000 bytes at physical `0xb8000`, which the
kernel reaches at `0xc00b8000` — and three virtual ones. The active screen
lives in the real buffer and *nowhere else*; each inactive screen is a save
slot in `.bss`: the same 2 000 cells plus the cursor and colour that go with
them. `switch_screen(n)` copies the live buffer into the leaving screen's slot
and the entering slot into the buffer, so contents, colours and cursor position
all survive a round trip untouched.

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
two-instruction `outb` wrapper around inline `asm!`, which lives in
`kernel/src/port.rs` beside the keyboard's `inb`. It was the kernel's first
assembly outside `boot/boot.asm`, and it is no longer the only one: `gdt.rs`,
`idt.rs`, `paging.rs`, `stack.rs` and `panic.rs` all reach for `asm!` too.
Before this, the cursor blinked wherever GRUB abandoned it; now it tracks the
end of our output, which is what makes the screen read like a terminal.

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

`make check` has screen assertions next to the `EIP` one — same principle, ask
the emulator from outside, because the kernel has no way to report on itself.
The QEMU monitor dumps the text buffer with `xp/2000hx 0xb8000`, and the script
decodes it in `awk`: one 16-bit word per cell, the low byte is the character,
printable bytes come out as themselves and everything else as a space, a line
break every 80 cells. `screen` returns the 25 rows as text, and `row n` picks
one of them.

The address in that command is physical. `xp` reads guest physical memory and
walks no page tables, so `0xb8000` stays the right thing to ask for even though
the driver writes `0xc00b8000`. Nothing in this file needed changing when the
kernel moved to the alias: the dump reads the same 4 000 bytes it always read,
and every screen capture above is still what the monitor returns.

Three assertions rest directly on that decode:

```
OK: row 0 shows the mandatory "42"
OK: row 1 shows the kfs-3 banner
OK: shell prompt is on screen
```

`booted()` uses the same reader to decide the machine is up — it waits until
row 0 reads `42` and the prompt is somewhere on screen — and every later
assertion that quotes a command's output greps the decoded text. This driver is
the path that most of the 36 assertions in `tools/check.sh` read through.

What the decode cannot see is colour. `cut -c5-6` keeps the low byte of each
cell and throws the attribute byte away, so the bright green of the banner is
asserted as the text `kfs-3` and nothing more.

An earlier version of the check looked at pixels instead. It dumped the frame
with `screendump build/screen.ppm` and required pure-white bytes for the "42"
glyphs and no `#a8a8a8` anywhere, that grey being the colour of GRUB's leftover
boot text. It worked, and it cost the kernel its palette: the dim half of the
VGA colours contains `a8` bytes that adjacent pixels could reassemble into
GRUB-grey, so the boot screen had to stay on the bright half to keep the check
honest. Commit `fc95109` replaced it with the text dump, which compares strings
and can read a whole shell session. The palette constraint went with it.

The hardware cursor is invisible to this check for the same reason colour is:
it is not a character in the buffer, so it cannot appear in the decoded text.
