//! `ActiveStorage::Variation`: an ordered transformations hash, its Marshal-based digest (the
//! `variation_digest` column of `active_storage_variant_records`) and its signed URL key.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rails_compat::MessageVerifier;
use sha1::{Digest, Sha1};

use crate::json::Json;
use crate::marcel;
use crate::marshal::{self, Value};
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct Variation {
    transformations: Vec<(String, Value)>,
}

impl Variation {
    /// Transformations in Ruby's insertion order, e.g.
    /// `[("resize_to_limit", Array([Int(512), Int(512)])), ("format", Symbol("webp"))]`.
    pub fn new(transformations: Vec<(String, Value)>) -> Self {
        Self { transformations }
    }

    /// `resize_to_limit: [width, height]` plus an optional `format:` symbol, the only shape the
    /// app's named variants and previews use (`Message::Attachment`, `User::Avatar`, `Account`).
    pub fn resize_to_limit(width: i64, height: i64, format: Option<&str>) -> Self {
        let mut transformations = vec![("resize_to_limit".to_string(), Value::Array(vec![Value::Int(width), Value::Int(height)]))];
        if let Some(format) = format {
            transformations.push(("format".to_string(), Value::Symbol(format.to_string())));
        }
        Self { transformations }
    }

    /// `preview(format: :webp)` and friends.
    pub fn format_only(format: &str) -> Self {
        Self { transformations: vec![("format".to_string(), Value::Symbol(format.to_string()))] }
    }

    pub fn transformations(&self) -> &[(String, Value)] {
        &self.transformations
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.transformations.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn is_empty(&self) -> bool {
        self.transformations.is_empty()
    }

    /// `default_to(defaults)`: `transformations.reverse_merge(defaults)`, i.e. `defaults.merge(self)`,
    /// so default keys come first and keep their position when overridden.
    pub fn default_to(&self, defaults: &[(String, Value)]) -> Variation {
        let mut merged: Vec<(String, Value)> = defaults.to_vec();
        for (key, value) in &self.transformations {
            match merged.iter_mut().find(|(k, _)| k == key) {
                Some(entry) => entry.1 = value.clone(),
                None => merged.push((key.clone(), value.clone())),
            }
        }
        Variation { transformations: merged }
    }

    pub fn marshal(&self) -> Vec<u8> {
        marshal::dump(&Value::Hash(self.transformations.clone()))
    }

    /// `OpenSSL::Digest::SHA1.base64digest Marshal.dump(transformations)`.
    pub fn digest(&self) -> String {
        STANDARD.encode(Sha1::digest(self.marshal()))
    }

    /// `transformations.fetch(:format, :png)`, validated against Marcel's extension table.
    pub fn format(&self) -> Result<String> {
        let format = match self.get("format") {
            None => "png".to_string(),
            Some(Value::Symbol(s)) | Some(Value::Str(s)) => s.clone(),
            Some(other) => return Err(Error::InvalidVariation(format!("invalid format {other:?}"))),
        };
        if marcel::by_extension(&format).is_none() {
            return Err(Error::InvalidVariation(format!("invalid variant format ({format:?})")));
        }
        Ok(format)
    }

    /// `Marcel::MimeType.for(extension: format)`.
    pub fn content_type(&self) -> Result<String> {
        Ok(marcel::for_extension(&self.format()?))
    }

    /// The transformations as JSON, the way the verifier serializes them (symbols become strings).
    pub fn to_json(&self) -> Json {
        to_json(&Value::Hash(self.transformations.clone()))
    }

    /// `Variation.decode`: symbolized keys, but values stay strings.
    pub fn from_json(json: &Json) -> Result<Variation> {
        match from_json(json)? {
            Value::Hash(transformations) => Ok(Variation { transformations }),
            _ => Err(Error::InvalidVariation("transformations must be a hash".into())),
        }
    }

    /// `Variation#key`: `ActiveStorage.verifier.generate(transformations, purpose: :variation)`.
    pub fn key(&self, verifier: &MessageVerifier) -> String {
        verifier.generate_raw(&self.to_json().encode(), Some("variation"), None)
    }

    pub fn decode(verifier: &MessageVerifier, key: &str, now: jiff::Timestamp) -> Result<Variation> {
        let data = verifier.verify_raw(key, Some("variation"), now).map_err(|_| Error::InvalidSignature)?;
        Variation::from_json(&Json::parse(&data).map_err(|_| Error::InvalidSignature)?)
    }
}

fn to_json(value: &Value) -> Json {
    match value {
        Value::Nil => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(i) => Json::Int(*i),
        Value::Symbol(s) | Value::Str(s) => Json::String(s.clone()),
        Value::Array(items) => Json::Array(items.iter().map(to_json).collect()),
        Value::Hash(entries) => Json::Object(entries.iter().map(|(k, v)| (k.clone(), to_json(v))).collect()),
    }
}

fn from_json(json: &Json) -> Result<Value> {
    Ok(match json {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::Bool(*b),
        Json::Int(i) => Value::Int(*i),
        Json::Float(_) => return Err(Error::InvalidVariation("float transformation arguments are unsupported".into())),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(items) => Value::Array(items.iter().map(from_json).collect::<Result<_>>()?),
        Json::Object(entries) => Value::Hash(entries.iter().map(|(k, v)| Ok((k.clone(), from_json(v)?))).collect::<Result<_>>()?),
    })
}
