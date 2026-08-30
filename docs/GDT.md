# The descriptor table

The descriptor table gives the CPU one segment descriptor for every segment the
kernel uses. Each segment descriptor is 8 bytes long, and it carries a base, a
limit and an access byte. The subject demands six segments: the kernel code, the
kernel data, the kernel stack, the user code, the user data and the user stack.
The subject also fixes the address of the table at physical `0x00000800`.
`kernel/src/gdt.rs` builds the table, copies it to that address, loads it into
the table register, and reloads every segment register from it.

## Why the kernel needs its own table

GRUB installs a descriptor table before it hands control to the kernel. That
table belongs to GRUB. The multiboot specification states two facts about the
handoff:

- The table register may be invalid at entry.
- The kernel must not load any segment register, even with the same value, until
  it owns a table.

The second rule looks strange, because every segment register works at entry.
The reason is a hardware detail. Every segment register holds a hidden copy of
its descriptor. The CPU reads that hidden copy on each memory access, and it
reads the table in memory only at a load. GRUB's segments therefore stay valid
after GRUB's table is gone. The first load of a segment register is the moment
the table becomes real.

Install the table before the first load of a segment register. `kmain` calls
`gdt::install()` as its first act, before `vga::clear()` and before the banner.

## The segment descriptor

The CPU reads 8 bytes for each row of the table. The fields are not contiguous,
because the descriptor keeps the layout of the 16-bit ancestor of this format.
The byte order is:

| byte | contents |
| ---- | -------- |
| 0    | the limit, bits 7-0 |
| 1    | the limit, bits 15-8 |
| 2    | the base, bits 7-0 |
| 3    | the base, bits 15-8 |
| 4    | the base, bits 23-16 |
| 5    | the access byte |
| 6    | the flags, over the limit, bits 19-16 |
| 7    | the base, bits 31-24 |

The access byte holds the type and the ring. Its bits are:

| bit | name | meaning |
| --- | ---- | ------- |
| 7   | present | the descriptor is valid |
| 6-5 | ring | the privilege level of the segment |
| 4   | code or data | 1 for a code or data segment |
| 3   | executable | 1 for code, 0 for data |
| 2   | direction or conforming | expand-down for data, conforming for code |
| 1   | readable or writable | code is readable, data is writable |
| 0   | accessed | the CPU sets this bit |

The seven rows of the table are:

| selector | segment | access byte | meaning |
| -------- | ------- | ----------- | ------- |
| `0x00` | null | none | eight zero bytes |
| `0x08` | kernel code | `0x9a` | ring 0, code, readable |
| `0x10` | kernel data | `0x92` | ring 0, data, writable |
| `0x18` | kernel stack | `0x92` | ring 0, data, writable |
| `0x23` | user code | `0xfa` | ring 3, code, readable |
| `0x2b` | user data | `0xf2` | ring 3, data, writable |
| `0x33` | user stack | `0xf2` | ring 3, data, writable |

Row 0 must hold eight zero bytes. The CPU refuses a selector of 0, and that
refusal turns an empty segment register into a fault instead of a silent read.

A selector is the offset of its row in the table, plus the requested ring in the
low two bits. The kernel selectors ask for ring 0, so they are the bare offsets.
Each user selector carries a 3 in the low two bits. The offset of the user code
row is `0x20`, and `0x20 | 3` gives the selector `0x23`.

The flags byte is `0xcf` for all six segments. The high half, `0xc`, sets the
granularity flag and the 32-bit operand size. The granularity flag scales the
limit by 4 KiB, so a limit of `0xfffff` covers 4 GiB. The low half, `0xf`,
carries bits 19-16 of that limit.

`flat()` packs one descriptor as a single 64-bit value:

```rust
const fn flat(access: u8) -> u64 {
    0xffff | ((access as u64) << 40) | (0xcf << 48)
}
```

The base is 0, so three of the fields hold zero. The function writes only three
things. `0xffff` fills bytes 0 and 1 with the low half of the limit. The shift by
40 puts the access byte at byte 5. The shift by 48 puts `0xcf` at byte 6. The
kernel code descriptor is therefore `flat(0x9a)`:

```
0x00cf9a000000ffff

byte 7  0x00   base 31-24
byte 6  0xcf   flags 0xc, limit 19-16 = 0xf
byte 5  0x9a   the access byte
byte 4  0x00   base 23-16
byte 3  0x00   base 15-8
byte 2  0x00   base 7-0
byte 1  0xff   limit 15-8
byte 0  0xff   limit 7-0
```

The low 32 bits of the six segment descriptors are `0x0000ffff`. Only the high
32 bits differ, and they differ only in the access byte.

## Why all six segments are flat

All six segments have a base of `0x00000000` and a limit of `0xfffff` with the
granularity flag set, which is 4 GiB. They differ only in the access byte, which
carries the type and the ring. A flat segment maps every linear address to
itself, so segmentation adds nothing to an address. That is the correct choice
here, because paging now carries memory protection. Rights are a property of a
page in this kernel, not of a segment; see [PAGING.md](PAGING.md).

