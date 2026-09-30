//! Rails' SQLite datetime encoding.
//!
//! Rails writes `datetime(6)` columns as UTC text, `"%Y-%m-%d %H:%M:%S"` followed by
//! `".%06d"` microseconds only when the microseconds are non-zero
//! (`ActiveRecord::ConnectionAdapters::Quoting#quoted_date`). Values are truncated, not
//! rounded, to microseconds on assignment (`ActiveModel::Type::Helpers::TimeValue`).
//! `insert_all` instead lets SQLite stamp rows with `STRFTIME('%Y-%m-%d %H:%M:%f', 'NOW')`,
//! which has millisecond precision; see [`SQLITE_NOW`].

use std::fmt;

use jiff::{SignedDuration, Timestamp as JiffTimestamp};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

/// The timestamp expression Rails' `insert_all` uses on SQLite for `created_at`/`updated_at`.
pub const SQLITE_NOW: &str = "STRFTIME('%Y-%m-%d %H:%M:%f', 'NOW')";

/// A UTC instant with microsecond precision, stored the way Active Record stores it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(JiffTimestamp);

impl Timestamp {
    pub fn from_jiff(ts: JiffTimestamp) -> Self {
        Self(JiffTimestamp::from_microsecond(ts.as_microsecond()).expect("in range"))
    }

    pub fn from_microsecond(us: i64) -> Self {
        Self(JiffTimestamp::from_microsecond(us).expect("in range"))
    }

    pub fn from_second(s: i64) -> Self {
        Self(JiffTimestamp::from_second(s).expect("in range"))
    }

    pub fn jiff(self) -> JiffTimestamp {
        self.0
    }

    pub fn as_microsecond(self) -> i64 {
        self.0.as_microsecond()
    }

    pub fn as_second(self) -> i64 {
        self.0.as_second()
    }

    pub fn subsec_microsecond(self) -> i32 {
        self.0.subsec_microsecond()
    }

    /// `1.hour.ago`-style arithmetic.
    pub fn ago(self, duration: SignedDuration) -> Self {
        Self::from_jiff(self.0 - duration)
    }

    pub fn since(self, duration: SignedDuration) -> Self {
        Self::from_jiff(self.0 + duration)
    }

    /// The exact text Active Record writes to SQLite.
    pub fn to_db(self) -> String {
        let base = self.0.strftime("%Y-%m-%d %H:%M:%S").to_string();
        match self.0.subsec_microsecond() {
            0 => base,
            us => format!("{base}.{us:06}"),
        }
    }

    /// Parses what Rails (or SQLite's `STRFTIME`) wrote: `YYYY-MM-DD HH:MM:SS[.fraction]`,
    /// also tolerating a `T` separator and a trailing `Z` or ` UTC`.
    pub fn parse_db(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_suffix(" UTC").or_else(|| text.strip_suffix('Z')).unwrap_or(text);
        if text.len() < 19 || !text.is_char_boundary(19) {
            return None;
        }
        let (whole, fraction) = text.split_at(19);
        let b = whole.as_bytes();
        if b[4] != b'-' || b[7] != b'-' || !(b[10] == b' ' || b[10] == b'T') || b[13] != b':' || b[16] != b':' {
            return None;
        }
        let num = |range: std::ops::Range<usize>| whole.get(range)?.parse::<i32>().ok();
        let date = jiff::civil::Date::new(num(0..4)? as i16, num(5..7)? as i8, num(8..10)? as i8).ok()?;
        let micros = match fraction.strip_prefix('.') {
            Some(digits) if !digits.is_empty() && digits.bytes().all(|d| d.is_ascii_digit()) => {
                let mut padded: String = digits.chars().take(6).collect();
                while padded.len() < 6 {
                    padded.push('0');
                }
                padded.parse::<i32>().ok()?
            }
            None if fraction.is_empty() => 0,
            _ => return None,
        };
        let time = jiff::civil::Time::new(num(11..13)? as i8, num(14..16)? as i8, num(17..19)? as i8, micros * 1000).ok()?;
        let datetime = jiff::civil::DateTime::from_parts(date, time);
        let zoned = datetime.to_zoned(jiff::tz::TimeZone::UTC).ok()?;
        Some(Self(zoned.timestamp()))
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_db())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_db())
    }
}

impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_db()))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        Timestamp::parse_db(text).ok_or_else(|| FromSqlError::Other(format!("invalid datetime {text:?}").into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impossible_dates_dont_parse() {
        assert!(Timestamp::parse_db("2026-13-01 00:00:00").is_none());
        assert!(Timestamp::parse_db("2026-02-30 00:00:00").is_none());
    }

    #[test]
    fn encodes_like_active_record() {
        let ts = Timestamp::parse_db("2026-09-26 12:34:56.123456").unwrap();
        assert_eq!(ts.to_db(), "2026-09-26 12:34:56.123456");
        assert_eq!(Timestamp::parse_db("2026-09-26 12:34:56").unwrap().to_db(), "2026-09-26 12:34:56");
        assert_eq!(Timestamp::parse_db("2026-09-26 12:34:56.120000").unwrap().to_db(), "2026-09-26 12:34:56.120000");
        assert_eq!(Timestamp::parse_db("2026-01-01 00:00:00.000001").unwrap().to_db(), "2026-01-01 00:00:00.000001");
    }

    #[test]
    fn reads_sqlite_strftime_milliseconds() {
        assert_eq!(Timestamp::parse_db("2026-09-26 12:25:26.826").unwrap().to_db(), "2026-09-26 12:25:26.826000");
    }

    #[test]
    fn truncates_to_microseconds() {
        let jiff = JiffTimestamp::new(1_700_000_000, 123_456_999).unwrap();
        assert_eq!(Timestamp::from_jiff(jiff).subsec_microsecond(), 123_456);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Timestamp::parse_db("yesterday").is_none());
        assert!(Timestamp::parse_db("2026-09-26 12:34:56.x").is_none());
    }
}
