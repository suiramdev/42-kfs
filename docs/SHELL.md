# The debug shell

`kernel/src/shell.rs` holds a prompt, a line editor and a table of six commands.
`kmain` calls `shell::run` last, and that function never returns. This document
explains the command table, the line editor and each command. It also explains
the port I/O module that three drivers share, and the proof from outside the
guest.

## What the shell is, and what it is not

The subject asks for a minimal debug shell. The shell here is a prompt, a line,
and a name. The kernel reads the name, finds it in a table of six commands, and
calls one function.

The shell is not a POSIX shell. It has no arguments, no quotes, no pipes, no
redirection, no variables, no environment and no job control. It also has no
file system to name, because this kernel has none. A command is a bare word, and
nothing else.

The shell exists for one reason. kfs-2 adds two things that deserve inspection:
the descriptor table and a dump of the kernel stack. A print at boot shows each
one once, and then the text scrolls away. A command shows each one at any
moment. The deep explanation of the two subjects lives in
[GDT.md](GDT.md) and [STACK.md](STACK.md).

## The command table

```rust
struct Cmd {
    name: &'static str,
    help: &'static str,
    run: fn(),
}

static CMDS: [Cmd; 6] = [
    Cmd { name: "stack", help: "hex dump of the live kernel stack", run: cmd_stack },
    Cmd { name: "gdt", help: "the descriptor table, read back from the CPU", run: cmd_gdt },
    Cmd { name: "clear", help: "blank the screen", run: cmd_clear },
    Cmd { name: "reboot", help: "restart the machine through the 8042", run: cmd_reboot },
    Cmd { name: "halt", help: "stop this processor for good", run: cmd_halt },
    Cmd { name: "help", help: "this list", run: cmd_help },
];
```

`CMDS` is a table, not a `match`. The reason is the `help` command. `dispatch`
searches `CMDS` for the name, and `cmd_help` prints every row of `CMDS`. Both
read the same array. A command therefore cannot exist without a help line. A
help line cannot describe a command that nobody can run.

A `match` puts the two jobs in two places. The second place then drifts from the
first, and the help text becomes a lie. The table removes that failure from the
design.

Every row has the same shape. The `run` field accepts no argument and returns
nothing, so the table needs no generic type and no trait object. A new command
costs one row and one function.

Two other tables in this kernel make the same choice:

- The keyboard layout tables `LOWER` and `UPPER` in `kernel/src/keyboard.rs`.
  Each table maps a scancode to a byte, so the US QWERTY layout is data and not
  a chain of branches.
- The colour palette `Color` in `kernel/src/vga.rs`. The enum holds the 16
  hardware values, and the attribute byte comes from arithmetic on them.

`dispatch` trims the line first. It does nothing with an empty line. It prints
one message for an unknown name:

```
frobnicate: no such command, try `help`
```

## The line editor

`read_line` collects keys until Enter. It fills a buffer of 74 bytes, the value
of `LINE_MAX`. The prompt `kfs> ` is 5 columns wide, so a full line plus the
prompt is 79 columns and fits one 80-column row.

The buffer is a local array in `run`, so it lives on the kernel stack. It keeps
the last command until the next line overwrites it. The `stack` command shows
that buffer in its own dump, as the text between the `|` marks.

The editor has five rules:

- **Enter ends the line.** The editor echoes a new line and returns the bytes as
  a `&str`. The shell needs a boundary between the line and the command.
- **A printable key that fits goes into the buffer, and the screen shows it.**
  The operator then sees the exact bytes that `dispatch` receives.
- **The editor drops a printable key that does not fit, and echoes nothing.** An
  echo without a store shows a character that the command never receives. The
  buffer and the screen must always agree.
- **Backspace at column zero does nothing.** An erase there deletes a character
  of the prompt, which the buffer never held. This rule protects the prompt.
- **F1, F2 and F3 switch the virtual screen, even in the middle of a line.** The
  editor forwards these keys to `vga::switch_screen` and stores nothing, so the
  line survives the switch.

`keyboard::poll` returns `None` for a key release, a shift key or a dead key. The
editor then calls `core::hint::spin_loop` and asks again. The loop is a poll,
because this kernel has no interrupt descriptor table yet.

The two layout tables only produce bytes below 0x80. The collected line is
therefore valid UTF-8, and `read_line` returns it through
`core::str::from_utf8_unchecked`.

## The commands

### `stack`

