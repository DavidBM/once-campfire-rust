use crate::string::SPACE;

/// `String#to_f` (`rb_cstr_to_dbl` in Ruby 3.4's object.c, which reads up to the first NUL): the
/// number after any leading whitespace, and 0.0 when there's none. An underscore between two
/// digits is skipped, so `"1_000.5"` is 1000.5; `"1.2.3"` is 1.2 and `"1e2"` 100.0. Hexadecimal
/// is read only after a sign: `"-0x1A"` is -26.0, and `"0x1A"` 0.0.
pub fn to_f(s: &str) -> f64 {
    let s = s.split('\0').next().unwrap_or_default().trim_start_matches(SPACE).as_bytes();
    if is_hex(s) {
        return 0.0;
    }
    let (value, end) = strtod(s);
    if end == 0 || end == s.len() {
        return value;
    }
    let number = without_underscores(s, end);
    if is_hex(&number) { 0.0 } else { strtod(&number).0 }
}

fn is_hex(s: &[u8]) -> bool {
    s.first() == Some(&b'0') && matches!(s.get(1), Some(b'x' | b'X'))
}

/// A number's significand fills at most this much of `rb_cstr_to_dbl`'s buffer (`DBL_DIG * 4`),
/// and it all at most this much (`sizeof(buf) - 1`).
const SIGNIFICAND_WIDTH: usize = 60;
const BUFFER_WIDTH: usize = 69;

/// What `rb_cstr_to_dbl` hands `strtod` a second time when the first read stopped short at `end`:
/// the number copied into a fixed buffer with the underscores between digits left out, up to
/// anything else that can't continue it. Significand characters past the buffer's first 60 are
/// dropped, not scaled, so `"1" + "0" * 70 + "x"` is 1e59.
fn without_underscores(s: &[u8], end: usize) -> Vec<u8> {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut out = Vec::with_capacity(BUFFER_WIDTH);
    let mut width = SIGNIFICAND_WIDTH;
    let (mut base, mut exponent_letter) = (10, b'e');
    let mut dot_seen = false;
    let mut previous = 0;
    let mut p = 0;
    if matches!(at(p), b'+' | b'-') {
        previous = at(p);
        out.push(previous);
        p += 1;
    }
    if at(p) == b'0' {
        previous = b'0';
        out.push(b'0');
        p += 1;
        if matches!(at(p), b'x' | b'X') {
            previous = b'x';
            out.push(b'x');
            (base, exponent_letter) = (16, b'p');
            p += 1;
        }
        // Successive zeros are squeezed into the one.
        while at(p) == b'0' {
            p += 1;
        }
    }
    while p < end && out.len() < width {
        previous = s[p];
        out.push(previous);
        p += 1;
    }
    while p < s.len() {
        if s[p] == b'_' {
            p += 1;
            if out.is_empty() || !is_digit(previous, base) || !is_digit(at(p), base) {
                break;
            }
        }
        previous = s[p];
        p += 1;
        if width == SIGNIFICAND_WIDTH && previous.to_ascii_lowercase() == exponent_letter {
            width = BUFFER_WIDTH;
            out.push(previous);
            if matches!(at(p), b'+' | b'-') {
                previous = at(p);
                out.push(previous);
                p += 1;
            }
            if at(p) == b'0' {
                previous = b'0';
                out.push(b'0');
                while at(p) == b'0' {
                    p += 1;
                }
            }
            base = 10;
            continue;
        } else if is_space(previous) {
            while is_space(at(p)) {
                p += 1;
            }
            if p < s.len() {
                break;
            }
        } else if previous == b'.' {
            if std::mem::replace(&mut dot_seen, true) {
                break;
            }
        } else if !is_digit(previous, base) {
            break;
        }
        if out.len() < width {
            out.push(previous);
        }
    }
    out
}

fn is_digit(byte: u8, base: u32) -> bool {
    char::from(byte).is_digit(base)
}

fn is_space(byte: u8) -> bool {
    SPACE.contains(&char::from(byte))
}

/// The most significant digits `ruby_strtod` reads of a fraction (`DBL_DIG * 4`): past them
/// the rest is ignored. Zeros count once a digit follows them.
const FRACTION_DIGITS: usize = 60;

