# Physical memory, frames and the two heaps

This file explains where memory comes from on this machine and how the
kernel hands it out: the multiboot memory map (`kernel/src/multiboot.rs`),
the frame allocator (`kernel/src/pmm.rs`), the boot layout
(`kernel/src/mem.rs`) and the allocator behind `kmalloc` and `vmalloc`
(`kernel/src/heap.rs`). Page tables are in [PAGING.md](PAGING.md),
allocation failures in [PANIC.md](PANIC.md), terms in
[GLOSSARY.md](GLOSSARY.md). The reader this file is written for has
called `malloc` and `free` and has never written them.

## The three layers

| layer | owner | unit | question it answers |
| ----- | ----- | ---- | ------------------- |
| 1 | `multiboot.rs` | byte range | which physical RAM exists |
| 2 | `pmm.rs` | 4 kb frame | which of that RAM is free |
| 3 | `heap.rs` | byte | which bytes of a region are yours |

The layers only talk downwards. The frame allocator asks the memory map
what exists, once, at boot. A heap asks the frame allocator for frames
when it needs to grow, and asks paging to place those frames at the
virtual addresses it hands out. Nothing in layer 3 knows what RAM the
machine has, and nothing in layer 1 knows what a heap is. The boot
banner prints one line per layer boundary:

```
memory: 130559 kb usable in 2 regions, 32639 frames of 4 kb
paging: directory at 0x0010e000, 32636 kb carved at 0x0013a000 for kmalloc
```

The first line is layer 1 handing 32639 frames to layer 2. The second
is layer 2 handing one large block to layer 3.

## Layer 1: what RAM exists

The multiboot header at the top of `boot/boot.asm` sets two flag bits:
`MBALIGN` (page-aligned modules) and `MEMINFO` (describe memory).
Without `MEMINFO` GRUB may say nothing about RAM and the kernel would
have to probe the hardware itself.

GRUB answers by writing a structure into low physical memory and
leaving its address in `ebx`. `_start` pushes it, `kmain` passes it to
`mem::init`, and `multiboot::probe(magic, info)` reads it. `probe`
checks two things before it touches any field: `magic` must be
`0x2BADB002`, because any other value means no multiboot loader started
this kernel and the pointer means nothing, and `info` must be below
`0x00400000`.

The second check is about paging. Paging is already on when `mem::init`
runs, and the only place where a physical address is also a valid
virtual address is the low identity window, the first 4 MB (see
[PAGING.md](PAGING.md)). GRUB's structure lives in low memory, so the
identity window reaches it. A structure above 4 MB would need a mapping
the kernel does not have yet, so `probe` returns an error and
`mem::init` turns it into a fatal panic.

The memory map is a list of entries declared `#[repr(C, packed)]` with
fields `size: u32`, `base: u64`, `length: u64`, `kind: u32`. Because
`base` is 8 bytes wide starting 4 bytes into a packed struct, it sits
at a misaligned address. An ordinary load of a misaligned value is a
defect in Rust even where the hardware allows it, so every field is
read with `read_unaligned` through `addr_of!`; taking a plain reference
to a packed field is refused by the compiler, which forces the honest
form. The walk steps by the entry's own `size` plus 4, because a loader
may make entries longer than these four fields, and only `kind == 1`
(available) entries are kept.

With no memory map, `probe` falls back to bit 0 of the flags word,
which promises `mem_lower` and `mem_upper` in kilobytes, and pushes two
regions from them: `0` for `mem_lower * 1024` bytes and `0x00100000`
for `mem_upper * 1024`. That is coarser, because it cannot describe a
hole, but it boots. With neither source the kernel panics with
`the loader described no usable memory`. Regions are clamped to 32 bits
on the way in, and at most 12 are kept.

`mem` prints the parsed result. On the default QEMU machine:

```
ram: 130559 kb usable in 2 regions, from the multiboot memory map
  0x00000000..0x0009fc00  639 kb
  0x00100000..0x07fe0000  129920 kb
```

