//! `ActionDispatch::Cookies::CookieJar` with its plain, permanent, signed and encrypted jars.
//!
//! Semantics follow `action_dispatch/middleware/cookies.rb`:
//! - setting a cookie only emits `Set-Cookie` when the value changed or an expiry was given;
//! - every cookie gets `path=/` and `samesite=lax` unless told otherwise
//!   (`cookies_same_site_protection = :lax` from `load_defaults 6.1`);
//! - `permanent` means an `expires` 20 calendar years out, embedded in signed/encrypted metadata;
//! - deleting only emits a header when the cookie was present;
//! - secure cookies are dropped on plain-HTTP requests;
//! - signed/encrypted values over 4096 bytes (name included) raise `CookieOverflow`.
//!
//! Header formatting is `Rack::Utils.set_cookie_header` (Rack 3.2).

use std::sync::Arc;

use jiff::Timestamp;
use rails_compat::Secrets;
use serde_json::Value;

use crate::clock::{self, SharedClock};
use crate::{Error, Result};

pub use rails_compat::cookies::escape;

pub const MAX_COOKIE_SIZE: usize = 4096;
const PERMANENT_YEARS: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Lax,
    Strict,
    None,
}

/// A cookie to set, with Rails' option names.
#[derive(Debug, Clone, PartialEq)]
pub struct Cookie {
    pub value: String,
    pub path: String,
    pub domain: Option<String>,
    pub expires: Option<Timestamp>,
    pub permanent: bool,
    pub secure: bool,
    pub httponly: bool,
    pub same_site: Option<SameSite>,
    pub partitioned: bool,
}

impl Cookie {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            path: "/".into(),
            domain: None,
            expires: None,
            permanent: false,
            secure: false,
            httponly: false,
            same_site: Some(SameSite::Lax),
            partitioned: false,
        }
    }

    /// `cookies.permanent[...]`: expires 20 years from now.
    pub fn permanent(mut self) -> Self {
        self.permanent = true;
        self
    }

    pub fn expires(mut self, at: Timestamp) -> Self {
        self.expires = Some(at);
        self
    }

    pub fn httponly(mut self) -> Self {
        self.httponly = true;
        self
    }

    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn same_site(mut self, same_site: Option<SameSite>) -> Self {
        self.same_site = same_site;
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }
}

impl From<&str> for Cookie {
    fn from(value: &str) -> Self {
        Cookie::new(value)
    }
}

impl From<String> for Cookie {
    fn from(value: String) -> Self {
        Cookie::new(value)
    }
}

/// Options for `cookies.delete(name, options)`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteOptions {
    pub path: String,
    pub domain: Option<String>,
    pub same_site: Option<SameSite>,
}

impl Default for DeleteOptions {
    fn default() -> Self {
        Self { path: "/".into(), domain: None, same_site: Some(SameSite::Lax) }
    }
}

pub struct CookieJar {
    /// The current value of every cookie: the request's, updated by sets and deletes.
    cookies: Vec<(String, String)>,
    set_cookies: Vec<(String, Cookie)>,
    delete_cookies: Vec<(String, DeleteOptions)>,
    secrets: Arc<Secrets>,
    clock: SharedClock,
}

impl std::fmt::Debug for CookieJar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieJar")
            .field("cookies", &self.cookies)
            .field("set_cookies", &self.set_cookies)
            .field("delete_cookies", &self.delete_cookies)
            .finish()
    }
}

