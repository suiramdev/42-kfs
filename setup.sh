#!/bin/sh
# Put a Fedora machine in a state where `make check` passes, without ever
# asking for root — the case on a school machine, or on a peer's during
# defense.
#
# What it takes care of, in order:
#   1. the build tools, installed inside a toolbox container when the host
#      lacks them (a toolbox is a Fedora you own, so `sudo` works in it);
#   2. rustup, which installs into $HOME and needs no root either. The
#      nightly toolchain and rust-src come from rust-toolchain.toml, which
#      cargo applies on its own the first time it runs.
#
# Idempotent: run it twice, the second run installs nothing.

set -eu

REPO=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PKGS="make nasm binutils gcc glibc-devel grub2-tools grub2-tools-extra \
grub2-pc-modules xorriso mtools qemu-system-x86 socat"

say() { printf '\n==> %s\n' "$1"; }
have() { command -v "$1" >/dev/null 2>&1; }

# Every binary the Makefile shells out to, plus the BIOS boot modules
# grub-mkrescue needs: without them it writes an ISO with no boot sector and
# says nothing about it.
host_has_tools() {
	for t in nasm ld grub2-mkrescue grub2-file xorriso qemu-system-i386 socat; do
		have "$t" || return 1
	done
	test -d /usr/lib/grub/i386-pc
}

# --- 1. build tools ---------------------------------------------------------

if host_has_tools; then
	say "Build tools: already on the host, no container needed."
	RUN=""
else
	say "Build tools: missing on the host, using a toolbox container."
	have toolbox || {
		echo "FAIL: 'toolbox' is not installed, and installing it needs root." >&2
		echo "      Ask for a machine that has it, or install the packages" >&2
		echo "      listed at the top of this script by hand." >&2
		exit 1
	}
	# `toolbox run true` fails when no container exists yet. Creating one
	# pulls ~500 MB the first time, then costs nothing.
	toolbox run true >/dev/null 2>&1 || toolbox create -y
	# shellcheck disable=SC2086 # PKGS is a word list on purpose
	toolbox run sudo dnf install -y $PKGS
	RUN="toolbox run"
fi

# --- 2. rust ----------------------------------------------------------------

# $HOME is shared with the container, so one install serves both sides.
# `toolbox run` does NOT carry the environment into the container, so these
# two are passed explicitly through `env` at every call below. Set them before
# running this script to put the ~900 MB of rust somewhere other than $HOME.
RUSTUP_DIR="${RUSTUP_HOME:-$HOME/.rustup}"
CARGO_DIR="${CARGO_HOME:-$HOME/.cargo}"
ENV="env RUSTUP_HOME=$RUSTUP_DIR CARGO_HOME=$CARGO_DIR"

if [ -x "$CARGO_DIR/bin/cargo" ]; then
	say "Rust: already installed at $CARGO_DIR/bin/cargo."
else
	say "Rust: installing rustup (no root involved)."
	# Toolchain, rust-src and the crate registry come to ~1.6 GB (measured),
	# landing on a home partition that is 4.7 GB in total at school. Say so
	# before filling it rather than after — measured on the partition that
	# will actually receive it, which RUSTUP_HOME may have moved elsewhere.
	# A toolbox shares $HOME and nothing else of the host's disks — a
	# RUSTUP_HOME pointing anywhere else simply would not exist in there.
	case "$RUSTUP_DIR/" in
	"$HOME"/*) ;;
	*)
		if [ -n "$RUN" ]; then
			echo "FAIL: $RUSTUP_DIR is outside \$HOME, which the toolbox" >&2
			echo "      does not share. Choose a path under $HOME." >&2
			exit 1
		fi
		;;
	esac
	target="$RUSTUP_DIR"
	while [ ! -d "$target" ]; do target=$(dirname "$target"); done
	free_mb=$(df -Pm "$target" | awk 'NR==2 {print $4}')
	if [ "$free_mb" -lt 1800 ]; then
		echo "FAIL: only ${free_mb} MB free on $target, rust needs ~1.6 GB." >&2
		echo "      Free some up ('rm -rf ~/.cache' is usually the fastest)," >&2
		echo "      or set RUSTUP_HOME and CARGO_HOME to a roomier disk —" >&2
		echo "      possible only on a host that has the build tools itself," >&2
		echo "      since the toolbox sees no disk but $HOME." >&2
		exit 1
	fi
	if [ -n "$RUN" ]; then
		$RUN sudo dnf install -y rustup
	elif ! have rustup-init; then
		echo "FAIL: no rustup-init on this host, and no container to" >&2
		echo "      install it from. See https://rustup.rs" >&2
		exit 1
	fi
	$RUN $ENV rustup-init -y --no-modify-path \
		--default-toolchain nightly --component rust-src
fi

# --- 3. proof ---------------------------------------------------------------

say "Building and running the boot proof."
# shellcheck disable=SC2086 # RUN and ENV are command prefixes, split on purpose
$RUN $ENV make -C "$REPO" check

say "Done. To see it on screen:"
if [ -n "$RUN" ]; then
	printf '    toolbox enter\n'
fi
if [ "$CARGO_DIR" != "$HOME/.cargo" ]; then
	printf '    export CARGO_HOME=%s\n' "$CARGO_DIR"
fi
printf '    cd %s && make run\n\n' "$REPO"
