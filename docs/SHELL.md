# The debug shell

`kernel/src/shell.rs` holds a prompt, a line editor and a table of fourteen
commands. `kmain` calls `shell::run` last, and that function never returns. This
document explains the command table, the argument the table now passes, the
line editor and each command. It also explains the port I/O module that three
drivers share, and the proof from outside the guest.

## What the shell is, and what it is not

The subject asks for a minimal debug shell. The shell here is a prompt, a line,
and a name with an optional argument. The kernel reads the name, finds it in a
table of fourteen commands, and calls one function with the rest of the line.

The shell is not a POSIX shell. It has one argument string and nothing more: no
quotes, no pipes, no redirection, no variables, no environment and no job
control. It also has no file system to name, because this kernel has none. A
command is a word, and at most one word after it.

The shell exists for one reason. Each subject adds machine state that deserves
inspection. kfs-2 added the descriptor table and the kernel stack. kfs-3 adds
paging, a frame allocator, two heaps and two panic severities. A print at boot
shows each one once, and then the text scrolls away. A command shows each one at
any moment. Four of the eight new commands do more than print: they allocate,
map, write and fault on purpose. The deep explanation of each subject lives in
[GDT.md](GDT.md), [STACK.md](STACK.md), [PAGING.md](PAGING.md),
[MEMORY.md](MEMORY.md) and [PANIC.md](PANIC.md).

## The command table