`cmd_stack` calls `stack::print(stack::esp(), stack::WINDOW)`. `WINDOW` is 256
bytes. The dump starts at the live stack pointer. It stops at the top of the
kernel stack or after 256 bytes, whichever comes first, so a shallow stack gives
a short dump.

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
```

The header names the stack pointer, the top of the kernel stack, the bytes in
use and the size of the whole stack. The stack grows down, so the part in use is
the range from the stack pointer up to the top.

A row is 77 columns:

- 8 columns for the address of the first byte in the row, then two spaces.
- 16 bytes, each one as `xx ` in memory order, with one extra space after the
  eighth byte.
- The printable characters between two `|` marks. A byte outside 0x20 to 0x7e
  prints as a full stop.

Bytes appear in memory order, and x86 is little-endian. The saved address
0x0010063c therefore appears as `3c 06 10 00`.

Three parts of this dump have a name. The first four bytes are the return
address into `dispatch`, which the call to `cmd_stack` pushed. The bytes
`73 74 61 63 6b 00` are the shell line buffer, which still holds the word
`stack`. The bytes `34 32` are the text `42` that `kmain` left on the stack.
[STACK.md](STACK.md) covers the copy, the overlap defect and the absent
backtrace.

### `gdt`

```
kfs> gdt
gdtr: base 0x00000800 limit 0x0037 7 entries, as declared
 sel  segment       range                   ring type     in use
 0x00 null          -                      -    -
 0x08 kernel code   0x00000000..0xffffffff 0    code rx  cs
 0x10 kernel data   0x00000000..0xffffffff 0    data rw  ds
 0x18 kernel stack  0x00000000..0xffffffff 0    data rw  ss
 0x23 user code     0x00000000..0xffffffff 3    code rx
 0x2b user data     0x00000000..0xffffffff 3    data rw
 0x33 user stack    0x00000000..0xffffffff 3    data rw
```

The header comes from the CPU. `gdt::current` runs `sgdt`, which reads the table
register back into memory. The command then compares two numbers: the base
against `gdt::BASE`, which is 0x00000800, and the limit against `gdt::LIMIT`,
which is 0x0037. Both `install` and this command read that one constant, so the
two cannot disagree about the size of the table.

Both numbers agree here, so the header ends with `as declared`. A mismatch
prints `DOES NOT MATCH THE SOURCE`. That verdict means that the CPU uses a table
that the source does not describe. Three faults produce it: `install` never ran,
the copy went to another address, or other code loaded the table register after
`install`. The rows below the header then describe a table that the CPU ignores.

The columns are these:

- `sel` is the selector, the value that a segment register holds.
- `segment` is the name from the `Segment` row in `kernel/src/gdt.rs`.
- `range` is the first and the last linear address that the selector reaches.
- `ring` is bits 46 and 45 of the descriptor, the privilege level.
- `type` is the decode of the access byte by `seg_type`.
- `in use` names each of `cs`, `ss` and `ds` that holds this selector now.

The `in use` test ignores the low two bits of both values. Those bits carry the
requested ring, and a match on the descriptor index is the useful answer.

`seg_range` prints the real range, not the raw limit field. It rebuilds the base
from bits 39 to 16 and bits 63 to 56. It rebuilds the limit from bits 15 to 0 and
bits 51 to 48. The granularity flag multiplies that limit by the 4 KiB page
size, and the last page counts in full. The expression `raw << 12 | 0xfff`
therefore gives 0xffffffff here.

A null descriptor reaches nothing, and the row shows `-`.

An expand-down data descriptor also shows as empty. Intel inverts the limit for
such a descriptor, and valid offsets start at limit + 1. A flat expand-down
segment has a limit of 0xffffffff, so the first valid offset does not exist and
the segment reaches no address at all. `seg_range` returns `None` for it. A
report of the raw limit would show a range of 4 GiB for a segment that no
instruction can touch. This kernel declares no expand-down descriptor, and the
decoder still gives the correct answer for one.

`seg_type` reads three bits. Bit 43 splits code from data. Bit 41 grants the
second right of each kind, which is read on code and write on data. Bit 42 means
conforming on code and expand-down on data.

### `clear`

`cmd_clear` calls `vga::clear`. That function fills all 2000 cells with a space
in the current colour and parks the cursor at the top left. The shell then prints
a new prompt, so the screen holds one line again. [VGA.md](VGA.md) describes the
driver.

### `reboot`

`cmd_reboot` prints `rebooting` and then tries two mechanisms in order:

1. **The 8042 keyboard controller.** The code repeats one pair of steps ten
   times. It waits until bit 1 of the status byte on port 0x64 is clear. It then
   writes 0xfe to port 0x64. Bit 1 marks a full input buffer, and the
   controller drops a byte that arrives into a full buffer. Command 0xfe pulls
   the CPU reset line low, because the 8042 drives that line on a PC.
2. **A deliberate triple fault.** The code loads the table register for
   interrupts with base 0 and limit 0. It then runs `int3`. The CPU cannot find
   the breakpoint handler, cannot find the double-fault handler either, and the
   third fault stops the processor. Every chipset wires that stop to a reset.
   Linux reboots the same way.

If both mechanisms fail, `cmd_reboot` calls `cmd_halt`, and the machine stops in
a state that the operator can see.

### `halt`

`cmd_halt` prints `halted`, runs `cli`, and then runs `hlt` in a loop.

`cli` clears the interrupt flag, so no external interrupt can wake the CPU. `hlt`
stops the CPU until the next interrupt, and a single `hlt` can therefore resume.
The firmware can still raise an interrupt. The loop answers that case, and
`_start` in `boot/boot.asm` ends the same way.

The inline assembly for `cli` cannot promise `preserves_flags`, because `cli`
writes the interrupt flag. The `hlt` block keeps that promise.

### `help`

`cmd_help` walks `CMDS` and prints the name in six columns, then the help text.
The order on screen is the order in the table.

```
kfs> help
  stack   hex dump of the live kernel stack
  gdt     the descriptor table, read back from the CPU
  clear   blank the screen
  reboot  restart the machine through the 8042
  halt    stop this processor for good
  help    this list