The first region is the memory below the BIOS data area at the top of
the first 640 kb. The second starts at 1 MB and stops 128 kb short of
128 MB, because the loader does not mark that top 128 kb usable. The
two sizes add up to 130559 kb.

## Layer 2: the frame allocator

`pmm.rs` owns physical memory in 4 kb frames. The whole state is one
static array of 1 << 20 bits, zero-initialised, where a set bit means
the frame is free. That polarity is why this shape works: zero-init
lands in `.bss`, so zero means "nothing is free" rather than
"everything is free". A map that defaults to handing out RAM the
machine does not have would be far worse. Three consequences: no
bootstrap allocator is needed, because the map exists before the first
line of Rust runs; the map costs nothing in the ISO, because `.bss` is
`NOBITS` and records only its size (128 KB); and 1 << 20 frames of
4 kb cover 4 GB, every physical address a 32-bit machine can name.

`set_free` and `set_used` both test the current bit before changing it,
so marking a used frame used again is a no-op and the free counter
cannot drift.

### What `init` marks free, and what `mem::init` takes back

`pmm::init(&ram)` walks the regions from layer 1 and marks whole frames
free. It rounds the base up and the end down, so a partial frame at
either edge is dropped. Region 1 ends at `0x0009fc00`, not a frame
boundary, so its last 3 kb is dropped. `TOTAL` counts a frame the
first time it is marked free, so overlapping regions cannot be counted
twice.

Marking free comes first, taking back comes second. `mem::init` makes
three reservations:

1. `pmm::reserve(0, BIOS_END)` takes the first 1 MB: the descriptor
   table at physical `0x800`, VGA at `0x000b8000`, BIOS data area,
   and the boot page tables.
2. `pmm::reserve(load_phys(), kernel_phys(image.end))` takes the
   loaded kernel image, from physical `0x00100000` to `0x0013a000`.
   Both bounds come from the linker, not a constant.
3. `pmm::carve(...)` takes one large contiguous block for `kmalloc`.

The image ends exactly on `0x0013a000`, so frame `0x13a` stays free,
and that is where the carve lands. `mem` prints the outcome:

```
frames: 24263 of 32639 free, 8376 used, largest free run 24263
```

Every number traces back to these steps:

| step | frames | which frames |
| ---- | ------ | ------------ |
| region `0x0..0x9fc00` | 159 | 0 to 158 |
| region `0x100000..0x7fe0000` | 32480 | 256 to 32735 |
| total | 32639 | sum of the two |
| reserve the first 1 MB | -159 | 0 to 158; 159-255 never existed |
| reserve the kernel image | -58 | 256 to 313, 232 kb |
| carve the kmalloc block | -8159 | 314 to 8472 |
| used | 8376 | 159 + 58 + 8159 |
| free | 24263 | 32639 - 8376 |

The image reservation covers 58 frames (232 kb), while `mem` reports
228 kb. The difference is the one frame at physical `0x00100000` that
holds the `.boot` section, below `kernel_start`. `largest_run` equals
the free count because every free frame lies above the carved block in
one unbroken run from frame 8473 to 32735. These numbers move with the
machine and with the kernel size; the shapes do not.

### `alloc_frame`, `alloc_range` and `carve`

`alloc_frame` gives one frame by scanning the bitmap for the first
non-zero word and using `trailing_zeros` to find the lowest set bit.
`alloc_range(n)` gives `n` contiguous frames using `find_run`, a
first-fit scan that skips forward by the length of each rejected run.
`carve(size)` calls `alloc_range` and halves the request on failure.

`carve` is called once. `mem::init` asks for the smaller of the whole
`kmalloc` address range (32768 kb) and a quarter of usable RAM, rounded
down to a whole page. Here the quarter of 130559 kb wins: 8159 pages,
32636 kb. The first attempt succeeds and returns the pair
`(0x0013a000, 0x02119000)`. Failure is fatal.

The carve happens before any other frame is handed out. Nothing else
has called `alloc_frame` yet, so the lowest free frame is the one just
above the kernel image and the block lands directly after it. Carving
later would still work, but a run that large might not exist.