An x86 stack segment is a data segment. Intel requires a writable data
descriptor in `ss`. The kernel stack descriptor is therefore byte-identical to
the kernel data descriptor, and only the selector differs. The separate row
exists because the subject demands it. It also exists because `ss` and `ds`
become different descriptors as soon as a later project narrows one of them.

A narrow stack segment is expressible on x86. The direction bit of a data
descriptor selects expand-down, and an expand-down descriptor inverts the limit:
the valid range runs from above the limit up to the top of the segment. Two
reasons keep that form out of this kernel:

- A flat expand-down segment reaches no address at all. The limit of `0xfffff`
  with the granularity flag covers the whole space, so the inverted range is
  empty. `shell::seg_range` prints the real range, and it shows such a
  descriptor as empty.
- The code generator assumes that `ss` and `ds` share a base. An address that
  the kernel takes from the stack, and then reads back through `ds`, would
  resolve at a different place.

## Segmentation and paging together

Since kfs-3 an address goes through two translations, in this order:

1. **Segmentation.** The CPU takes the offset in the instruction and adds the
   base of the segment that the selector names. The result is a linear address.
2. **Paging.** The linear address goes through the page directory and a page
   table, and the result is a physical address.

The first step is the identity function in this kernel. Every base is
`0x00000000` and every limit covers 4 GiB, so the linear address equals the
offset, and no segment refuses an access that paging allows. That is why the
descriptors can stay flat. Everything the subject calls memory rights lives in
the R/W and U/S bits of a page entry, and [PAGING.md](PAGING.md) describes
them.

The order also explains why the user descriptors in the table still describe
nothing that runs. A ring 3 descriptor takes effect only when the CPU is in
ring 3, and nothing here enters ring 3: no far return to a user selector, no
task state segment. The privilege split that is real today is the U/S bit in
the page tables, and at rest every mapping that the `pages` command prints
reads `kernel`.

## The address 0x00000800

The table sits at physical `0x00000800`. Seven descriptors of 8 bytes each
occupy `0x00000800` to `0x00000837`, which is 56 bytes. The address is free
memory, and it is a multiple of 8, which is the alignment that Intel recommends.
The first megabyte holds several structures, and the table avoids all of them:

| range | contents |
| ----- | -------- |
| `0x00000000`-`0x000003FF` | the interrupt vector table for real mode |
| `0x00000400`-`0x000004FF` | the BIOS data area |
| `0x00000500`-`0x00007BFF` | free conventional memory |
| `0x00010000`-`0x0009FFFF` | the region where GRUB puts its multiboot data |

`0x00000800` is inside the free window, above the BIOS data area, and far below
the multiboot information structure. A write at `0x00000800` cannot reach any of
them.

The kernel copies the table to `0x00000800` at run time. It does not ask the
linker to place it there. `linker.ld` starts the image at 1 MiB and gives every
section a load address at or above that point:

```
    . = KERNEL_LOAD;
    .boot ALIGN(4K) : { KEEP(*(.multiboot)) *(.boot) }

    . += KERNEL_SPACE;
    .text : AT(ADDR(.text) - KERNEL_SPACE) { *(.text*) }
```

A second load segment in the first page would land in memory that the BIOS and
GRUB still use during the load of the image. The copy avoids that conflict
completely. Only 56 bytes of data move down, and they move after the handoff.
No part of the image is loaded below 1 MiB. `KERNEL_LOAD` is `0x00100000` and
`KERNEL_SPACE` is `0xC0000000`, so `.text` runs at a higher-half address while
it loads low; [PAGING.md](PAGING.md) explains that split.

## The install sequence

`gdt::install()` runs these steps, in this order:

1. Write the seven descriptors to `0x00000800` with `write_volatile`.
2. Build the 6-byte operand for the table register: a 16-bit limit, then a
   32-bit base. The struct is `#[repr(C, packed)]`, which keeps the base at
   offset 2. The limit is `7 * 8 - 1`, which is 55, or `0x0037`.
3. Run `lgdt` on that operand. The CPU now knows the base and the limit.
4. Load `0x10` into `ds`, `es`, `fs` and `gs`.
5. Load `0x18` into `ss`.
6. Push the code selector `0x08`, push the address of the next instruction, and
   run `retf`.

Step 3 changes nothing that an instruction can observe. Every segment register
still holds its hidden descriptor from GRUB's table. Steps 4 to 6 are the point
of the whole sequence.

Step 6 exists because no instruction can write `cs`. A `mov` to `cs` is invalid,
and a plain `ret` reloads only `eip`. A far return pops two values, and it puts
them into `cs` and `eip` together. The code pushes both values in that order and
runs `retf`, so the next instruction executes under the new code selector.

Never omit the far return. Its absence produces no fault at once, because the
old hidden copy stays valid. The failure arrives later, at the first
interrupt, the first far call or the first change of ring. That delay separates
the cause from the symptom by an unbounded amount of time.

After `install()` returns, the CPU reports these values:

```
cs = 0x0008        ds = es = fs = gs = 0x0010        ss = 0x0018
```

