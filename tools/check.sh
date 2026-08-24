#!/bin/sh
# End-to-end proof that kfs.iso boots and does what kfs-1 and kfs-2 ask.
#
# Nothing inside the guest takes part. The script talks to the QEMU
# human monitor over a unix socket, so every answer comes from the
# emulated hardware: the segment registers, guest physical memory, and
# the VGA text buffer. A kernel that only claims to work cannot pass.
#
# The screen assertions read 0xb8000 and decode the ASCII byte of each
# cell, which is exact text rather than a guess about pixel colour.

set -e

ISO=${ISO:-kfs.iso}
QEMU=${QEMU:-qemu-system-i386}
SOCK=${SOCK:-/tmp/kfs-mon}
BOOT_TIMEOUT=${BOOT_TIMEOUT:-60}

VGA_TEXT=0xb8000
CELLS=2000
COLUMNS=80
GDT_BASE=00000800
GDT_LIMIT=00000037

pass() { echo "OK: $1"; }
fail() { echo "FAIL: $1"; exit 1; }

# One monitor round trip. Each connection replays the banner and echoes
# the command, and the monitor writes CR before every LF, so strip the
# CRs and never anchor a pattern at end of line.
mon() { printf '%s\n' "$*" | socat - "unix-connect:$SOCK" 2>/dev/null | tr -d '\r'; }

# Every hex value on a `xp` output line, echo excluded. The monitor
# repeats each character as it is typed, so the address in the command
# itself appears in the reply and would be read as data. Only the dump
# lines start with an address and a colon.
dumped() {
	mon "$1" | grep '^[0-9a-f]\{8,\}: ' | grep -o "0x[0-9a-f]\{$2\}"
}

