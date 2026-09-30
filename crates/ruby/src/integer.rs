use crate::string::SPACE;

/// `String#to_i`, saturating at the i64 bounds where Ruby goes on to a Bignum.
pub fn to_i(s: &str) -> i64 {
    to_i128(s).clamp(i64::MIN.into(), i64::MAX.into()) as i64
}

/// `String#to_i`, or `None` where Ruby's answer doesn't fit in an i64.
pub fn to_i_checked(s: &str) -> Option<i64> {
    i64::try_from(to_i128(s)).ok()
}

/// A string as Active Record binds it for an integer column (`find`, `find_by(id:)`, `where`):
/// `ActiveModel::Type::Integer#serialize`, with the SQLite adapter's 8-byte limit. nil unless it
/// starts like a number (`/\A\s*[+-]?\d/`), then `to_i`, and nil out of range, where Rails raises
/// (activemodel's `type/integer.rb`).
pub fn integer_cast(s: &str) -> Option<i64> {
    let unsigned = s.trim_start_matches(SPACE);
    let unsigned = unsigned.strip_prefix(['+', '-']).unwrap_or(unsigned);
    if !unsigned.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    to_i_checked(s)
}

/// `String#to_i`: optional leading whitespace and sign, an optional `0d`, then digits (an
/// underscore allowed between two). Saturating, and wide enough to tell every i64, and every file
/// size, from what's past it.
pub(crate) fn to_i128(s: &str) -> i128 {
    let s = s.trim_start_matches(SPACE);
    let (negative, rest) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let rest = rest.strip_prefix("0d").or_else(|| rest.strip_prefix("0D")).unwrap_or(rest);
    let mut number: i128 = 0;
    let mut previous_digit = false;
    for c in rest.chars() {
        match c {
            '0'..='9' => {
                number = number.saturating_mul(10).saturating_add(i128::from(c as u8 - b'0'));
                previous_digit = true;
            }
            '_' if previous_digit => previous_digit = false,
            _ => break,
        }
    }
    if negative { -number } else { number }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_i_like_ruby() {
        // `String#to_i` in the reference.
        assert_eq!(to_i("1717243200000"), 1717243200000);
        assert_eq!(to_i(" +12abc"), 12);
        assert_eq!(to_i("\t\n\u{b}\u{c}\r 7"), 7);
        assert_eq!(to_i("\u{a0}5"), 0);
        assert_eq!(to_i("abc"), 0);
        assert_eq!(to_i("-5"), -5);
        assert_eq!(to_i("--5"), 0);
        assert_eq!(to_i("5_6"), 56);
        assert_eq!(to_i("5__6"), 5);
        assert_eq!(to_i("_5"), 0);
        assert_eq!(to_i("0__5"), 0);
        assert_eq!(to_i("-0d5"), -5);
        assert_eq!(to_i("0d_5"), 0);
        assert_eq!(to_i("0x5"), 0);
        assert_eq!(to_i("99999999999999999999"), i64::MAX);
        assert_eq!(to_i("-99999999999999999999"), i64::MIN);
        assert_eq!(to_i_checked("99999999999999999999"), None);
        assert_eq!(to_i128("99999999999999999999"), 99999999999999999999);
    }

    #[test]
    fn casts_like_active_record() {
        // `Room.type_for_attribute(:id).serialize(s)` in the reference; a RangeError is None.
        for (value, id) in [
            ("12", Some(12)),
            ("12abc", Some(12)),
            (" -3", Some(-3)),
            (" +12abc", Some(12)),
            ("\t\n\u{b}\u{c}\r 7", Some(7)),
            ("\u{b}-0d7", Some(-7)),
            ("0d12", Some(12)),
            ("0D12", Some(12)),
            ("0d", Some(0)),
            ("0d_5", Some(0)),
            ("0d0_5", Some(5)),
            ("5_6", Some(56)),
            ("5__6", Some(5)),
            ("1_000", Some(1000)),
            ("0x5", Some(0)),
            ("-0", Some(0)),
            ("5\0", Some(5)),
            ("3000000000", Some(3000000000)),
            ("9223372036854775807", Some(i64::MAX)),
            ("-9223372036854775808", Some(i64::MIN)),
            ("9223372036854775808", None),
            ("-9223372036854775809", None),
            ("99999999999999999999", None),
            ("\u{a0}5", None),
            ("\0 5", None),
            ("_5", None),
            ("--5", None),
            ("+-5", None),
            ("+", None),
            ("abc", None),
            ("", None),
        ] {
            assert_eq!(integer_cast(value), id, "{value:?}");
        }
    }
}