```rust
struct Cmd {
    name: &'static str,
    help: &'static str,
    run: fn(&str),
}

static CMDS: [Cmd; 14] = [
    Cmd { name: "mem", help: "physical memory, frames and both heaps", run: cmd_mem },
    Cmd { name: "space", help: "the kernel and user address space layout", run: cmd_space },
    Cmd { name: "pages", help: "every mapping in the page directory", run: cmd_pages },
    Cmd { name: "virt", help: "translate an address: virt [addr]", run: cmd_virt },
    Cmd { name: "alloc", help: "exercise kmalloc, vmalloc, kbrk and vbrk", run: cmd_alloc },
    Cmd { name: "user", help: "map, use and drop a user space page", run: cmd_user },
    Cmd { name: "fault", help: "make a page fault: fault [demand|ro|kernel]", run: cmd_fault },
    Cmd { name: "panic", help: "panic on purpose: panic [oops|fatal]", run: cmd_panic },
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

Every row has the same shape. The `run` field takes one `&str` and returns
nothing, so the table needs no generic type and no trait object. A command that
wants no argument names the parameter `_`. A new command costs one row and one
function.

Two other tables in this kernel make the same choice:

- The keyboard layout tables `LOWER` and `UPPER` in `kernel/src/keyboard.rs`.
  Each table maps a scancode to a byte, so the US QWERTY layout is data and not
  a chain of branches.
- The colour palette `Color` in `kernel/src/vga.rs`. The enum holds the 16
  hardware values, and the attribute byte comes from arithmetic on them.

## How a line becomes a call

`dispatch` trims the line, then splits it once at the first space. The name is
the part before the space, and the argument is the rest, trimmed again:

```rust
fn dispatch(line: &str) {
    let line = line.trim();
    let (name, args) = match line.find(' ') {
        Some(at) => (&line[..at], line[at + 1..].trim()),
        None => (line, ""),
    };
    if name.is_empty() {
        return;
    }
    match CMDS.iter().find(|c| c.name == name) {
        Some(c) => (c.run)(args),
        None => printk!("{}: no such command, try `help`\n", name),
    }
}
```

Three rules follow from that shape:

- **An empty line does nothing.** The trim leaves an empty name, and `dispatch`
  returns before it searches the table. Enter on an empty prompt costs one new
  prompt and no message.
- **A line with no space passes the empty string.** `fault` and `fault demand`
  therefore reach the same function, and `cmd_fault` handles `""` and `"demand"`
  in one arm. Every command with an argument has a default this way.
- **An unknown name reports itself.** The message names the word that failed and
  not the whole line:

```
frobnicate: no such command, try `help`
```

Only `virt`, `fault` and `panic` read the argument. The other eleven ignore it.
Nothing splits further, so `virt 0xc0101000 extra` hands the whole string
`0xc0101000 extra` to `klib::parse_u32`, and that function refuses any byte that
is not a digit of its radix.

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
editor then calls `core::hint::spin_loop` and asks again. The loop is still a
poll. kfs-3 installs an interrupt descriptor table, but only to catch faults:
`sti` appears nowhere, the interrupt controller keeps the mapping the firmware
left, and no device interrupt ever reaches the CPU. [PANIC.md](PANIC.md)
explains why that table belongs to the panic work. A keyboard driven by its own
interrupt belongs to kfs-4.

The two layout tables only produce bytes below 0x80. The collected line is
therefore valid UTF-8, and `read_line` returns it through
`core::str::from_utf8_unchecked`.

## The commands

The order below is the order of `CMDS`, which is also the order that `help`
prints. Two of the new commands can end the session, in three argument forms:
`fault ro`, `fault kernel` and `panic fatal` each print a report and then stop
the processor with `cli; hlt`. `halt` still does the same on request. Every
other command comes back to the prompt, and that includes the two recovered
cases, `fault demand` and `panic oops`. A stopped processor needs a reset from
outside the guest, and `tools/check.sh` uses the monitor command `system_reset`
for it.

### `mem`

`cmd_mem` prints the whole memory stack in one screen: the RAM that the
multiboot map declares with one line per usable region, the frame counters of
the bitmap, the kernel image as virtual and physical bounds, the physical block
carved for `kmalloc`, and one report per heap with base, break, ceiling, bytes
used, bytes free, live blocks and backing. It answers the first question after
any allocation, which is how much is left and where it came from. The
field-by-field reading is in [MEMORY.md](MEMORY.md).

### `space`

`cmd_space` prints `mem::LAYOUT`, one row per region of the address space, with
its bounds, its owner and what it holds. The subject asks the kernel to define
kernel space and user space, and this table of eight regions is that definition
in a form the operator can read. Both the table and `mem::space_of`, the
function that answers kernel or user for a single address, are built from the
same constants in `kernel/src/mem.rs`. [PAGING.md](PAGING.md) explains the
split and why the low 4 MB stays with the kernel.

### `pages`

`cmd_pages` calls `paging::dump`. That function walks all 1024 directory
entries, walks every present table under them, and merges each run of pages that
is contiguous in both the virtual and the physical direction and carries the
same rights. Seven rows then describe the whole machine at boot. It exists
because a page table is invisible otherwise: no banner at boot can prove that
the read-only range is really read only, and this dump names the range and the
rights together. [PAGING.md](PAGING.md) reads the dump row by row.

### `virt`

`cmd_virt` is the one command with two forms. With no argument it prints one
summary row for each of seven addresses, chosen so that between them they touch
the descriptor table, the VGA buffer, the kernel image, both heaps and the
page-table window. With an argument it parses the text through
`klib::parse_u32`, which reads hex after `0x` and decimal otherwise, and then
performs the full walk: directory index, table index, offset, both entries with
every flag spelled out, and the physical address. A byte that is not a digit of
its radix makes the command refuse the argument and repeat the example. The
command turns the arithmetic of 10, 10 and 12 bits into something the operator
can run on any address. [PAGING.md](PAGING.md) walks the same output.

### `alloc`

`cmd_alloc` is a self test and not a report. It calls `kmalloc` twice, checks
that `ksize` is at least the request, writes a byte pattern over every byte and
reads it back, translates two pages of the big block to prove that block is
physically contiguous, frees both and watches the used counter fall, then does
the same for `vmalloc` and translates its three frames to show that they are
scattered. It ends with `kbrk` and `vbrk` in both directions. The last line is
the verdict, and the measured run prints `alloc: 14 checks passed`. A check that
fails prints `alloc: FAILED` with a reason and returns to the prompt.
[MEMORY.md](MEMORY.md) explains what each check defends.

### `user`

`cmd_user` exercises memory rights in user space, on a page it creates and
destroys. It maps one frame at `0x00c00000` with the user and write bits,
describes both entries, writes `0x42424242` and reads it back, drops the write
right and prints the new table entry, asks for a user page inside kernel space
and gets a recovered oops instead of a mapping, then unmaps the page and shows
that translation finds nothing there. Memory rights are a claim about hardware,
and this command makes the hardware answer it. [PAGING.md](PAGING.md) covers the
entry bits, and [PANIC.md](PANIC.md) covers the refusal.

### `fault`

`cmd_fault` makes the CPU fault on purpose, and its argument names the kind.
`fault` and `fault demand` read `0x00800000` in the user demand zone: the page
is absent, the handler takes a frame and maps it, the instruction runs again,
and the read returns zero after one yellow oops line. `fault ro` writes to
`0xc0101000`, the kernel's own code, which is present and read only.
`fault kernel` writes to `0xd1800000`, kernel space with no page. Those two are
fatal and stop the processor. Any other word prints
`fault: say demand, ro or kernel`. The three cases together are the proof that
the handler separates a fault it can repair from one it cannot.
[PANIC.md](PANIC.md) reads each report field by field.

### `panic`

`cmd_panic` reaches the two panic macros with no CPU fault involved. `panic` and
`panic oops` call `koops!`, which prints one yellow line, counts it and returns,
so the prompt comes back. `panic fatal` calls `kpanic!`, which prints in red,
dumps CR0, CR2, CR3, the count of recovered panics and a 64-byte window of the
live stack, and then halts the processor. Any other word prints
`panic: say oops or fatal`. The subject asks for the difference between a panic
the kernel walks away from and one it does not, and this command shows both
without a memory bug anywhere. [PANIC.md](PANIC.md) is the whole story, and
[STACK.md](STACK.md) covers the stack window in the fatal report.

### `stack`

`cmd_stack` calls `stack::print(stack::esp(), stack::WINDOW)`. `WINDOW` is 256
bytes. The dump starts at the live stack pointer. It stops at the top of the
kernel stack or after 256 bytes, whichever comes first, so a shallow stack gives
a short dump.

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
```

