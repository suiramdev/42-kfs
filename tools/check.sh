#!/bin/sh

set -e

ISO=${ISO:-kfs.iso}
QEMU=${QEMU:-qemu-system-i386}
SOCK=${SOCK:-/tmp/kfs-mon}
BOOT_TIMEOUT=${BOOT_TIMEOUT:-60}

VGA_TEXT=0xb8000
CELLS=2000
WIDTH=80
GDT_BASE=00000800
GDT_LIMIT=00000037
KERNEL_LOW=0xc0100000
KERNEL_HIGH=0xc0200000

pass() { echo "OK: $1"; }
fail() { echo "FAIL: $1"; exit 1; }

mon() { printf '%s\n' "$*" | socat - "unix-connect:$SOCK" 2>/dev/null | tr -d '\r'; }

dumped() {
	mon "$1" | grep '^[0-9a-f]\{8,\}: ' | grep -o "0x[0-9a-f]\{$2\}"
}

screen() {
	dumped "xp/${CELLS}hx $VGA_TEXT" 4 | cut -c5-6 | awk -v w=$WIDTH '
		BEGIN { for (i = 0; i < 16; i++) v[substr("0123456789abcdef", i + 1, 1)] = i }
		{
			b = v[substr($0, 1, 1)] * 16 + v[substr($0, 2, 1)]
			printf "%c", (b >= 32 && b < 127) ? b : 32
			if (++n % w == 0) printf "\n"
		}'
}

row() {
	screen | sed -n "$(($1 + 1))p"
}

type_line() {
	for char in $(printf '%s\n' "$1" | sed 's/ /_/g; s/./& /g'); do
		case $char in
		_) mon "sendkey spc" > /dev/null ;;
		*) mon "sendkey $char" > /dev/null ;;
		esac
	done
	mon "sendkey ret" > /dev/null
	sleep 2
}

registers() { mon info registers; }

control() {
	registers | sed -n "s/.*$1=\([0-9a-f]*\).*/\1/p" | head -1
}

mappings() {
	mon "info mem" \
		| sed -n 's/^0*\([0-9a-f]\{1,8\}\)-0*\([0-9a-f]\{1,8\}\) [0-9a-f]* \(.*\)/\1-\2 \3/p'
}

booted() {
	waited=0
	while [ $waited -lt "$BOOT_TIMEOUT" ]; do
		sleep 2
		waited=$((waited + 2))
		if [ "$(row 0 | tr -d ' ')" = "42" ] && screen | grep -q 'kfs>'; then
			return 0
		fi
	done
	return 1
}

cleanup() {
	status=$?
	mon quit > /dev/null 2>&1 || true
	kill "$(cat "$PIDFILE" 2>/dev/null)" 2> /dev/null || true
	rm -f "$SOCK" "$PIDFILE" || true
	exit $status
}

PIDFILE=$(mktemp)
trap cleanup EXIT INT TERM

rm -f "$SOCK"
$QEMU -cdrom "$ISO" -display none -monitor "unix:$SOCK,server,nowait" &
echo $! > "$PIDFILE"

eip=
waited=0
while [ $waited -lt "$BOOT_TIMEOUT" ]; do
	sleep 2
	waited=$((waited + 2))
	eip=$(control EIP)
	if [ -n "$eip" ] && [ $((0x$eip)) -ge $((KERNEL_LOW)) ] \
		&& [ $((0x$eip)) -lt $((KERNEL_HIGH)) ]; then
		break
	fi
done
[ -n "$eip" ] || fail "qemu monitor unreachable, the guest died"
{ [ $((0x$eip)) -ge $((KERNEL_LOW)) ] && [ $((0x$eip)) -lt $((KERNEL_HIGH)) ]; } \
	|| fail "EIP=$eip outside the higher half kernel after ${BOOT_TIMEOUT}s"
pass "guest alive, EIP=$eip inside the kernel at 3 gb"

regs=$(registers)

gdt=$(printf '%s\n' "$regs" | sed -n 's/^GDT= *\([0-9a-f]*\) \([0-9a-f]*\).*/\1 \2/p' | head -1)
[ "$gdt" = "$GDT_BASE $GDT_LIMIT" ] \
	|| fail "GDTR is '$gdt', expected '$GDT_BASE $GDT_LIMIT'"
pass "GDT still at 0x$GDT_BASE, limit 0x$GDT_LIMIT (7 descriptors)"

