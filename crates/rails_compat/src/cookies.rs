//! `cookies.signed[...]` / `cookies.encrypted[...]` values (`action_dispatch/middleware/cookies.rb`),
//! plus Rack's escaping of cookie values on the wire. Attributes (path, expires, HttpOnly,
//! SameSite) are the HTTP layer's job.
//!
//! The functions here work on the *raw* jar value. On the wire Rack escapes it with
//! `URI.encode_www_form_component` ([`escape`]) and unescapes incoming cookies ([`unescape`]).
//!
//! How the jars work, per Rails main:
//! - the value is first dumped with `cookies_serializer` (`:json` here, so `ActiveSupport::JSON`),
//! - then signed (HMAC-**SHA1**, key `generate_key("signed cookie")`) or encrypted (aes-256-gcm,
//!   key `generate_key("authenticated encrypted cookie", 32)`) with the legacy metadata envelope
//!   carrying `pur: "cookie.<name>"` and `exp` (ISO 8601 with milliseconds, or `null`),
//! - reading tries purpose `cookie.<name>` first, then *no purpose*, so a value signed without
//!   metadata (pre-Rails 5.2) is accepted under any cookie name.
use jiff::{Timestamp, ToSpan, tz::TimeZone};
use serde_json::Value;

use crate::message_verifier::{Digest, Encoding};
use crate::metadata::Serializer;
use crate::{MessageEncryptor, MessageVerifier, Secrets, json};

pub const SIGNED_COOKIE_SALT: &str = "signed cookie";
pub const AUTHENTICATED_ENCRYPTED_COOKIE_SALT: &str = "authenticated encrypted cookie";

/// `cookies.permanent`: expires 20 years from now (calendar years, like `20.years.from_now`).
pub fn permanent_expires_at(now: Timestamp) -> Timestamp {
    now.to_zoned(TimeZone::UTC).checked_add(20.years()).expect("20 years from now is in range").timestamp()
}

/// The raw value for `cookies.signed[name] = { value:, expires: expires_at }`.
/// `cookies.signed.permanent[...]` is `expires_at: Some(permanent_expires_at(now))`.
pub fn sign(secrets: &Secrets, name: &str, value: &str, expires_at: Option<Timestamp>) -> String {
    let dumped = json::encode(&Value::String(value.to_string()));
    signed_cookie_verifier(secrets).generate(&Value::String(dumped), Some(&purpose(name)), expires_at)
}

/// Reads `cookies.signed[name]`; `None` wherever Rails returns nil. A value that is valid JSON
/// but not a string (Rails would return it) is also `None`.
pub fn verify_signed(secrets: &Secrets, name: &str, raw: &str, now: Timestamp) -> Option<String> {
    match verify_signed_value(secrets, name, raw, now)? {
        Value::String(s) => Some(s),
        _ => None,
    }
}

/// Reads `cookies.signed[name]` as whatever JSON value it holds.
pub fn verify_signed_value(secrets: &Secrets, name: &str, raw: &str, now: Timestamp) -> Option<Value> {
    let verifier = signed_cookie_verifier(secrets);
    let dumped = verifier.verify(raw, Some(&purpose(name)), now).or_else(|_| verifier.verify(raw, None, now)).ok()?;
    load(dumped)
}

/// The raw value for `cookies.encrypted[name] = { value:, expires: expires_at }`.
/// The session store writes `_campfire_session` this way with a 20-year `expire_after`.
pub fn encrypt(secrets: &Secrets, name: &str, value: &Value, expires_at: Option<Timestamp>) -> String {
    let dumped = json::encode(value);
    encrypted_cookie_encryptor(secrets).encrypt_and_sign(&Value::String(dumped), Some(&purpose(name)), expires_at)
}

/// Reads `cookies.encrypted[name]`; `None` wherever Rails returns nil.
pub fn decrypt(secrets: &Secrets, name: &str, raw: &str, now: Timestamp) -> Option<Value> {
    let encryptor = encrypted_cookie_encryptor(secrets);
    let dumped =
        encryptor.decrypt_and_verify(raw, Some(&purpose(name)), now).or_else(|_| encryptor.decrypt_and_verify(raw, None, now)).ok()?;
    load(dumped)
}

fn purpose(name: &str) -> String {
    format!("cookie.{name}")
}

/// `SerializerWithFallback[:json].load`: Marshal payloads aren't allowed for cookies.
fn load(dumped: Value) -> Option<Value> {
    let Value::String(dumped) = dumped else { return None };
    Serializer::JsonWithFallback { allow_marshal: false }.load(dumped.as_bytes()).ok()
}

pub fn signed_cookie_verifier(secrets: &Secrets) -> MessageVerifier {
    // `signed_cookie_digest` is unset, so the jar falls back to "SHA1" even though the key itself
    // is derived with PBKDF2-SHA256.
    let secret = secrets.key_generator.generate_key(SIGNED_COOKIE_SALT, 64);
    MessageVerifier::new(secret, Digest::Sha1, Encoding::Strict, Serializer::Null)
}

pub fn encrypted_cookie_encryptor(secrets: &Secrets) -> MessageEncryptor {
    let secret = secrets.key_generator.generate_key(AUTHENTICATED_ENCRYPTED_COOKIE_SALT, 32);
    MessageEncryptor::new(&secret, Serializer::Null)
}

/// `Rack::Utils.escape` (`URI.encode_www_form_component`), which Rack applies to every cookie
/// value it writes: `*-._` and alphanumerics stay, a space becomes `+`, the rest is `%XX`. That's
/// the form encoding the `form_urlencoded` crate writes.
pub fn escape(raw: &str) -> String {
    form_urlencoded::byte_serialize(raw.as_bytes()).collect()
}

/// `Rack::Utils.parse_cookies_header`'s `unescape(value) rescue value`: `+` is a space, `%XX` is
/// decoded, and a malformed escape leaves the value untouched.
pub fn unescape(wire: &str) -> String {
    let bytes = wire.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => match bytes.get(i + 1..i + 3).filter(|hex| hex.iter().all(u8::is_ascii_hexdigit)) {
                Some(hex) => {
                    out.push(u8::from_str_radix(std::str::from_utf8(hex).expect("hex digits"), 16).expect("hex digits"));
                    i += 2;
                }
                None => return wire.to_string(),
            },
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