/// `ruby_strtod` (David Gay's, in Ruby's missing/dtoa.c): the number at the start of `s`, and how
/// much of `s` it takes up (0 for none). Rust parses the digits it reads; both round correctly.
fn strtod(s: &[u8]) -> (f64, usize) {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut i = 0;
    while is_space(at(i)) {
        i += 1;
    }
    let negative = at(i) == b'-';
    if matches!(at(i), b'+' | b'-') {
        i += 1;
    }
    if i == s.len() {
        return (0.0, 0);
    }
    if at(i) == b'0' && matches!(at(i + 1), b'x' | b'X') {
        return hex_strtod(s, i + 2, negative);
    }
    let signed_zero = if negative { -0.0 } else { 0.0 };

    let leading_zero = at(i) == b'0';
    while at(i) == b'0' {
        i += 1;
    }
    if leading_zero && i == s.len() {
        return (signed_zero, i);
    }
    let integer_start = i;
    while at(i).is_ascii_digit() {
        i += 1;
    }
    let integer = &s[integer_start..i];
    let mut digits = integer.len();
    let mut fraction = Vec::new();
    let mut fraction_zeros = false;
    if at(i) == b'.' && at(i + 1).is_ascii_digit() {
        i += 1;
        let mut zeros = 0;
        while at(i).is_ascii_digit() {
            let digit = at(i);
            i += 1;
            if digits > FRACTION_DIGITS {
                continue;
            } else if digit == b'0' {
                zeros += 1;
                fraction_zeros = true;
                continue;
            }
            fraction.extend(std::iter::repeat_n(b'0', zeros));
            fraction.push(digit);
            digits += if digits == 0 { 1 } else { zeros + 1 };
            zeros = 0;
        }
    } else if at(i) == b'.' {
        i += 1;
    }
    let any_digits = digits > 0 || fraction_zeros || leading_zero;

    let mut exponent = 0;
    if matches!(at(i), b'e' | b'E') {
        if !any_digits {
            return (0.0, 0);
        }
        (exponent, i) = strtod_exponent(s, i);
    }
    if digits == 0 {
        return if any_digits { (signed_zero, i) } else { (0.0, 0) };
    }
    let mut number = String::with_capacity(integer.len() + fraction.len() + 8);
    if negative {
        number.push('-');
    }
    number.extend(integer.iter().map(|&b| char::from(b)));
    number.push('.');
    number.extend(fraction.iter().map(|&b| char::from(b)));
    number.push_str(&format!("e{exponent}"));
    (number.parse().unwrap_or(signed_zero), i)
}

/// The exponent at `s[e]` (the `e`), and where it ends; `s[e]` itself when no digit follows. Past
/// 19999 it's 19999, as in Ruby.
fn strtod_exponent(s: &[u8], e: usize) -> (i32, usize) {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut i = e + 1;
    let negative = at(i) == b'-';
    if matches!(at(i), b'+' | b'-') {
        i += 1;
    }
    if !at(i).is_ascii_digit() {
        return (0, e);
    }
    while at(i) == b'0' {
        i += 1;
    }
    let start = i;
    let mut exponent: i64 = 0;
    while at(i).is_ascii_digit() {
        exponent = exponent.saturating_mul(10).saturating_add(i64::from(at(i) - b'0'));
        i += 1;
    }
    let exponent = if i - start > 8 || exponent > 19999 { 19999 } else { exponent as i32 };
    (if negative { -exponent } else { exponent }, i)
}

/// `ruby_strtod`'s hexadecimal branch, from the digits after `0x` at `start`: hex digits, a
/// fraction and a binary exponent (`p`), added up as doubles the way Ruby does, then scaled.
fn hex_strtod(s: &[u8], start: usize, negative: bool) -> (f64, usize) {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let hex = |i: usize| char::from(at(i)).to_digit(16).map(f64::from);
    let signed = |value: f64| if negative { -value } else { value };
    let mut i = start;
    if hex(i).is_none() && at(i) != b'.' {
        return (0.0, 0);
    }
    let (mut sum, mut weight, mut exponent) = (0.0, 1.0, -4_i64);
    while at(i) == b'0' {
        i += 1;
    }
    if i == s.len() {
        return (signed(0.0), i);
    }
    while let Some(digit) = hex(i) {
        sum += weight * digit;
        exponent += 4;
        weight /= 16.0;
        i += 1;
    }
    if at(i) == b'.' {
        i += 1;
        if hex(i).is_some() {
            if exponent < 0 {
                while at(i) == b'0' {
                    i += 1;
                    exponent -= 4;
                }
            }
            while let Some(digit) = hex(i) {
                sum += weight * digit;
                i += 1;
                weight /= 16.0;
                if weight == 0.0 {
                    while hex(i).is_some() {
                        i += 1;
                    }
                    break;
                }
            }
        }
    }
    if matches!(at(i), b'p' | b'P') {
        i += 1;
        let sign = match at(i) {
            b'-' => -1,
            b'+' => 1,
            _ => 0,
        };
        if sign != 0 {
            i += 1;
        }
        let sign = if sign == 0 { 1 } else { sign };
        if !at(i).is_ascii_digit() {
            return (0.0, 0);
        }
        let mut power: i64 = 0;
        while at(i).is_ascii_digit() {
            power = power * 10 + i64::from(at(i) - b'0');
            i += 1;
            // Ruby stops reading the exponent past this, where any significand overflows.
            if power + sign * exponent > 2095 {
                while at(i).is_ascii_digit() {
                    i += 1;
                }
                break;
            }
        }
        exponent += power * sign;
    }
    (signed(scale_by_two(sum, exponent)), i)
}