for pair in "CS 0008" "DS 0010" "ES 0010" "FS 0010" "GS 0010" "SS 0018"; do
	name=${pair% *}
	want=${pair#* }
	got=$(printf '%s\n' "$regs" \
		| sed -n "s/^$name *=\([0-9a-f]\{4\}\) .*/\1/p" | head -1)
	[ "$got" = "$want" ] || fail "$name is $got, expected $want"
done
pass "cs=0008 ss=0018 ds=es=fs=gs=0010, all from the new table"

words=$(dumped "xp/14wx 0x800" 8 | tr '\n' ' ')
i=0
got=
for word in $words; do
	i=$((i + 1))
	case $((i % 2)) in
	1) got="$got $word" ;;
	*) got="$got $(printf '0x%08x' $((0x${word#0x} & ~0x100)))" ;;
	esac
done
want=" 0x00000000 0x00000000"
want="$want 0x0000ffff 0x00cf9a00"
want="$want 0x0000ffff 0x00cf9200"
want="$want 0x0000ffff 0x00cf9200"
want="$want 0x0000ffff 0x00cffa00"
want="$want 0x0000ffff 0x00cff200"
want="$want 0x0000ffff 0x00cff200"
[ "$got" = "$want" ] || fail "descriptors at 0x800 are$got, expected$want"
pass "null + kernel code/data/stack + user code/data/stack at 0x800"

cr0=$(control CR0)
cr3=$(control CR3)
[ -n "$cr0" ] || fail "the monitor printed no CR0"
[ $((0x$cr0 & 0x80000000)) -ne 0 ] || fail "CR0=$cr0 has paging off"
pass "CR0=$cr0, paging is on"
[ $((0x$cr0 & 0x10000)) -ne 0 ] || fail "CR0=$cr0 has write protect off"
pass "CR0=$cr0, write protect is on, so ring 0 obeys the read only bit"
[ $((0x$cr3 & 0xfff)) -eq 0 ] || fail "CR3=$cr3 is not page aligned"
pass "CR3=$cr3, a page aligned directory"

maps=$(mappings)
printf '%s\n' "$maps" | grep -q '^0-400000 -rw' \
	|| fail "the low identity window is not mapped rw: $maps"
pass "the low window 0x00000000-0x00400000 is mapped, the GDT lives there"
printf '%s\n' "$maps" | grep -q '^c0000000-' \
	|| fail "the kernel window at 0xc0000000 is not mapped: $maps"
pass "the kernel window at 0xc0000000 is mapped, 4 mb of ram at 3 gb"
printf '%s\n' "$maps" | awk '$2 == "-r-" && $1 ~ /^c01/ { found = 1 } END { exit !found }' \
	|| fail "no read only range inside the kernel image: $maps"
pass "the kernel's own code is mapped read only: $(printf '%s\n' "$maps" | awk '$2 == "-r-" { print $1 }')"
printf '%s\n' "$maps" | grep -q '^ffc00000-\|^fff00000-\|^fffff000-' \
	|| fail "the page table window at 0xffc00000 is not mapped: $maps"
pass "the page table window is mapped, the directory sees itself"

[ "$(row 0 | tr -d ' ')" = "42" ] || fail "row 0 is '$(row 0)', expected 42"
pass 'row 0 shows the mandatory "42"'
printf '%s\n' "$(row 1)" | grep -q 'kfs-3' || fail "row 1 lacks the banner"
pass "row 1 shows the kfs-3 banner"
screen | grep -q 'faults: 256 vectors' || fail "the boot said nothing about the idt"
pass "the boot reports the exception table"
boot_cr3=$(screen | sed -n "s/.*directory at 0x\([0-9a-f]*\).*/\1/p" | head -1)
[ "$boot_cr3" = "$cr3" ] \
	|| fail "the kernel printed cr3 0x$boot_cr3, the cpu holds 0x$cr3"
pass "the kernel and the cpu agree on cr3 0x$cr3"

screen | grep -q 'kfs>' || fail "no shell prompt on screen"
pass "shell prompt is on screen"

type_line help
screen | grep -q 'physical memory, frames and both heaps' \
	|| fail "\`help\` printed no command list"
pass "\`help\` lists the commands"

type_line clear
type_line mem
text=$(screen)
printf '%s\n' "$text" | grep -q 'ram: [0-9]* kb usable' || fail "\`mem\` printed no ram total"
printf '%s\n' "$text" | grep -q 'frames: [0-9]* of [0-9]* free' || fail "\`mem\` printed no frames"
printf '%s\n' "$text" | grep -q 'kmalloc: 0xd0000000' || fail "\`mem\` printed no kmalloc heap"
printf '%s\n' "$text" | grep -q 'vmalloc: 0xe0000000' || fail "\`mem\` printed no vmalloc heap"
printf '%s\n' "$text" | grep -q 'physically contiguous' \
	|| fail "\`mem\` does not say the kernel heap is contiguous"
pass "\`mem\` reports ram, frames, the carved block and both heaps"

