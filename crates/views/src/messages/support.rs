//! Small Ruby/Rails behaviors the message and room views depend on: time formats and numbers as
//! Ruby prints them.

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

/// A number as Ruby prints it: integers bare, floats as `Float#to_s` writes them, always with a
/// fractional part (`600.0`) and in exponent form outside `1e-4..1e15` (`1.0e+15`).
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
            RubyNumber::Float(value) => f.write_str(&ruby_compat::float_to_s(*value)),
        }
    }
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
    fn formats_numbers_like_ruby() {
        assert_eq!(RubyNumber::Float(600.0).to_string(), "600.0");
        assert_eq!(RubyNumber::Float(16.0 / 9.0).to_string(), "1.7777777777777777");
        assert_eq!(RubyNumber::Float(1e15).to_string(), "1.0e+15");
        assert_eq!(RubyNumber::Float(0.00001).to_string(), "1.0e-05");
        assert_eq!(RubyNumber::Int(641).half().to_string(), "320");
    }

    #[test]
    fn formats_json_times_with_milliseconds() {
        let time: Timestamp = "2026-09-26T12:26:46.848999Z".parse().unwrap();
        assert_eq!(json_time(time), "2026-09-26T12:26:46.848Z");
    }
}
