//! Kernel helpers — the subject's "basic kernel library".
//!
//! `core` already ships most of what a kernel needs from a libc
//! (`str::len`, slice comparison, ...) and the build's
//! `compiler-builtins-mem` feature supplies `memcpy`/`memset`/`memcmp`.
//! What neither covers are the C-shaped gaps: NUL-terminated strings —
//! the form GRUB's multiboot info structure uses — and turning numbers
//! into text before any formatting machinery exists.

/// Length of a NUL-terminated C string, excluding the NUL.
///
/// # Safety
/// `s` must point to readable memory that contains a NUL byte; the walk
/// stops nowhere else.
#[allow(dead_code)] // no caller until the kernel reads multiboot info strings
pub unsafe fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

/// Compare two NUL-terminated C strings; <0, 0 or >0 like C's strcmp.
///
/// # Safety
/// Both pointers must point to readable memory containing a NUL byte.
#[allow(dead_code)] // no caller until the kernel reads multiboot info strings
pub unsafe fn strcmp(a: *const u8, b: *const u8) -> i32 {
    let mut i = 0;
    loop {
        let (ca, cb) = unsafe { (*a.add(i), *b.add(i)) };
        if ca != cb || ca == 0 {
            return ca as i32 - cb as i32;
        }
        i += 1;
    }
}

/// Render `n` in decimal into `buf`, filled from the end, and return the
/// used tail as text. 10 bytes hold any u32, so this cannot fail.
pub fn utoa(mut n: u32, buf: &mut [u8; 10]) -> &str {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    // Every byte written above is an ASCII digit, so this is valid UTF-8.
    unsafe { core::str::from_utf8_unchecked(&buf[i..]) }
}
