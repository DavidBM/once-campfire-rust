//! GlobalID / SignedGlobalID (`gid://campfire/User/1`, `user.attachable_sgid`).
//!
//! SGIDs are signed by `GlobalID::Verifier` (key `generate_key("signed_global_ids")`, HMAC-SHA1,
//! URL-safe Base64 *with* padding, the app's `:json_allow_marshal` serializer, and the
//! `{"_rails":{"data":..,"exp":..,"pur":..}}` envelope). Rails 7.0 SGIDs used the legacy
//! envelope with a Marshal-dumped string, which Rails still reads.
use std::fmt;

use jiff::Timestamp;
use serde_json::Value;

use crate::message_verifier::{Digest, Encoding, MessageVerifier, Serializer};
use crate::{Secrets, encoding};

pub const SALT: &str = "signed_global_ids";
/// `GlobalID.app`, from the application name (`Campfire::Application`).
pub const APP: &str = "campfire";
/// `ActionText::Attachable::LOCATOR_NAME`.
pub const ATTACHABLE_PURPOSE: &str = "attachable";
/// `SignedGlobalID::DEFAULT_PURPOSE`.
pub const DEFAULT_PURPOSE: &str = "default";

/// A parsed `gid://app/Model/id`. Query params (Rails puts `?expires_in` into attachable SGIDs)
/// are dropped: the locator ignores them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalId {
    pub app: String,
    pub model_name: String,
    pub id: String,
}

impl GlobalId {
    pub fn new(model_name: &str, id: impl ToString) -> Self {
        Self { app: APP.to_string(), model_name: model_name.to_string(), id: id.to_string() }
    }

    /// `GlobalID.parse` for the URI form (`URI::GID`): `gid://<app>/<Model>/<id>[?params]`.
    pub fn parse(gid: &str) -> Option<GlobalId> {
        let rest = gid.strip_prefix("gid://")?;
        let rest = rest.split('?').next()?;
        let (app, path) = rest.split_once('/')?;
        let (model_name, id) = path.split_once('/')?;
        if app.is_empty() || model_name.is_empty() || id.is_empty() {
            return None;
        }
        Some(GlobalId { app: app.to_string(), model_name: model_name.to_string(), id: id.to_string() })
    }

    /// `GlobalID#to_param`: URL-safe Base64 without padding, as used in Turbo stream names.
    pub fn to_param(&self) -> String {
        encoding::urlsafe_encode_unpadded(self.to_string().as_bytes())
    }

    pub fn from_param(param: &str) -> Option<GlobalId> {
        GlobalId::parse(&String::from_utf8(encoding::urlsafe_decode(param)?).ok()?)
    }
}

impl fmt::Display for GlobalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gid://{}/{}/{}", self.app, self.model_name, self.id)
    }
}

/// `record.attachable_sgid`, i.e. `to_sgid(expires_in: nil, for: "attachable")`. GlobalID turns
/// the leftover `expires_in: nil` option into a query param, so the signed data is
/// `gid://campfire/User/1?expires_in`, with no expiry in the envelope.
pub fn attachable_sgid(secrets: &Secrets, gid: &GlobalId) -> String {
    verifier(secrets).generate(&Value::String(format!("{gid}?expires_in")), Some(ATTACHABLE_PURPOSE), None)
}

/// `SignedGlobalID.new(gid_uri, for: purpose, expires_at:)` for a bare GID URI (no params).
pub fn sgid(secrets: &Secrets, gid: &GlobalId, purpose: &str, expires_at: Option<Timestamp>) -> String {
    verifier(secrets).generate(&Value::String(gid.to_string()), Some(purpose), expires_at)
}

/// `SignedGlobalID.parse(sgid, for: purpose)`: the GID if the signature, purpose and expiry
/// check out. Looking the record up (and `only:` restrictions) is the caller's job.
pub fn locate_signed(secrets: &Secrets, sgid: &str, purpose: &str, now: Timestamp) -> Option<GlobalId> {
    let verifier = verifier(secrets);
    let data = match verifier.verify(sgid, Some(purpose), now) {
        Ok(data) => data,
        Err(_) => verify_with_legacy_self_validated_metadata(&verifier, sgid, purpose, now)?,
    };
    match data {
        Value::String(uri) => GlobalId::parse(&uri).or_else(|| GlobalId::from_param(&uri)),
        _ => None,
    }
}

/// globalid < 1.0 signed `{"gid":..,"purpose":..,"expires_at":..}` without a Rails envelope.
fn verify_with_legacy_self_validated_metadata(verifier: &MessageVerifier, sgid: &str, purpose: &str, now: Timestamp) -> Option<Value> {
    let metadata = verifier.verify(sgid, None, now).ok()?;
    let metadata = metadata.as_object()?;
    if let Some(expires_at) = metadata.get("expires_at").filter(|v| !v.is_null()) {
        let expires_at: Timestamp = expires_at.as_str()?.parse().ok()?;
        if now > expires_at {
            return None;
        }
    }
    (crate::metadata::ruby_to_s(metadata.get("purpose")) == purpose).then(|| metadata.get("gid").cloned().unwrap_or(Value::Null))
}

pub fn verifier(secrets: &Secrets) -> MessageVerifier {
    let secret = secrets.key_generator.generate_key(SALT, 64);
    MessageVerifier::new(secret, Digest::Sha1, Encoding::UrlSafePadded, Serializer::JsonWithFallback { allow_marshal: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let gid = GlobalId::parse("gid://campfire/Rooms::Open/1?expires_in").unwrap();
        assert_eq!(gid, GlobalId::new("Rooms::Open", 1));
        assert_eq!(gid.to_param(), "Z2lkOi8vY2FtcGZpcmUvUm9vbXM6Ok9wZW4vMQ");
        assert_eq!(GlobalId::from_param(&gid.to_param()), Some(gid));
        assert_eq!(GlobalId::parse("gid://campfire/User"), None);
    }
}
