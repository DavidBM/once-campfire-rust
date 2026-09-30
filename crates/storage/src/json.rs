//! Order-preserving JSON, encoded by `rails_compat::json` as `ActiveSupport::JSON.encode` does.
//!
//! Blob metadata (`ActiveRecord::Coders::JSON`) and the Active Storage verifier payloads are
//! Ruby hashes, so key order is part of the stored bytes and the signed messages. This small
//! owned value type keeps that order, and gives `Hash#[]=`/`Hash#merge` and an integer/float
//! split that callers match on (a video's `1920.0` width renders differently from an image's
//! `1920`). `serde_json::Value` with the workspace's `preserve_order` could hold the same data.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Serialize, Serializer};

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
        rails_compat::json::encode(self)
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

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(b) => serializer.serialize_bool(*b),
            Json::Int(i) => serializer.serialize_i64(*i),
            Json::Float(f) => serializer.serialize_f64(*f),
            Json::String(s) => serializer.serialize_str(s),
            Json::Array(items) => serializer.collect_seq(items),
            Json::Object(entries) => serializer.collect_map(entries.iter().map(|(key, value)| (key, value))),
        }
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
    fn leaves_js_line_separators_raw() {
        // ActiveSupport::JSON.encode and ActiveRecord::Coders::JSON in the reference.
        assert_eq!(Json::from("a\u{2028}b\u{2029}c<>&").encode(), "\"a\u{2028}b\u{2029}c\\u003c\\u003e\\u0026\"");
    }

    #[test]
    fn preserves_order_and_escapes_like_active_support() {
        let json = Json::parse(r#"{"z":1,"a":[true,null,2.0,1e16,1e-5],"s":"<a & b>"}"#).unwrap();
        assert_eq!(json.encode(), r#"{"z":1,"a":[true,null,2.0,1e+16,0.00001],"s":"\u003ca \u0026 b\u003e"}"#);
    }
}
