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

/// Ruby's `Float#to_s`, which ActiveSupport uses for finite floats: the shortest round-trip
/// digits, always with a fractional part, switching to `e` notation outside 1e-4...1e16.
pub fn encode_float(f: f64) -> String {
    if !f.is_finite() {
        return "null".to_string();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    // `{:e}` yields the shortest round-trip digits as `d.ddde<exp>`.
    let sci = format!("{:e}", f.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if f < 0.0 { "-" } else { "" };

    if (-4..16).contains(&exponent) {
        let point = exponent + 1;
        let body = if point <= 0 {
            format!("0.{}{}", "0".repeat((-point) as usize), digits)
        } else if point as usize >= digits.len() {
            format!("{}{}.0", digits, "0".repeat(point as usize - digits.len()))
        } else {
            format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
        };
        format!("{sign}{body}")
    } else {
        let fraction = if digits.len() > 1 { &digits[1..] } else { "0" };
        format!("{sign}{}.{}e{}{:02}", &digits[..1], fraction, if exponent < 0 { '-' } else { '+' }, exponent.abs())
    }
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
    fn floats_match_ruby() {
        assert_eq!(encode_float(320.0), "320.0");
        assert_eq!(encode_float(65.84), "65.84");
        assert_eq!(encode_float(0.0001), "0.0001");
        assert_eq!(encode_float(0.00001), "1.0e-05");
        assert_eq!(encode_float(1e16), "1.0e+16");
        assert_eq!(encode_float(1234567890123456.0), "1234567890123456.0");
        assert_eq!(encode_float(-2.5), "-2.5");
        assert_eq!(encode_float(1.5e-7), "1.5e-07");
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