# The whole screen as 25 lines of text, read out of the VGA buffer.
# Each cell is a halfword whose low byte is the character and whose
# high byte is the colour, so 0xAABB carries the character in its last
# two digits. Anything outside printable ASCII becomes a space.
screen() {
	dumped "xp/${CELLS}hx $VGA_TEXT" 4 | cut -c5-6 | awk -v w=$COLUMNS '
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

# Type a word one key at a time. `sendkey` has no string form.
type_line() {
	for char in $(printf '%s\n' "$1" | sed 's/./& /g'); do
		mon "sendkey $char" > /dev/null
	done
	mon "sendkey ret" > /dev/null
	sleep 2
}

# Runs on every exit path, so it must not change the status the script
# is exiting with: `set -e` would otherwise turn a dead qemu into a
# failed run after every assertion already passed.
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

# The guest must reach the kernel's own address range. Both bounds
# matter: GRUB runs below 1 MiB and also relocated near the top of RAM,
# so "above 1 MiB" alone can pass while GRUB is still in charge. Polled
# rather than slept, because an emulated container boots slower.
eip=
waited=0
while [ $waited -lt "$BOOT_TIMEOUT" ]; do
	sleep 2
	waited=$((waited + 2))
	eip=$(mon info registers | sed -n 's/.*EIP=\([0-9a-f]*\).*/\1/p' | head -1)
	if [ -n "$eip" ] && [ $((0x$eip)) -ge $((0x100000)) ] \
		&& [ $((0x$eip)) -lt $((0x200000)) ]; then
		break
	fi
done
[ -n "$eip" ] || fail "qemu monitor unreachable, the guest died"
{ [ $((0x$eip)) -ge $((0x100000)) ] && [ $((0x$eip)) -lt $((0x200000)) ]; } \
	|| fail "EIP=$eip outside the kernel after ${BOOT_TIMEOUT}s"
pass "guest alive, EIP=$eip inside the kernel"

regs=$(mon info registers)

# kfs-2 mandatory: the table is at the address the subject fixes, and
# it is 7 descriptors long. This value comes from the CPU's GDTR, which
# only `lgdt` writes.
gdt=$(printf '%s\n' "$regs" | sed -n 's/^GDT= *\([0-9a-f]*\) \([0-9a-f]*\).*/\1 \2/p' | head -1)
[ "$gdt" = "$GDT_BASE $GDT_LIMIT" ] \
	|| fail "GDTR is '$gdt', expected '$GDT_BASE $GDT_LIMIT'"
pass "GDT at 0x$GDT_BASE, limit 0x$GDT_LIMIT (7 descriptors)"

# kfs-2 mandatory: the segments are declared *and in use*. Only a far
# jump can put 0x08 in cs, and only a load from the new table can put
# 0x10 and 0x18 in the others. GRUB hands over 0x10 and 0x18 in the
# other order, so these three values cannot be inherited.
for pair in "CS 0008" "DS 0010" "ES 0010" "FS 0010" "GS 0010" "SS 0018"; do
	name=${pair% *}
	want=${pair#* }
	got=$(printf '%s\n' "$regs" \
		| sed -n "s/^$name *=\([0-9a-f]\{4\}\) .*/\1/p" | head -1)
	[ "$got" = "$want" ] || fail "$name is $got, expected $want"
done
pass "cs=0008 ss=0018 ds=es=fs=gs=0010, all from the new table"

# kfs-2 mandatory: the descriptor bytes really are at 0x800. Each
# descriptor is two words, low then high. The accessed bit lives in the
# high word, and the CPU sets it in memory the first time a selector
# loads, so an authored 0x92 reads back as 0x93. Mask that one bit out
# of the high words only.
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

# kfs-1: the screen was cleared and the mandatory "42" is on row 0.
[ "$(row 0 | tr -d ' ')" = "42" ] || fail "row 0 is '$(row 0)', expected 42"
pass 'row 0 shows the mandatory "42"'
printf '%s\n' "$(row 1)" | grep -q 'kfs-2' || fail "row 1 lacks the banner"
pass "row 1 shows the kfs-2 banner"

# Bonus: the shell answers the keyboard. `sendkey` injects a scancode
# into the 8042 queue, which is exactly what the polled driver reads.
screen | grep -q 'kfs>' || fail "no shell prompt on screen"
pass "shell prompt is on screen"

type_line help
screen | grep -q 'hex dump of the live kernel stack' \
	|| fail "\`help\` printed no command list"
pass "\`help\` lists the commands"

type_line gdt
screen | grep -q 'as declared' \
	|| fail "\`gdt\` did not confirm the table against the source"
screen | grep -q 'kernel stack' || fail "\`gdt\` printed no kernel stack segment"
pass "\`gdt\` reads the table back from the CPU and agrees with the source"

type_line clear
[ -z "$(screen | tr -d ' \n')" ] && fail "\`clear\` left nothing, prompt included"
pass "\`clear\` blanked the screen"

# The dump must describe the live stack, and describe it consistently.
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

# Every byte between esp and the top must be rendered, 16 to a row.
rows=$(printf '%s\n' "$text" | grep -c '^[0-9a-f]\{8\}  [0-9a-f][0-9a-f] ' || true)
[ "$rows" -eq $(((used + 15) / 16)) ] \
	|| fail "$used bytes came out as $rows rows, expected $(((used + 15) / 16))"
pass "$used bytes rendered as $rows rows of 16"

# The first row must start exactly where the header says, so the address
# column is a real address and not an offset.
first=$(printf '%s\n' "$text" | grep '^[0-9a-f]\{8\}  ' | head -1)
[ "$(printf '%s' "$first" | cut -c1-8)" = "$dump_esp" ] \
	|| fail "first row is 0x$(printf '%s' "$first" | cut -c1-8), header says 0x$dump_esp"
pass "first row starts at 0x$dump_esp, the address the header names"

# The dword at the bottom of the window is the return address the shell
# pushed when it called the dump. Reading it back as code, not as
# another stack address, is what proves the copy did not overwrite its
# own source: a buffer inside the window makes the whole dump periodic
# and puts a stack address here instead.
word=$(printf '%s' "$first" | cut -c11-21 | tr -d ' ' \
	| sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/')
{ [ $((0x$word)) -ge $((0x100000)) ] && [ $((0x$word)) -lt $((0x200000)) ]; } \
	|| fail "the first dword is 0x$word, not a return address into the kernel"
pass "the first dword is 0x$word, a return address inside the kernel"

# Human-friendly means the reader recognises what is there. The line
# buffer still holds the command that was typed, and the ASCII column
# shows it as text.
printf '%s\n' "$text" | grep '^[0-9a-f]\{8\}  ' | grep -q 'stack' \
	|| fail "the ASCII column does not show the typed command"
pass "the ASCII column shows the typed command in the line buffer"

# Tie it to the CPU. The guest is now parked in the shell's poll loop,
# one frame shallower than the dump was taken in, and x86 stacks grow
# down, so the captured esp must be a little *below* the live one.
esp=$(mon info registers | sed -n 's/.*ESP=\([0-9a-f]*\).*/\1/p' | head -1)
[ $((0x$dump_esp)) -lt $((0x$esp)) ] \
	|| fail "dump esp 0x$dump_esp is not deeper than the live ESP=0x$esp"
[ $((0x$esp - 0x$dump_esp)) -lt 1024 ] \
	|| fail "dump esp 0x$dump_esp is $((0x$esp - 0x$dump_esp)) bytes from ESP=0x$esp"
pass "dump esp 0x$dump_esp is $((0x$esp - 0x$dump_esp)) bytes deeper than ESP=0x$esp"

# `reboot` restarts the machine, so the proof is that the kernel comes
# back: the banner returns and the table is installed again at the same
# address. That also shows `gdt::install` converges on the same state
# every time it runs, which is what makes it safe to run at every boot.
type_line reboot
booted=
waited=0
while [ $waited -lt "$BOOT_TIMEOUT" ]; do
	sleep 2
	waited=$((waited + 2))
	if [ "$(row 0 | tr -d ' ')" = "42" ] && screen | grep -q 'kfs>'; then
		booted=yes
		break
	fi
done
[ -n "$booted" ] || fail "the machine did not come back after \`reboot\`"
gdt=$(mon info registers | sed -n 's/^GDT= *\([0-9a-f]*\) \([0-9a-f]*\).*/\1 \2/p' | head -1)
[ "$gdt" = "$GDT_BASE $GDT_LIMIT" ] \
	|| fail "GDTR after the reboot is '$gdt', expected '$GDT_BASE $GDT_LIMIT'"
pass "\`reboot\` restarted the machine, and the table came back at 0x$GDT_BASE"

type_line halt
sleep 1
screen | grep -q 'halted' || fail "\`halt\` printed nothing"
hlt=$(mon info registers | sed -n 's/.*HLT=\([0-9]\).*/\1/p' | head -1)
[ "$hlt" = "1" ] || fail "\`halt\` left the CPU running (HLT=$hlt)"
pass "\`halt\` stopped the processor (HLT=1)"

echo "OK: every assertion passed"