## Layer 3: the two heaps

`heap.rs` defines one `Heap` type and instantiates it twice:

| | `kmalloc` | `vmalloc` |
| --- | --- | --- |
| base | `0xD0000000` | `0xE0000000` |
| ceiling | `0xD2000000`, 32 MB | `0xF0000000`, 256 MB |
| backing | `Contiguous` | `Scattered` |
| frames from | carved block, by address | `alloc_frame`, any frame |
| physically | one unbroken run | wherever a frame was free |

The only difference is `Backing`, and it changes exactly one function.
When a heap needs a frame for a page at virtual address `at`,
`frame_for` either computes `self.phys_base + (at - self.base)` for
a contiguous heap, or calls `pmm::alloc_frame()` for a scattered one.

The contiguous case does arithmetic, not allocation: the frame for a
page is fixed by the page's position in the heap. So for any live
`kmalloc` pointer, `phys = carved_base + (virt - 0xD0000000)`. The
first line of `alloc` shows it, 8 bytes into the heap and 8 bytes
into the carved block:

```
kmalloc(64)   -> 0xd0000008, ksize 64, physical 0x0013a008
```

### kmalloc is physically contiguous

The proof is a block that crosses a page boundary. `alloc` asks for
5000 bytes, which needs two pages, and translates the first byte and
the byte 4096 further on:

```
kmalloc(5000) -> 0xd0000050, physical 0x0013a050 then 0x0013b050
```

`0x0013b050 - 0x0013a050` is exactly 4096: two virtually adjacent
pages of `kmalloc` are physically adjacent as well. `cmd_alloc` asserts
it and fails the test otherwise.

### vmalloc is virtually contiguous only

`vmalloc` takes a fresh frame for every new page. `alloc` asks for
12288 bytes, three pages worth, and translates all three:

```
vmalloc(12288)-> 0xe0000008, vsize 12288
  frames 0x0211a000 0x0211c000 0x0211d000, physically scattered
```

The three frames are not consecutive: the gap between the first two is
8192, not 4096. The word `scattered` is computed, not printed blindly.
The gap has a specific cause. `grow` takes the data frame first and
maps it second; `paging::map_page` finds no page table for directory
slot 896 and takes `0x0211b000` for the table itself, inserting a frame
in the middle of the run. Any frame consumer has the same effect.
Physical adjacency in `vmalloc` is never a promise.

### Which one a caller should choose

The subject splits these into physical memory helpers (`kmalloc`,
`kfree`, `ksize`, `kbrk`) and virtual memory helpers (`vmalloc`,
`vfree`, `vsize`, `vbrk`). With paging on, that split cannot mean
"returns a physical pointer": both return a virtual address. What
differs is the promise about the physical side, which is exactly the
`Backing` distinction above.

- Call `kmalloc` when the physical layout matters: bytes a device would
  read by physical address, or code that must convert a pointer to a
  physical address by subtraction instead of walking the tables. The
  price is size: 32 MB of address range, backed by a 32636 kb block
  reserved at boot whether it is used or not.
- Call `vmalloc` for a plain buffer where only the virtual view
  matters. It can use any free frame, so it works when physical memory
  is too fragmented for a contiguous run. Its range is 256 MB and its
  real limit is the free frame count: 24263 frames, 97052 kb.

## Inside the allocator

Both heaps run the same code: a first-fit free list with no list
structure, because the blocks themselves tile the region.

Every block starts with an 8-byte header: 4 bytes for the usable size
of the payload and 4 bytes for the used flag (1 = handed out, 0 =
free). The payload starts at `block + 8`. `round_up` raises any
request below 8 bytes to 8 and rounds up to a multiple of 8; each heap
base is page aligned, so every payload is 8-byte aligned.

No header holds a pointer. The next block is at
`at + HEADER + size_at(at)`, so blocks tile the region from the base to
the break with no gap and no slack.