type_line clear
type_line space
text=$(screen)
printf '%s\n' "$text" | grep -q '0x00400000..0x007fffff user' \
	|| fail "\`space\` does not define user space"
printf '%s\n' "$text" | grep -q '0xc0000000..0xc03fffff kernel' \
	|| fail "\`space\` does not define kernel space"
pass "\`space\` names every region and its owner"

type_line clear
type_line pages
text=$(screen)
printf '%s\n' "$text" | grep -q '0xc0000000..' || fail "\`pages\` dumped no kernel window"
printf '%s\n' "$text" | grep -q 'r  kernel' \
	|| fail "\`pages\` shows no read only kernel pages"
printf '%s\n' "$text" | grep -q 'rw kernel' || fail "\`pages\` shows no writable pages"
pass "\`pages\` walks the directory and shows the rights it holds"

type_line clear
type_line "virt 0xc0100000"
text=$(screen)
printf '%s\n' "$text" | grep -q 'directory 768 table 256 offset 0' \
	|| fail "\`virt\` did not split the address into 10, 10 and 12 bits"
printf '%s\n' "$text" | grep -q 'physical        0x00100000' \
	|| fail "\`virt\` did not translate 0xc0100000 to 0x00100000"
pass "\`virt\` walks directory 768, table 256 down to physical 0x00100000"

type_line clear
type_line alloc
text=$(screen)
printf '%s\n' "$text" | grep -q 'FAILED' && fail "\`alloc\` reported a failure"
checks=$(printf '%s\n' "$text" | sed -n 's/.*alloc: \([0-9]*\) checks passed.*/\1/p' | head -1)
[ -n "$checks" ] && [ "$checks" -ge 14 ] \
	|| fail "\`alloc\` passed only '$checks' checks"
pass "\`alloc\` passed $checks checks over kmalloc, vmalloc, kbrk and vbrk"
printf '%s\n' "$text" | grep -q 'kmalloc(64)   -> 0xd0000008' \
	|| fail "the first kmalloc did not land at the base of the heap"
pass "kmalloc hands out the start of the kernel heap"

type_line clear
type_line user
text=$(screen)
printf '%s\n' "$text" | grep -q 'rw user' || fail "\`user\` mapped no user page"
printf '%s\n' "$text" | grep -q 'read back 0x42424242' \
	|| fail "\`user\` could not write through the user page"
printf '%s\n' "$text" | grep -q 'r  user' || fail "\`user\` could not drop the write right"
printf '%s\n' "$text" | grep -q 'kernel oops' \
	|| fail "a user page in kernel space was not refused"
printf '%s\n' "$text" | grep -q 'nothing there' || fail "\`user\` could not unmap the page"
pass "\`user\` maps, writes, protects and drops a page in user space"

type_line clear
type_line "fault demand"
text=$(screen)
printf '%s\n' "$text" | grep -q 'kernel oops' \
	|| fail "the demand fault printed no recovered panic"
printf '%s\n' "$text" | grep -q 'demand paged' || fail "the fault handler mapped nothing"
printf '%s\n' "$text" | grep -q 'the read returned 0x00000000' \
	|| fail "the retried read did not come back"
pass "a page fault in the demand zone is recovered, the kernel keeps running"

type_line "panic oops"
screen | grep -q 'walk away from' || fail "\`panic oops\` printed nothing"
screen | grep -q 'kfs>' || fail "\`panic oops\` did not return to the prompt"
pass "\`panic oops\` prints and returns, this panic is not fatal"

type_line clear
type_line stack
text=$(screen)

header=$(printf '%s\n' "$text" | grep 'stack: esp ' | head -1)
[ -n "$header" ] || fail "\`stack\` printed no header"
set -- $(printf '%s\n' "$header" \
	| sed -n 's/.*esp 0x\([0-9a-f]*\) -> top 0x\([0-9a-f]*\), \([0-9]*\) of \([0-9]*\) .*/\1 \2 \3 \4/p')