/// `ldexp(value, exponent)`: `value` times 2 to the `exponent`, rounded once (musl's `scalbn`).
fn scale_by_two(value: f64, exponent: i64) -> f64 {
    let power_of_two = |n: i64| f64::from_bits(((0x3ff + n) as u64) << 52);
    let (mut value, mut n) = (value, exponent);
    if n > 1023 {
        value *= power_of_two(1023);
        n -= 1023;
        if n > 1023 {
            value *= power_of_two(1023);
            n = (n - 1023).min(1023);
        }
    } else if n < -1022 {
        // Scaled so that the last step, into the subnormals, is the only one that rounds.
        value *= power_of_two(-1022) * power_of_two(53);
        n += 1022 - 53;
        if n < -1022 {
            value *= power_of_two(-1022) * power_of_two(53);
            n = (n + 1022 - 53).max(-1022);
        }
    }
    value * power_of_two(n)
}

/// `Float#to_s`: plain decimals from 1e-4 up to (not including) 1e15, and above that while the
/// shortest digits still reach past the decimal point (`1000000000000000.1`); the exponent form
/// otherwise (`flo_to_s` in Ruby 3.4's numeric.c).
pub fn float_to_s(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let (digits, decpt) = shortest_digits(f.abs());
    let sign = if f < 0.0 { "-" } else { "" };
    if decpt < -3 || (decpt > 15 && digits.len() as i32 <= decpt) {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        let e = decpt - 1;
        format!("{sign}{first}.{rest}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else if decpt <= 0 {
        format!("{sign}0.{}{}", "0".repeat((-decpt) as usize), digits)
    } else if decpt as usize >= digits.len() {
        format!("{sign}{}{}.0", digits, "0".repeat(decpt as usize - digits.len()))
    } else {
        let (int, frac) = digits.split_at(decpt as usize);
        format!("{sign}{int}.{frac}")
    }
}

/// The shortest digits that read back as `magnitude`, and where the decimal point goes in them.
/// When two such forms are equally close, Ruby's dtoa takes the even one and Rust the upper one:
/// `667020902720176.25.to_s` is "667020902720176.2". Those ties take 16 or 17 digits, and Rust's
/// fixed-precision formatting breaks them to even.
fn shortest_digits(magnitude: f64) -> (String, i32) {
    let shortest = scientific_digits(&format!("{magnitude:e}"));
    let (digits, _) = &shortest;
    if digits.len() >= 16 && digits.ends_with(['1', '3', '5', '7', '9']) {
        let even = format!("{:.*e}", digits.len() - 1, magnitude);
        if even.parse::<f64>() == Ok(magnitude) {
            return scientific_digits(&even);
        }
    }
    shortest
}

/// The digits of `1.2345e6` and where the decimal point goes in them (7).
fn scientific_digits(formatted: &str) -> (String, i32) {
    let (mantissa, exponent) = formatted.split_once('e').unwrap();
    let digits = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    (digits, exponent.parse::<i32>().unwrap() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_f_like_ruby() {
        // `String#to_f` in the reference.
        for (s, f) in [
            ("0.5", 0.5),
            ("0.5.1", 0.5),
            ("1.5.5e2", 1.5),
            ("1e2", 100.0),
            ("1E2", 100.0),
            ("1.2e-1", 0.12),
            ("1.5e+2", 150.0),
            ("1.e5", 100000.0),
            ("1e2.5", 100.0),
            ("1e0_1", 10.0),
            ("1e", 1.0),
            ("1e+", 1.0),
            ("0.5e-", 0.5),
            ("1e_2", 1.0),
            ("1_0.5", 10.5),
            ("1_2_3.4_5", 123.45),
            ("0.5_5", 0.55),
            ("0_0.5", 0.5),
            ("1__0", 1.0),
            ("1_e2", 1.0),
            ("1._5", 1.0),
            ("_1", 0.0),
            (".5", 0.5),
            ("+.5", 0.5),
            ("-.5", -0.5),
            ("5.", 5.0),
            ("00.5", 0.5),
            ("  -1.5x", -1.5),
            ("\u{b}0.5", 0.5),
            ("1,5", 1.0),
            ("-0", -0.0),
            ("0x1A", 0.0),
            ("-0x1A", -26.0),
            ("-0x1.8p1", -3.0),
            (".e5", 0.0),
            ("e5", 0.0),
            (".", 0.0),
            ("-", 0.0),
            ("+", 0.0),
            ("", 0.0),
            ("Infinity", 0.0),
            ("1e-400", 0.0),
            ("1e400", f64::INFINITY),
            ("1\x002", 1.0),
        ] {
            assert_eq!(to_f(s).to_bits(), f64::to_bits(f), "{s:?}");
        }
    }

    #[test]
    fn to_f_drops_what_ruby_has_no_room_for() {
        // Significand characters past 60, when there's more after the number.
        assert_eq!(to_f(&format!("1{}x", "0".repeat(70))), 1e59);
        assert_eq!(to_f(&format!("1{}", "0".repeat(70))), 1e70);
        // Fraction digits past 60 significant ones: halfway between two doubles, but not seen as
        // halfway, so this rounds down rather than to the even one.
        let halfway = "0.00000000093132257461547882581772970738537807677825952623607008717954158782958984375";
        assert_eq!(to_f(halfway), f64::from_bits(0x3E10_0000_0000_0001));
    }

    #[test]
    fn float_to_s_like_ruby_3_4() {
        // `Float#to_s` in the reference (Ruby 3.4.10): from 1e15, the exponent form unless there
        // are digits after the decimal point.
        for (f, s) in [
            (100.0, "100.0"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.00012345, "0.00012345"),
            (1e-5, "1.0e-05"),
            (1.5e-7, "1.5e-07"),
            (5e-324, "5.0e-324"),
            (1e14, "100000000000000.0"),
            (123456789012345.6, "123456789012345.6"),
            (999999999999999.0, "999999999999999.0"),
            (999999999999999.9, "999999999999999.9"),
            (-999999999999999.0, "-999999999999999.0"),
            (1e15, "1.0e+15"),
            (-1e15, "-1.0e+15"),
            (1.5e15, "1.5e+15"),
            (1234567890123456.0, "1.234567890123456e+15"),
            (9007199254740992.0, "9.007199254740992e+15"),
            (1000000000000001.0, "1.000000000000001e+15"),
            (1963684456584958.8, "1963684456584958.8"),
            (1000000000000000.1, "1000000000000000.1"),
            (2251799813685248.5, "2251799813685248.5"),
            (-2551800308696183.5, "-2551800308696183.5"),
            (1e16, "1.0e+16"),
            (1e20, "1.0e+20"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(float_to_s(f), s, "{f:e}");
        }
    }

    #[test]
    fn float_to_s_breaks_ties_to_even_like_ruby() {
        // `Float#to_s` in the reference: of two shortest forms equally close, the even one, as long
        // as it reads back. Doubles are twice as dense just below 2^-24, so its even form
        // (5.960464477539062e-08) reads back as a different double.
        for (f, s) in [
            (667020902720176.0 + 0.25, "667020902720176.2"),
            (667020902720176.0 + 0.75, "667020902720176.8"),
            (1000000000000000.2, "1000000000000000.2"),
            (1125899906842624.0 + 0.25, "1125899906842624.2"),
            (-(2074704973491874.0 + 0.25), "-2074704973491874.2"),
            (24603114260468.0 + 0.0625, "24603114260468.062"),
            (210745403561986.0 + 0.125, "210745403561986.12"),
            (2f64.powi(-24), "5.960464477539063e-08"),
            (2f64.powi(-25), "2.9802322387695312e-08"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
        ] {
            assert_eq!(float_to_s(f), s, "{f:e}");
        }
    }
}
