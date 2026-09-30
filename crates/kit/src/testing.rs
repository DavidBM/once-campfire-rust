//! Test support: an insecure, transparent [`Crypto`] and a frozen clock.
//!
//! `TestCrypto` has the same *shape* as Rails' (signed values are `data--digest`, encrypted values
//! are opaque) but none of the byte compatibility, which `rails_compat` owns. Only compiled for
//! tests: this crate's own, and others' through the `test-support` feature.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use jiff::Timestamp;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::clock::{SharedClock, TestClock};
use crate::crypto::{Crypto, SharedCrypto};

pub const TEST_TIME: &str = "2024-06-01T12:00:00Z";

pub fn crypto() -> SharedCrypto {
    Arc::new(TestCrypto::default())
}

pub fn frozen_clock() -> SharedClock {
    Arc::new(TestClock::frozen_at(TEST_TIME.parse().unwrap()))
}

#[derive(Debug, Clone)]
pub struct TestCrypto {
    secret: String,
}

impl Default for TestCrypto {
    fn default() -> Self {
        Self { secret: "test-secret".into() }
    }
}

impl TestCrypto {
    fn digest(&self, parts: &[&str]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.secret.as_bytes());
        for part in parts {
            hasher.update(b"\0");
            hasher.update(part.as_bytes());
        }
        hex::encode(&hasher.finalize()[..16])
    }

    fn envelope(&self, kind: &str, name: &str, value: Value, expires_at: Option<Timestamp>) -> String {
        let payload = json!({ "v": value, "exp": expires_at.map(|t| t.to_string()), "pur": format!("cookie.{name}") });
        let data = STANDARD.encode(payload.to_string());
        let digest = self.digest(&[kind, &data]);
        format!("{data}--{digest}")
    }

    fn open(&self, kind: &str, name: &str, raw: &str, now: Timestamp) -> Option<Value> {
        let (data, digest) = raw.rsplit_once("--")?;
        if self.digest(&[kind, data]) != digest {
            return None;
        }
        let payload: Value = serde_json::from_slice(&STANDARD.decode(data).ok()?).ok()?;
        if payload["pur"] != format!("cookie.{name}") {
            return None;
        }
        if let Some(exp) = payload["exp"].as_str()
            && exp.parse::<Timestamp>().ok()? <= now
        {
            return None;
        }
        Some(payload["v"].clone())
    }
}

impl Crypto for TestCrypto {
    fn sign_cookie(&self, name: &str, value: &str, expires_at: Option<Timestamp>) -> String {
        self.envelope("signed", name, Value::String(value.into()), expires_at)
    }

    fn verify_signed_cookie(&self, name: &str, raw: &str, now: Timestamp) -> Option<String> {
        self.open("signed", name, raw, now)?.as_str().map(str::to_string)
    }

    fn encrypt_cookie(&self, name: &str, value: &Value, expires_at: Option<Timestamp>) -> String {
        // A random prefix so repeated encryptions differ, like AES-GCM with a fresh IV.
        let nonce: u32 = rand::random();
        format!("{nonce:08x}{}", self.envelope("encrypted", name, value.clone(), expires_at))
    }

    fn decrypt_cookie(&self, name: &str, raw: &str, now: Timestamp) -> Option<Value> {
        self.open("encrypted", name, raw.get(8..)?, now)
    }
}