The header names the stack pointer, the top of the kernel stack, the bytes in
use and the size of the whole stack. The stack grows down, so the part in use is
the range from the stack pointer up to the top. Both bounds moved into the
higher half in kfs-3. The region is `0xc0110000..0xc0114000` now, and
`boot/boot.asm` still reserves the same 16384 bytes for it.

A row is 77 columns:

- 8 columns for the address of the first byte in the row, then two spaces.
- 16 bytes, each one as `xx ` in memory order, with one extra space after the
  eighth byte.
- The printable characters between two `|` marks. A byte outside 0x20 to 0x7e
  prints as a full stop.

Bytes appear in memory order, and x86 is little-endian. The address 0xc01036a1
therefore appears as `a1 36 10 c0`.

Four parts of this dump have a name. The first four bytes hold 0xc010d560, which
is inside `.data` and not inside `.text`, so it is a saved register and not a
return address; the fatal reports in [PANIC.md](PANIC.md) show the same value in
`ebx`. The bytes `73 74 61 63 6b 00` are the shell line buffer, which still
holds the word `stack`. The bytes `34 32` are the text `42` that `kmain` left on
the stack. The last four dwords below `stack_top` are what `boot/boot.asm` and
the prologue of `kmain` pushed at the very first call. [STACK.md](STACK.md)
names each one, covers the copy, the alias defect and the absent backtrace, and
describes the second caller of the printer: the fatal panic path prints a
64-byte window of its own.

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
   interrupts with base 0 and limit 0. kfs-3 installed a real table there
   ([PANIC.md](PANIC.md)), so this step throws it away on purpose. It then runs
   `int3`. The CPU cannot find the breakpoint handler, cannot find the
   double-fault handler either, and the third fault stops the processor. Every
   chipset wires that stop to a reset. Linux reboots the same way.

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
  mem     physical memory, frames and both heaps
  space   the kernel and user address space layout
  pages   every mapping in the page directory
  virt    translate an address: virt [addr]
  alloc   exercise kmalloc, vmalloc, kbrk and vbrk
  user    map, use and drop a user space page
  fault   make a page fault: fault [demand|ro|kernel]
  panic   panic on purpose: panic [oops|fatal]
  stack   hex dump of the live kernel stack
  gdt     the descriptor table, read back from the CPU
  clear   blank the screen
  reboot  restart the machine through the 8042
  halt    stop this processor for good
  help    this list