`first_fit(need)` walks that tiling. At each free block it coalesces
forward: while the next block is also free it absorbs it and rewrites
its own size, so two adjacent frees become one block the next time
anyone allocates. The scan that looks for space is the scan that
merges. It returns the first block of at least `need` bytes. If nothing
fits, `alloc` calls `grow` for the pages `need + HEADER` requires and
searches once more.

`take(block, need)` splits the block when the remainder can hold a
header and the smallest payload: `size >= need + 8 + 8`. It writes a
used header at `block` and a free header at `block + 8 + need`.
Otherwise the whole block is handed out unsplit.

```
 after grow(1), one free block covers the page:

 0xd0000000                                       0xd0001000
 +------------------+---------------------------------+
 | size 4088 used 0 |  4088 free bytes                |
 +------------------+---------------------------------+

 after kmalloc(64), take() split it in two:

 0xd0000000         0xd0000048                0xd0001000
 +--------+--------+----------+-----------------+
 | sz  64 | 64-byte| sz  4016 |  4016 free      |
 | used 1 | payload| used 0   |                 |
 +--------+--------+----------+-----------------+
          ^                   ^
          0xd0000008          0xd0000050
```

8 + 64 + 8 + 4016 = 4096, the whole page, which is what "tile" means.

`ksize` and `vsize` find the block whose payload starts at the pointer
and return the header's size: the usable size, not the request. For a
rounded request that fitted a split block the two agree (`ksize 64` for
`kmalloc(64)`). For a block handed out without a split it can be larger
still. All of it is usable.

## `kbrk` and `vbrk`

`brk` moves the top of a heap with sbrk semantics: the **old** break
is returned, and a delta of 0 queries without changing anything.
Growth is in whole pages; for each page `grow` checks the ceiling,
gets a frame from `frame_for`, and calls `paging::map_page`. It then
repairs the tiling: if the block at the old top is free it absorbs the
new span, otherwise a fresh free header is written at the old break.

Shrinking unmaps pages from the new break up to the old one and frees
each returned frame **only** for a scattered heap. A contiguous heap
must not free its frames: they belong to the carved block and
`frame_for` computes them again from the address.

Shrinking refuses in three cases, each an oops, not a crash:

- The top block is in use (`used_at(block)` is true).
- The new break would cut into a used block (`target < block`).
- The remaining free top is too small for a header plus 8 bytes.

`koops!` prints one yellow line and returns; `brk` then returns null
and the kernel keeps running. See [PANIC.md](PANIC.md).

`alloc` exercises all four combinations:

```
kbrk(+4096)   -> break was 0xd0003000, now 0xd0004000
kbrk(-4096)   -> break 0xd0003000
vbrk(+8192)   -> break was 0xe0004000, now 0xe0006000
vbrk(-8192)   -> break 0xe0004000
```

The starting breaks are not arbitrary. `kmalloc(5000)` had grown the
heap to three pages (`0xd0003000`); `vmalloc(12288)` needed four pages
(`0xe0004000`). The returned value is visibly the old break each time.

## What the allocators refuse

The allocator never trusts a pointer it did not produce. Every case
reports through `koops!` and returns a harmless value: `Heap::free`
reports `did not come from NAME` for a foreign pointer and
`is freed twice in NAME` for a double free; `Heap::size` reports the
same `did not come from` and returns 0; `Heap::frame_for` reports
`ran out of its physical block` or `found no free physical frame`;
`Heap::grow` reports `reached the end of its range`; and
`paging::map_page` reports `no free frame for the page table`.

`find_block` makes the pointer checks possible. It walks the tiling
from the base and accepts the pointer only if some block's payload
starts exactly there. A pointer into the middle of a block, a stack
address and a pointer from the other heap all fail. `alloc(0)` returns
null with no report, and `free(null)` returns silently, for the same
reason `free(NULL)` is legal in C.

## The `mem` command

`cmd_mem` prints all three layers. From a fresh boot:

```
kfs> mem
ram: 130559 kb usable in 2 regions, from the multiboot memory map
  0x00000000..0x0009fc00  639 kb
  0x00100000..0x07fe0000  129920 kb
frames: 24263 of 32639 free, 8376 used, largest free run 24263
kernel: 0xc0101000..0xc013a000, code to 0xc010949b, read only to 0xc010d000
        physical 0x00101000..0x0013a000, 228 kb
carved: 0x0013a000..0x02119000 physical, kept for kmalloc
kmalloc: 0xd0000000..0xd0000000, ceiling 0xd2000000, 0 kb mapped
  0 bytes used, 0 free, 0 of 0 blocks live, physically contiguous
  physical 0x0013a000..0x02119000
vmalloc: 0xe0000000..0xe0000000, ceiling 0xf0000000, 0 kb mapped
  0 bytes used, 0 free, 0 of 0 blocks live, physically scattered
```

`ram:` is the parsed map with total, region count, and source string.
`frames:` is the bitmap summary. `kernel:` is the image bounds as
virtual and physical addresses; the end of code is the only address
that is not page aligned and the only one that moves on rebuild. The
rights those bounds produce are in [PAGING.md](PAGING.md). `carved:`
is the physical block reserved for `kmalloc`. Each heap line shows
base, break, ceiling, and mapped kilobytes, then bytes used, bytes
free, live and total block counts, and the backing. The physical range
is printed only for the contiguous heap; a scattered heap has no single
range. `make check` asserts that both heaps print at their base
addresses and that the kernel heap says `physically contiguous`.

## The `alloc` command

`alloc` is a self test with a counted verdict:

```
kfs> alloc
kmalloc(64)   -> 0xd0000008, ksize 64, physical 0x0013a008
kmalloc(5000) -> 0xd0000050, physical 0x0013a050 then 0x0013b050
kfree both    -> used bytes 5064 then 0
vmalloc(12288)-> 0xe0000008, vsize 12288
  frames 0x0211a000 0x0211c000 0x0211d000, physically scattered
kbrk(+4096)   -> break was 0xd0003000, now 0xd0004000
kbrk(-4096)   -> break 0xd0003000
vbrk(+8192)   -> break was 0xe0004000, now 0xe0006000
vbrk(-8192)   -> break 0xe0004000
alloc: 14 checks passed
```

Every step is a check, not a print. A failure returns early through
`failed` and the count is never reached. The 14 checks: both `kmalloc`
calls returned a pointer, `ksize >= 64`, the 5000-byte block is
physically contiguous across a page boundary, both blocks hold bytes
(every byte is written and read back), `kfree` reduced the used count,
`vmalloc` returned a pointer, the vmalloc range holds bytes, `vfree`
ran, and each of the four `brk` calls returned non-null.

The `kfree both` line reads `used bytes 5064 then 0`: 64 + 5000, the
two live payloads, because both requests were multiples of 8.

`make check` asserts no `FAILED` line, a count of at least 14, and
that the first allocation lands at `0xd0000008`, the base of the
kernel heap.

## Limits

- **No reference counting.** A block has one owner. A double free is
  reported, and sharing is the caller's problem.
- **No slab, no size classes, no cache.** One free list per heap with
  8-byte granularity.
- **First fit fragments.** Holes that are not adjacent cannot be
  merged. There is no compaction and a pointer never moves.
- **Linear walks.** `first_fit`, `find_block`, `last_block` and
  `stats` walk from the base; `largest_run` walks all 1 << 20 bits.
- **One heap of each kind.** No per-process heaps, because there are
  no processes yet.
- **Nothing shrinks by itself.** The carved 32636 kb block is held
  from boot to halt whether `kmalloc` uses a byte of it or not.
- **No locking.** One processor, no scheduler, interrupts off. The day
  interrupts arrive, every function in `heap.rs` and `pmm.rs` needs
  review.
- **The kmalloc ceiling and its block disagree.** The address range is
  32768 kb and the carved block is 32636 kb, so `frame_for` reports
  exhaustion about 132 kb before `grow` reaches the ceiling. The
  physical block is the real limit.