```

## Port input and output

x86 has two address spaces. A memory-mapped device answers at an ordinary
address, and the VGA text buffer at 0xb8000 is one. The rest of the devices
answer in a separate space of 65 536 slots, and only the `in` and `out`
instructions reach that space.

Three modules need those two instructions:

- `kernel/src/vga.rs` writes the CRT controller registers on ports 0x3D4 and
  0x3D5, which hold the hardware cursor position.
- `kernel/src/keyboard.rs` reads the 8042 status byte on port 0x64 and the
  scancode on port 0x60.
- `kernel/src/shell.rs` writes the reset command on port 0x64.

`kernel/src/port.rs` now owns `inb` and `outb`, and the three modules call it.
`vga.rs` and `keyboard.rs` each held a private copy of the wrapper before this,
and both gave the copy up. One instruction with two private wrappers in one
kernel has one wrapper too many.

Both functions are `unsafe`, and the safety condition is the side effect and not
the instruction. A read of port 0x60 consumes a scancode, and the byte is gone
afterwards. A write of 0xfe to port 0x64 resets the machine. Neither `in` nor
`out` can fault in ring 0.

## How the check drives the shell from outside the guest

`tools/check.sh` runs the whole proof through the QEMU human monitor on a unix
socket. Nothing inside the guest takes part, so a kernel that only claims to work
cannot pass. `make check` runs the script.

The script types with the monitor command `sendkey`. `sendkey` has no string
form, so `type_line` splits the word into characters and sends one `sendkey` per
character, and then one `sendkey ret`.

The injected key reaches the queue of the emulated 8042. The status bit on port
0x64 rises from queue occupancy alone, so the polled driver reads the key with no
interrupt. This kernel enables no interrupt, and the input path still works.

The script reads the result out of the VGA text buffer. The monitor command
`xp/2000hx 0xb8000` dumps 2000 cells as halfwords. The low byte of a cell is the
character, and the high byte is the colour. The script decodes the low byte and
rebuilds 25 rows of 80 characters. An assertion then compares exact text.

The previous check compared pixel colours in a screen dump. It could prove that
white glyphs exist and that the grey text of GRUB is gone, and it could not read
one word. [VGA.md](VGA.md) describes that older method. The text decode replaces
it, and every shell assertion depends on the replacement.

The monitor echoes each character that the script types. A command therefore
appears in its own reply, and a naive reader of the reply treats the address in
the command as data. The helper `dumped` accepts hex only from a line that starts
with an address and a colon.

## The shell assertions

`tools/check.sh` holds 20 assertions and all of them pass. These come from the
shell:

- `OK: shell prompt is on screen`. This rules out a kernel that reaches `kmain`
  and never reaches `shell::run`, and a shell that starts and prints nothing.
- ``OK: `help` lists the commands``. This rules out a broken input path. The key
  reached the 8042, the polled driver read it, the editor stored it, Enter ended
  the line, and `dispatch` found the row. The script greps the help text of
  `stack`, so the text also comes from `CMDS`.
- ``OK: `gdt` reads the table back from the CPU and agrees with the source``. The
  script greps `as declared`, which rules out a table that the source declares
  and the CPU does not use. It also greps `kernel stack`, which rules out a table
  without the kernel stack descriptor at selector 0x18.
- ``OK: `clear` blanked the screen``. The script fails when the screen holds no
  character at all. A screen with no prompt means that `clear` ran and the shell
  died. This assertion rules out a `clear` that takes the shell down with the
  text.
- `OK: header is self-consistent: 168 of 16384 bytes below 0x00107710`. The size
  matches the 16384 bytes that `boot/boot.asm` reserves. The stack pointer is
  below the top, and the difference equals the count in the header.
- `OK: 168 bytes rendered as 11 rows of 16`. No byte of the window is absent from
  the output.
- `OK: first row starts at 0x00107668, the address the header names`. The address
  column holds a real address and not an offset.
- `OK: the first dword is 0x0010063c, a return address inside the kernel`. This
  rules out the overlap defect. A copy buffer inside the window makes the dump
  periodic and puts a stack address in this position.
- `OK: the ASCII column shows the typed command in the line buffer`. The dump
  covers the frame of `run`, and a human reader recognises the content.
- `OK: dump esp 0x00107668 is 4 bytes deeper than ESP=0x0010766c`. The check ties
  the number in the dump to the register that the CPU reports. x86 stacks grow
  down, so the captured value must sit below the live one.
- ``OK: `reboot` restarted the machine, and the table came back at 0x00000800``.
  This rules out a reboot that hangs the machine. It also proves that
  `gdt::install` converges on the same state at every boot.
- ``OK: `halt` stopped the processor (HLT=1)``. The flag comes from
  `info registers`, so a busy loop that only looks stopped fails here.