```

## Port input and output

x86 has two address spaces. A memory-mapped device answers at an ordinary
address, and the VGA text buffer is one. Its frame is still physical 0xb8000,
and the kernel now writes it through the kernel-space alias 0xc00b8000. The
kernel window maps the first 4 MB of RAM at 0xc0000000, so that address and the
identity address 0x000b8000 reach the same frame ([VGA.md](VGA.md)). The rest of
the devices answer in a separate space of 65 536 slots, and only the `in` and
`out` instructions reach that space.

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
form, so `type_line` splits the line into characters and sends one `sendkey` per
character, and then one `sendkey ret`. kfs-3 added one translation to that loop.
Commands take arguments now, and the monitor calls the space bar `spc`, so
`type_line` rewrites every space as `_` while it splits the line, and sends
`sendkey spc` for that placeholder. Without the translation no assertion could
type `virt 0xc0100000` or `fault ro`.

The typed line is verified before it runs. After the last character,
`type_line` reads the echoed line back out of the text buffer. When the line on
screen is not the line asked, the script erases it with `sendkey backspace` and
types it again, up to three times, and only then sends `sendkey ret`. A loaded
host can lose a key on the way to the monitor, and a lost key turns one command
into a different one.

The injected key reaches the queue of the emulated 8042. The status bit on port
0x64 rises from queue occupancy alone, so the polled driver reads the key with no
interrupt. This kernel enables no interrupt, and the input path still works.

The script reads the result out of the VGA text buffer. The monitor command
`xp/2000hx 0xb8000` dumps 2000 cells as halfwords. The low byte of a cell is the
character, and the high byte is the colour. The script decodes the low byte and
rebuilds 25 rows of 80 characters. An assertion then compares exact text.

That dump is the one long reply the script asks for, and it gets two guards.
The script holds the monitor connection open until the reply goes quiet,
because hanging up on the monitor in the middle of a reply silences it for the
rest of the run. And a dump that does not hold all 2000 cells is asked again,
up to three times.

The previous check compared pixel colours in a screen dump. It could prove that
white glyphs exist and that the grey text of GRUB is gone, and it could not read
one word. [VGA.md](VGA.md) describes that older method. The text decode replaces
it, and every shell assertion depends on the replacement.

The monitor echoes each character that the script types. A command therefore
appears in its own reply, and a naive reader of the reply treats the address in
the command as data. The helper `dumped` accepts hex only from a line that starts
with an address and a colon.

## The shell assertions

`tools/check.sh` holds 36 assertions and all of them pass. Nineteen of them are
driven by injected keystrokes, so the whole input path runs before the assertion
can look at the screen at all: `help`, `mem`, `space`, `pages`,
`virt 0xc0100000`, `alloc` (two assertions), `user`, `fault demand`,
`panic oops`, `stack` (four assertions), `reboot`, `fault ro`, `fault kernel`,
`panic fatal` and `halt`. The other seventeen read registers, physical memory
and the screen after boot, and need no keystroke.

These are the assertions about the shell itself:

- `OK: shell prompt is on screen`. This rules out a kernel that reaches `kmain`
  and never reaches `shell::run`, and a shell that starts and prints nothing. It
  is the one shell assertion that types nothing.
- ``OK: `help` lists the commands``. This rules out a broken input path. The key
  reached the 8042, the polled driver read it, the editor stored it, Enter ended
  the line, and `dispatch` found the row. The script greps the help text of
  `mem`, so the text also comes from `CMDS`.
- ``OK: `virt` walks directory 768, table 256 down to physical 0x00100000``.
  This is the assertion that proves arguments work end to end. The script types
  `virt 0xc0100000`, so the space arrived as `spc`, `dispatch` split the line at
  it, `klib::parse_u32` read the hex, and the walk printed the two indices, the
  offset and the physical address.
- ``OK: `panic oops` prints and returns, this panic is not fatal``. The script
  greps the message, then greps `kfs>` again. A prompt after a panic is the
  proof that a recovered panic really comes back to the shell.
- ``OK: `reboot` restarted the machine, the table and paging came back``. This
  rules out a reboot that hangs the machine. It also proves that `gdt::install`
  and the paging setup converge on the same state at every boot.
- ``OK: `halt` stopped the processor (HLT=1)``. The flag comes from
  `info registers`, so a busy loop that only looks stopped fails here.
- The three fatal cases end with the same shape: ``OK: a write to read only
  kernel code panics and stops the processor``, ``OK: a write to unmapped kernel
  space panics and stops the processor`` and ``OK: `panic fatal` prints, dumps
  the machine and stops the processor``. Each one types its command, greps the
  report, and then requires `HLT=1`. A stopped processor cannot type the next
  command, so the script sends the monitor command `system_reset` after each of
  the three and waits for `42` and the prompt before it goes on.

Two commands carry no assertion of their own. `clear` is typed eight times, as
the screen-clearer before the next case, and every assertion after one of those
would fail on a `clear` that took the shell down with the text. `gdt` no longer
has one either: the check reads GDTR and the seven descriptors at physical 0x800
straight out of the machine, which proves the table without trusting the command
that prints it ([GDT.md](GDT.md)). The assertions for `stack` and for the eight
new commands live in the documents that own the subject:
[STACK.md](STACK.md), [PAGING.md](PAGING.md), [MEMORY.md](MEMORY.md) and
[PANIC.md](PANIC.md).
