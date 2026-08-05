# Local dev image only — evaluation builds natively on Fedora amd64.
# amd64 is pinned because grub-pc-bin (x86 BIOS modules for grub-mkrescue)
# only exists on x86 hosts.
FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        nasm \
        binutils \
        grub-pc-bin \
        grub-common \
        xorriso \
        mtools \
        make \
        curl \
        gcc \
        libc6-dev \
        ca-certificates \
        qemu-system-x86 \
        socat \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf \
    | sh -s -- -y --default-toolchain nightly --component rust-src

ENV PATH=/root/.cargo/bin:$PATH

WORKDIR /kfs
