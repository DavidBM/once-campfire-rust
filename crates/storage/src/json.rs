//! Order-preserving JSON, encoded the way `ActiveSupport::JSON.encode` does.
//!
//! Blob metadata (`ActiveRecord::Coders::JSON`) and the Active Storage verifier payloads are
//! Ruby hashes, so key order is part of the stored bytes and the signed messages. This small
//! owned value type keeps that order, and gives `Hash#[]=`/`Hash#merge` and an integer/float
//! split that callers match on (a video's `1920.0` width renders differently from an image's
//! `1920`). `serde_json::Value` with the workspace's `preserve_order` could hold the same data.

use std::fmt::{self, Write};

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn object() -> Json {
        Json::Object(Vec::new())
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// `Hash#[]=`: replaces an existing key in place, or appends a new one.
    pub fn set(&mut self, key: &str, value: Json) {
        if let Json::Object(entries) = self {
            match entries.iter_mut().find(|(k, _)| k == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key.to_string(), value)),
            }
        }
    }

    /// `Hash#merge`: keys of `other` overwrite in place, new keys are appended in order.
    pub fn merge(&mut self, other: &Json) {
        if let Json::Object(entries) = other {
            for (key, value) in entries {
                self.set(key, value.clone());
            }
        }
    }

    /// `ActiveSupport::JSON.encode`.
    pub fn encode(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => write!(out, "{i}").unwrap(),
            Json::Float(f) => out.push_str(&encode_float(*f)),
            Json::String(s) => encode_string(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    encode_string(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Self {
        Json::String(s.to_string())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Self {
        Json::String(s)
    }
}

impl From<i64> for Json {
    fn from(i: i64) -> Self {
        Json::Int(i)
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Self {
        Json::Bool(b)
    }
}

/// ActiveSupport escapes HTML-significant characters on top of the JSON gem's escaping. It leaves
/// U+2028 and U+2029 raw: `load_defaults` 8.1+ turns `escape_js_separators_in_json` off.
fn encode_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A finite float as `ActiveSupport::JSON.encode` writes it, which is not `Float#to_s`: json
/// 2.21.2's `fpconv_dtoa` (`ext/json/ext/vendor/fpconv.c`, `emit_digits`) writes `1e15` as
/// `1e+15` and `1e-5` as `0.00001`. Non-finite floats are `null` (`Float#as_json`).
///
/// The digits are the shortest that round-trip. fpconv's Grisu2 now and then writes a longer
/// equivalent (`250.70174600000001` for `250.701746`), which parses to the same float.
pub fn encode_float(f: f64) -> String {
    if !f.is_finite() {
        return "null".to_string();
    }
    let sign = if f.is_sign_negative() { "-" } else { "" };
    if f == 0.0 {
        return format!("{sign}0.0");
    }
    // `{:e}` yields the shortest round-trip digits as `d.ddde<exponent>`; fpconv's `K` is the
    // power of ten of the last digit.
    let sci = format!("{:e}", f.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = exponent + 1 - digits.len() as i32;

    let body = if k >= 0 && exponent < 15 {
        format!("{digits}{}.0", "0".repeat(k as usize))
    } else if k < 0 && (k > -7 || exponent.abs() < 10) {
        let point = exponent + 1;
        if point <= 0 {
            format!("0.{}{digits}", "0".repeat(-point as usize))
        } else {
            format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
        }
    } else {
        let fraction = if digits.len() > 1 { format!(".{}", &digits[1..]) } else { String::new() };
        format!("{}{fraction}e{}{}", &digits[..1], if exponent < 0 { '-' } else { '+' }, exponent.abs())
    };
    format!("{sign}{body}")
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;

        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = Json;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any JSON value")
            }

            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }

            fn visit_bool<E>(self, b: bool) -> Result<Json, E> {
                Ok(Json::Bool(b))
            }

            fn visit_i64<E>(self, i: i64) -> Result<Json, E> {
                Ok(Json::Int(i))
            }

            fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Json, E> {
                i64::try_from(u).map(Json::Int).map_err(|_| E::custom("integer out of range"))
            }

            fn visit_f64<E>(self, f: f64) -> Result<Json, E> {
                Ok(Json::Float(f))
            }

            fn visit_str<E>(self, s: &str) -> Result<Json, E> {
                Ok(Json::String(s.to_string()))
            }

            fn visit_string<E>(self, s: String) -> Result<Json, E> {
                Ok(Json::String(s))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Json::Array(items))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
                let mut entries: Vec<(String, Json)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Json>()? {
                    // Ruby's JSON.parse keeps the last duplicate, at the first one's position.
                    match entries.iter_mut().find(|(k, _)| *k == key) {
                        Some(entry) => entry.1 = value,
                        None => entries.push((key, value)),
                    }
                }
                Ok(Json::Object(entries))
            }
        }

        deserializer.deserialize_any(JsonVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_match_active_support_json() {
        // ActiveSupport::JSON.encode(f) in the reference (json 2.21.2).
        for (f, json) in [
            (320.0, "320.0"),
            (65.84, "65.84"),
            (-2.5, "-2.5"),
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.00001, "0.00001"),
            (1.25e-5, "0.0000125"),
            (1.5e-7, "0.00000015"),
            (-1.5e-7, "-0.00000015"),
            (1e-7, "0.0000001"),
            (1.2e-9, "0.0000000012"),
            (1.23456789012e-8, "0.0000000123456789012"),
            (1e-10, "1e-10"),
            (5e-324, "5e-324"),
            (1e14, "100000000000000.0"),
            (-1e14, "-100000000000000.0"),
            (123456789012345.6, "123456789012345.6"),
            (1e15, "1e+15"),
            (-1e15, "-1e+15"),
            (1.5e15, "1.5e+15"),
            (1234567890123456.0, "1.234567890123456e+15"),
            (9007199254740992.0, "9.007199254740992e+15"),
            (1e16, "1e+16"),
            (12345678901234567.0, "1.2345678901234568e+16"),
            (1e21, "1e+21"),
            (1e100, "1e+100"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(encode_float(f), json, "{f:e}");
        }
        assert_eq!(encode_float(f64::NAN), "null");
    }

    #[test]
    fn leaves_js_line_separators_raw() {
        // ActiveSupport::JSON.encode and ActiveRecord::Coders::JSON in the reference.
        assert_eq!(Json::from("a\u{2028}b\u{2029}c<>&").encode(), "\"a\u{2028}b\u{2029}c\\u003c\\u003e\\u0026\"");
    }

    #[test]
    fn preserves_order_and_escapes_like_active_support() {
        let json = Json::parse(r#"{"z":1,"a":[true,null,2.0],"s":"<a & b>"}"#).unwrap();
        assert_eq!(json.encode(), r#"{"z":1,"a":[true,null,2.0],"s":"\u003ca \u0026 b\u003e"}"#);
    }
}
