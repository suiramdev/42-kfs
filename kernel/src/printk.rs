macro_rules! printk {
    ($($arg:tt)*) => {
        $crate::printk::print(core::format_args!($($arg)*))
    };
}
pub(crate) use printk;

pub fn print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let _ = crate::vga::Writer.write_fmt(args);
}