impl CookieJar {
    /// Build the jar from the request's `Cookie` header(s).
    pub fn from_headers<'a>(headers: impl IntoIterator<Item = &'a str>, secrets: Arc<Secrets>, clock: SharedClock) -> Self {
        let mut cookies: Vec<(String, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for header in headers {
            for (name, value) in parse_cookie_header(header) {
                if seen.insert(name.clone()) {
                    cookies.push((name, value));
                }
            }
        }
        Self { cookies, set_cookies: vec![], delete_cookies: vec![], secrets, clock }
    }

    /// `cookies[name]`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.cookies.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// `cookies.signed[name]`.
    pub fn signed(&self, name: &str) -> Option<String> {
        let raw = self.get(name)?;
        rails_compat::cookies::verify_signed(&self.secrets, name, raw, self.clock.now())
    }

    /// `cookies.encrypted[name]`.
    pub fn encrypted(&self, name: &str) -> Option<Value> {
        let raw = self.get(name)?;
        rails_compat::cookies::decrypt(&self.secrets, name, raw, self.clock.now())
    }

    /// `cookies[name] = value` (or `cookies.permanent[name] = ...` with [`Cookie::permanent`]).
    pub fn set(&mut self, name: &str, cookie: impl Into<Cookie>) {
        let cookie = self.resolve_expiry(cookie.into());
        self.write_value(name, cookie);
    }

    /// `cookies.signed[name] = value`.
    pub fn set_signed(&mut self, name: &str, cookie: impl Into<Cookie>) -> Result<()> {
        let mut cookie = self.resolve_expiry(cookie.into());
        cookie.value = rails_compat::cookies::sign(&self.secrets, name, &cookie.value, cookie.expires);
        check_for_overflow(name, &cookie.value)?;
        self.write_value(name, cookie);
        Ok(())
    }

    /// `cookies.encrypted[name] = value`; `cookie.value` is ignored in favor of `value`.
    pub fn set_encrypted(&mut self, name: &str, value: &Value, cookie: impl Into<Cookie>) -> Result<()> {
        let mut cookie = self.resolve_expiry(cookie.into());
        cookie.value = rails_compat::cookies::encrypt(&self.secrets, name, value, cookie.expires);
        check_for_overflow(name, &cookie.value)?;
        self.write_value(name, cookie);
        Ok(())
    }

    /// `cookies.delete(name)`: a no-op unless the cookie is present.
    pub fn delete(&mut self, name: &str) {
        self.delete_with(name, DeleteOptions::default());
    }

    pub fn delete_with(&mut self, name: &str, options: DeleteOptions) {
        let Some(index) = self.cookies.iter().position(|(n, _)| n == name) else { return };
        self.cookies.remove(index);
        upsert(&mut self.delete_cookies, name, options);
    }

    /// `cookies.deleted?(name)`.
    pub fn is_deleted(&self, name: &str) -> bool {
        self.delete_cookies.iter().any(|(n, _)| n == name)
    }

    /// The `Set-Cookie` header values Rails would write, in order.
    pub fn set_cookie_headers(&self, ssl: bool, host: &str) -> Vec<String> {
        let mut headers = Vec::new();
        for (name, cookie) in &self.set_cookies {
            if ssl || !cookie.secure || host.ends_with(".onion") {
                headers.push(set_cookie_header(name, cookie));
            }
        }
        for (name, options) in &self.delete_cookies {
            headers.push(delete_cookie_header(name, options));
        }
        headers
    }

    fn resolve_expiry(&self, mut cookie: Cookie) -> Cookie {
        if cookie.permanent {
            cookie.expires = Some(clock::years_from(self.clock.now(), PERMANENT_YEARS));
        }
        cookie
    }

    fn write_value(&mut self, name: &str, cookie: Cookie) {
        if self.get(name) != Some(cookie.value.as_str()) || cookie.expires.is_some() {
            upsert(&mut self.cookies, name, cookie.value.clone());
            upsert(&mut self.set_cookies, name, cookie);
            self.delete_cookies.retain(|(n, _)| n != name);
        }
    }
}

fn upsert<T>(list: &mut Vec<(String, T)>, name: &str, value: T) {
    match list.iter_mut().find(|(n, _)| n == name) {
        Some(slot) => slot.1 = value,
        None => list.push((name.to_string(), value)),
    }
}

fn check_for_overflow(name: &str, value: &str) -> Result<()> {
    let total = name.len() + value.len();
    if total > MAX_COOKIE_SIZE {
        return Err(Error::CookieOverflow(format!("{name} cookie overflowed with size {total} bytes")));
    }
    Ok(())
}

/// `Rack::Utils.parse_cookies_header`: split on `/; */`, first occurrence wins, values unescaped
/// (kept raw when unescaping fails).
pub fn parse_cookie_header(header: &str) -> Vec<(String, String)> {
    let mut cookies: Vec<(String, String)> = Vec::new();
    // Seen names in a set: a header of tens of thousands of cookies stays linear.
    let mut seen = std::collections::HashSet::new();
    for (i, part) in header.split(';').enumerate() {
        let part = if i == 0 { part } else { part.trim_start_matches(' ') };
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        if !seen.insert(key) {
            continue;
        }
        let value = crate::params::decode_www_form_component(value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or_else(|| value.to_string());
        cookies.push((key.to_string(), value));
    }
    cookies
}

fn same_site_attribute(same_site: Option<SameSite>) -> &'static str {
    match same_site {
        None => "",
        Some(SameSite::Lax) => "; samesite=lax",
        Some(SameSite::Strict) => "; samesite=strict",
        Some(SameSite::None) => "; samesite=none",
    }
}

/// `Rack::Utils.set_cookie_header(key, value_hash)`.
pub fn set_cookie_header(name: &str, cookie: &Cookie) -> String {
    let mut header = format!("{name}={}", escape(&cookie.value));
    if let Some(domain) = &cookie.domain {
        header.push_str(&format!("; domain={domain}"));
    }
    header.push_str(&format!("; path={}", cookie.path));
    if let Some(expires) = cookie.expires {
        header.push_str(&format!("; expires={}", clock::httpdate(expires)));
    }
    if cookie.secure {
        header.push_str("; secure");
    }
    if cookie.httponly {
        header.push_str("; httponly");
    }
    header.push_str(same_site_attribute(cookie.same_site));
    if cookie.partitioned {
        header.push_str("; partitioned");
    }
    header
}

