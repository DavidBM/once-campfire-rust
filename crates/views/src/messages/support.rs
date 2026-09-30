//! Small Ruby/Rails behaviors the message and room views depend on: time formats and
//! `Float#to_s`.

use jiff::Timestamp;

/// `time.iso8601` for a UTC `ActiveSupport::TimeWithZone`: seconds precision, `Z` suffix.
pub fn iso8601(time: Timestamp) -> String {
    time.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// `time.to_fs(:epoch)`, defined in `reference/config/initializers/time_formats.rb` as
/// `(time.to_f * 1000).to_i`. The float round trip is deliberate: it truncates some
/// millisecond values down by one, and the client compares these numbers.
pub fn epoch_ms(time: Timestamp) -> i64 {
    // `Time#to_f` is the nearest double to the exact rational, which parsing the decimal
    // representation gives us.
    let seconds = time.as_second();
    let nanos = time.subsec_nanosecond();
    let decimal = if seconds < 0 && nanos != 0 {
        let whole = seconds + 1;
        let frac = 1_000_000_000 - nanos;
        format!("{}{}.{:09}", if whole == 0 { "-" } else { "" }, whole, frac)
    } else {
        format!("{seconds}.{nanos:09}")
    };
    let to_f: f64 = decimal.parse().unwrap_or(seconds as f64);
    (to_f * 1000.0) as i64
}

/// `time.as_json` with Active Support's default precision: `2026-09-26T12:26:46.848Z`.
pub fn json_time(time: Timestamp) -> String {
    let millis = time.subsec_nanosecond() / 1_000_000;
    format!("{}.{millis:03}Z", time.strftime("%Y-%m-%dT%H:%M:%S"))
}

/// A number as Ruby prints it: integers bare, floats always with a fractional part
/// (`600.0`) and in exponent form outside `1e-4..1e16` (`1.0e+16`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum RubyNumber {
    Int(i64),
    Float(f64),
}

impl RubyNumber {
    pub fn to_f(self) -> f64 {
        match self {
            RubyNumber::Int(value) => value as f64,
            RubyNumber::Float(value) => value,
        }
    }

    /// `number / 2`: integer division for integers.
    pub fn half(self) -> RubyNumber {
        match self {
            RubyNumber::Int(value) => RubyNumber::Int(value.div_euclid(2)),
            RubyNumber::Float(value) => RubyNumber::Float(value / 2.0),
        }
    }
}

impl std::fmt::Display for RubyNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RubyNumber::Int(value) => write!(f, "{value}"),
            RubyNumber::Float(value) => f.write_str(&ruby_float(*value)),
        }
    }
}

/// `Float#to_s`: plain decimals from 1e-4 up to (not including) 1e15, and above that while the
/// shortest digits still reach past the decimal point (`1000000000000000.1`); the exponent form
/// otherwise (`flo_to_s` in Ruby 3.4's numeric.c).
pub fn ruby_float(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let (digits, decpt) = shortest_digits(value.abs());
    let sign = if value < 0.0 { "-" } else { "" };
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
    fn epoch_truncates_through_a_float_like_ruby() {
        let time: Timestamp = "2026-09-26T12:23:46.483521Z".parse().unwrap();
        assert_eq!(epoch_ms(time), 1790425426483);
        let time: Timestamp = "2026-09-26T11:23:46Z".parse().unwrap();
        assert_eq!(epoch_ms(time), 1790421826000);
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
            assert_eq!(ruby_float(f), s, "{f:e}");
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
            assert_eq!(ruby_float(f), s, "{f:e}");
        }
    }

    #[test]
    fn formats_floats_like_ruby() {
        assert_eq!(ruby_float(600.0), "600.0");
        assert_eq!(ruby_float(1.0), "1.0");
        assert_eq!(ruby_float(16.0 / 9.0), "1.7777777777777777");
        assert_eq!(ruby_float(1e16), "1.0e+16");
        assert_eq!(ruby_float(0.00001), "1.0e-05");
        assert_eq!(RubyNumber::Int(641).half().to_string(), "320");
    }

    #[test]
    fn formats_json_times_with_milliseconds() {
        let time: Timestamp = "2026-09-26T12:26:46.848999Z".parse().unwrap();
        assert_eq!(json_time(time), "2026-09-26T12:26:46.848Z");
    }
}
