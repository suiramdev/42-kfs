//! printk — formatted printing over the VGA driver.
//!
//! `core` cannot print, but it ships the whole formatting engine:
//! anything implementing `core::fmt::Write` (one method, `write_str`)
//! gets `write_fmt` and every `{}` / `{:x}` / padding rule for free.
//! `vga::Writer` is that implementation, and `printk!` is the
//! `format_args!` front door — the same split as `printf` vs `write`,
//! except the formatting happens with zero allocation: `format_args!`
//! compiles the format string into a series of `write_str` calls.

/// Print with `core::fmt` formatting, e.g. `printk!("kfs-{}\n", 1)`.
macro_rules! printk {
    ($($arg:tt)*) => {
        $crate::printk::print(core::format_args!($($arg)*))
    };
}
pub(crate) use printk;

/// The non-macro half: hand pre-compiled format arguments to the VGA
/// writer. Formatting itself cannot fail and the screen cannot either,
/// so the `fmt::Result` plumbing is discharged here.
pub fn print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let _ = crate::vga::Writer.write_fmt(args);
}