[ $# -eq 4 ] || fail "cannot read the header: '$header'"
dump_esp=$1 dump_top=$2 used=$3 size=$4

[ "$size" = "16384" ] || fail "stack is $size bytes, boot.asm reserves 16384"
[ $((0x$dump_esp)) -lt $((0x$dump_top)) ] \
	|| fail "esp 0x$dump_esp is not below top 0x$dump_top"
[ $((0x$dump_top - 0x$dump_esp)) -eq "$used" ] \
	|| fail "header says $used bytes in use, but top - esp is not that"
pass "header is self-consistent: $used of $size bytes below 0x$dump_top"

first=$(printf '%s\n' "$text" | grep '^[0-9a-f]\{8\}  ' | head -1)
[ "$(printf '%s' "$first" | cut -c1-8)" = "$dump_esp" ] \
	|| fail "first row is 0x$(printf '%s' "$first" | cut -c1-8), header says 0x$dump_esp"
pass "first row starts at 0x$dump_esp, the address the header names"

word=$(printf '%s' "$first" | cut -c11-21 | tr -d ' ' \
	| sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/')
{ [ $((0x$word)) -ge $((KERNEL_LOW)) ] && [ $((0x$word)) -lt $((KERNEL_HIGH)) ]; } \
	|| fail "the first dword is 0x$word, not an address inside the kernel"
{ [ $((0x$word)) -lt $((0x$dump_top - size)) ] || [ $((0x$word)) -ge $((0x$dump_top)) ]; } \
	|| fail "the first dword 0x$word is in the stack: the dump shows its own frame"
pass "the first dword is 0x$word, a kernel pointer from outside the stack"

esp=$(control ESP)
[ $((0x$dump_esp)) -lt $((0x$esp)) ] \
	|| fail "dump esp 0x$dump_esp is not deeper than the live ESP=0x$esp"
[ $((0x$esp - 0x$dump_esp)) -lt 1024 ] \
	|| fail "dump esp 0x$dump_esp is $((0x$esp - 0x$dump_esp)) bytes from ESP=0x$esp"
pass "dump esp 0x$dump_esp is $((0x$esp - 0x$dump_esp)) bytes deeper than ESP=0x$esp"

type_line reboot
booted || fail "the machine did not come back after \`reboot\`"
gdt=$(registers | sed -n 's/^GDT= *\([0-9a-f]*\) \([0-9a-f]*\).*/\1 \2/p' | head -1)
[ "$gdt" = "$GDT_BASE $GDT_LIMIT" ] \
	|| fail "GDTR after the reboot is '$gdt', expected '$GDT_BASE $GDT_LIMIT'"
[ $((0x$(control CR0) & 0x80000000)) -ne 0 ] || fail "paging is off after the reboot"
pass "\`reboot\` restarted the machine, the table and paging came back"

type_line "fault ro"
sleep 1
text=$(screen)
printf '%s\n' "$text" | grep -q 'KERNEL PANIC: page fault' \
	|| fail "writing to read only kernel code did not panic"
printf '%s\n' "$text" | grep -q 'a kernel write' \
	|| fail "the panic did not name the write"
printf '%s\n' "$text" | grep -q 'the rights said no' \
	|| fail "the panic did not name the rights"
printf '%s\n' "$text" | grep -q 'the kernel is stopped' || fail "the panic did not stop"
hlt=$(registers | sed -n 's/.*HLT=\([0-9]\).*/\1/p' | head -1)
[ "$hlt" = "1" ] || fail "the fatal panic left the CPU running (HLT=$hlt)"
pass "a write to read only kernel code panics and stops the processor"

mon system_reset > /dev/null
booted || fail "the machine did not come back from system_reset"
pass "the machine boots again for the remaining fatal cases"

type_line "fault kernel"
sleep 1
text=$(screen)
printf '%s\n' "$text" | grep -q 'KERNEL PANIC: page fault' \
	|| fail "touching unmapped kernel space did not panic"
printf '%s\n' "$text" | grep -q 'the page was not present' \
	|| fail "the panic did not say the page was absent"
hlt=$(registers | sed -n 's/.*HLT=\([0-9]\).*/\1/p' | head -1)
[ "$hlt" = "1" ] || fail "the fatal page fault left the CPU running (HLT=$hlt)"
pass "a write to unmapped kernel space panics and stops the processor"

mon system_reset > /dev/null
booted || fail "the machine did not come back from the second system_reset"

type_line "panic fatal"
sleep 1
text=$(screen)
printf '%s\n' "$text" | grep -q 'KERNEL PANIC: the shell asked' \
	|| fail "\`panic fatal\` printed no panic"
printf '%s\n' "$text" | grep -q 'the kernel is stopped' || fail "\`panic fatal\` did not stop"
hlt=$(registers | sed -n 's/.*HLT=\([0-9]\).*/\1/p' | head -1)
[ "$hlt" = "1" ] || fail "\`panic fatal\` left the CPU running (HLT=$hlt)"
pass "\`panic fatal\` prints, dumps the machine and stops the processor"

mon system_reset > /dev/null
booted || fail "the machine did not come back from the third system_reset"

type_line halt
sleep 1
screen | grep -q 'halted' || fail "\`halt\` printed nothing"
hlt=$(registers | sed -n 's/.*HLT=\([0-9]\).*/\1/p' | head -1)
[ "$hlt" = "1" ] || fail "\`halt\` left the CPU running (HLT=$hlt)"
pass "\`halt\` stopped the processor (HLT=1)"

echo "OK: every assertion passed"
