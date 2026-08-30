#[allow(dead_code)]
pub unsafe fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

#[allow(dead_code)]
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
    unsafe { core::str::from_utf8_unchecked(&buf[i..]) }
}

pub fn parse_u32(text: &str) -> Option<u32> {
    let text = text.trim();
    let (digits, radix) = match text.strip_prefix("0x").or(text.strip_prefix("0X")) {
        Some(rest) => (rest, 16u32),
        None => (text, 10u32),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: u32 = 0;
    for byte in digits.bytes() {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        } as u32;
        if digit >= radix {
            return None;
        }
        value = value.checked_mul(radix)?.checked_add(digit)?;
    }
    Some(value)
}