/// `Rack::Utils.delete_set_cookie_header(key, options)`.
pub fn delete_cookie_header(name: &str, options: &DeleteOptions) -> String {
    let mut header = format!("{name}=");
    if let Some(domain) = &options.domain {
        header.push_str(&format!("; domain={domain}"));
    }
    header.push_str(&format!("; path={}; max-age=0; expires={}", options.path, clock::httpdate(Timestamp::UNIX_EPOCH)));
    header.push_str(same_site_attribute(options.same_site));
    header
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use super::*;
    use crate::clock::TestClock;

    fn jar(header: &str) -> CookieJar {
        static SECRETS: LazyLock<Arc<Secrets>> = LazyLock::new(|| Arc::new(Secrets::new("test-secret")));
        CookieJar::from_headers([header], SECRETS.clone(), Arc::new(TestClock::frozen_at("2024-06-01T12:00:00Z".parse().unwrap())))
    }

    #[test]
    fn parses_request_cookies() {
        let jar = jar("a=1; b=x%20y+z;c=3; a=2; d");
        assert_eq!(jar.get("a"), Some("1"));
        assert_eq!(jar.get("b"), Some("x y z"));
        assert_eq!(jar.get("c"), Some("3"));
        assert_eq!(jar.get("d"), Some(""));
        assert_eq!(jar.get("zz"), None);
    }

    #[test]
    fn sets_plain_cookies_with_rails_defaults() {
        let mut jar = jar("");
        jar.set("last_room", "42");
        assert_eq!(jar.set_cookie_headers(false, "example.com"), vec!["last_room=42; path=/; samesite=lax"]);
    }

    #[test]
    fn unchanged_values_are_not_rewritten_unless_expiring() {
        let mut jar = jar("last_room=42");
        jar.set("last_room", "42");
        assert!(jar.set_cookie_headers(false, "h").is_empty());
        jar.set("last_room", Cookie::new("42").permanent());
        assert_eq!(jar.set_cookie_headers(false, "h"), vec!["last_room=42; path=/; expires=Wed, 01 Jun 2044 12:00:00 GMT; samesite=lax"]);
    }

    #[test]
    fn escapes_values() {
        let mut jar = jar("");
        jar.set("x", "a b+c/=");
        assert_eq!(jar.set_cookie_headers(false, "h"), vec!["x=a+b%2Bc%2F%3D; path=/; samesite=lax"]);
    }

    #[test]
    fn signed_permanent_httponly_round_trip() {
        let mut jar = jar("");
        jar.set_signed("session_token", Cookie::new("tok").permanent().httponly()).unwrap();
        let headers = jar.set_cookie_headers(true, "h");
        assert_eq!(headers.len(), 1);
        assert!(headers[0].starts_with("session_token="));
        assert!(headers[0].ends_with("; path=/; expires=Wed, 01 Jun 2044 12:00:00 GMT; httponly; samesite=lax"));
        assert_eq!(jar.signed("session_token").as_deref(), Some("tok"));

        let raw = jar.get("session_token").unwrap().to_string();
        let next = CookieJar::from_headers([format!("session_token={}", escape(&raw)).as_str()], jar.secrets.clone(), jar.clock.clone());
        assert_eq!(next.signed("session_token").as_deref(), Some("tok"));
    }

    #[test]
    fn tampered_signed_cookies_read_as_nil() {
        let jar = jar("session_token=forged");
        assert_eq!(jar.signed("session_token"), None);
        assert_eq!(jar.get("session_token"), Some("forged"));
    }

    #[test]
    fn encrypted_round_trip() {
        let mut jar = jar("");
        jar.set_encrypted("secret", &serde_json::json!({"a": 1}), Cookie::new("")).unwrap();
        assert_eq!(jar.encrypted("secret"), Some(serde_json::json!({"a": 1})));
    }

    #[test]
    fn overflow() {
        let mut jar = jar("");
        let big = "x".repeat(MAX_COOKIE_SIZE);
        assert!(matches!(jar.set_signed("big", big.as_str()), Err(Error::CookieOverflow(_))));
    }

    #[test]
    fn delete_only_when_present() {
        let mut jar = jar("session_token=abc");
        jar.delete("missing");
        jar.delete("session_token");
        assert!(jar.is_deleted("session_token"));
        assert_eq!(jar.get("session_token"), None);
        assert_eq!(
            jar.set_cookie_headers(false, "h"),
            vec!["session_token=; path=/; max-age=0; expires=Thu, 01 Jan 1970 00:00:00 GMT; samesite=lax"]
        );
    }

    #[test]
    fn secure_cookies_need_ssl() {
        let mut jar = jar("");
        jar.set("s", Cookie::new("1").secure());
        assert!(jar.set_cookie_headers(false, "example.com").is_empty());
        assert_eq!(jar.set_cookie_headers(false, "x.onion").len(), 1);
        assert_eq!(jar.set_cookie_headers(true, "example.com"), vec!["s=1; path=/; secure; samesite=lax"]);
    }

    #[test]
    fn many_cookies_parse_in_linear_time() {
        let header: String = (0..100_000).map(|n| format!("c{n}=1")).collect::<Vec<_>>().join("; ");
        let started = std::time::Instant::now();
        assert_eq!(parse_cookie_header(&header).len(), 100_000);
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "{:?}", started.elapsed());
    }
}