GRUB hands over `cs = 0x0010`, and it puts the data registers at `0x0018`. The
two sets of values disagree in every register, so no value above is an
inheritance from GRUB.

## The accessed bit

Bit 0 of the access byte is the accessed bit. The CPU sets it, and the CPU sets
it in the descriptor in memory, not in the hidden copy. The write happens the
first time that a selector loads. The kernel authors `0x92` for the kernel data
segment, and a read of that byte in the table afterwards returns `0x93`.

This behaviour is correct, and it is invisible to the kernel. It is visible to
any check that compares the bytes in memory against the bytes in the source.
`tools/check.sh` masks the one bit:

```sh
*) got="$got $(printf '0x%08x' $((0x${word#0x} & ~0x100)))" ;;
```

The accessed bit sits in the high word of the descriptor, at bit 8 of that word,
which is the mask `0x100`. The check clears it in the high words only and leaves
the low words untouched. Every other bit of every descriptor still has to match.

## The proof

`tools/check.sh` proves the table from outside the guest, through the QEMU human
monitor. The kernel never reports on its own table: the monitor reads the
register and the bytes of memory directly. `make check` holds 36 assertions
since the memory work, and five of them cover the table:

```
OK: GDT still at 0x00000800, limit 0x00000037 (7 descriptors)
OK: cs=0008 ss=0018 ds=es=fs=gs=0010, all from the new table
OK: null + kernel code/data/stack + user code/data/stack at 0x800
OK: the low window 0x00000000-0x00400000 is mapped, the GDT lives there
OK: `reboot` restarted the machine, the table and paging came back
```

Each assertion rules out a different failure:

- The first reads the table register with `info registers`. Only `lgdt` writes
  that register, so this assertion rules out a table that the kernel wrote to
  memory and never installed. It also rules out a wrong limit.
- The second reads the six segment registers. Only a load from the new table can
  put these values there, and a far return is the only path to `0x08` in `cs`.
  This assertion rules out an install that stops after `lgdt`.
- The third reads guest physical memory at `0x800` with `xp/14wx`. It compares
  all 14 words against the source. This assertion rules out a correct table
  register that points at wrong bytes, and it rules out a wrong access byte.
- The fourth reads `info mem`, which lists the mappings that the CPU's own
  tables describe, and requires the range `0-400000 -rw`. Without that range,
  linear `0x00000800` names nothing. This assertion rules out a directory that
  builds the higher half and forgets the low window.
- The fifth types `reboot`, waits for the banner to come back, and reads the
  table register and CR0 again. It rules out a table, or a page directory, that
  only the first boot gets right.

The shell's `gdt` command is the same read from inside the guest. `cmd_gdt`
calls `sgdt` through `gdt::current()`, so it reports the table that the CPU
uses and not the table that the source declares, and a mismatch prints
`DOES NOT MATCH THE SOURCE`. `make check` no longer types it, because the third
assertion compares the same bytes from outside. See [SHELL.md](SHELL.md) for
the output of that command.

The first assertion caught a real defect during development. The code computed
the limit from the size of the Rust helper array, and not from the count of
descriptors. A `Segment` carries a name and a selector besides the 8 bytes that
the CPU reads, so the register held `0x8b` instead of `0x37`.

The kernel still ran, and the screen showed nothing wrong. A limit that is too
large only allows selectors past the end of the table, and this kernel loads
none of them. The defect was invisible from inside the guest. That invisibility
is the reason for an external check. The fix reads the count of descriptors, and
it lives in one constant that both `install()` and the shell's `gdt` command
use, so the two cannot disagree:

```rust
pub const LIMIT: u16 = (SEGMENTS.len() * core::mem::size_of::<u64>() - 1) as u16;
```

## What paging changed

The two hazards that this file predicted both arrived with kfs-3, and
[PAGING.md](PAGING.md) describes the answers in full.

The table register holds a linear address, not a physical one. Paging is on
before `kmain` runs, so `gdt::install()` writes its 56 bytes to linear
`0x00000800`, and `lgdt` loads a linear base. Both still reach physical
`0x00000800`, because directory entry 0 maps the first 4 MB of linear addresses
to the first 4 MB of memory. That identity window exists for this reason above
all others. Remove it, and the table register names memory that nothing maps,
so the next segment load faults. The window is supervisor only, because entry
0 has its U/S bit clear, and it costs one thing: user space starts at 4 MB
instead of 0, which is why `mem::USER_BASE` equals `mem::LOW_WINDOW_END`. The
`virt` command prints the mapping that the register depends on:

```
kfs> virt
 address     dir  table offset physical    rights
 0x00000800     0     0   2048 0x00000800  rw kernel
```

A physical page allocator reads the memory map from GRUB and sees the first
page as free memory. `mem::init` therefore calls `pmm::reserve(0, BIOS_END)`
before any allocation runs, and that takes the whole first megabyte out of
circulation: the descriptor table at `0x00000800`, the VGA buffer at
`0x000b8000`, the BIOS data area, and the boot page tables. See
[MEMORY.md](MEMORY.md).
