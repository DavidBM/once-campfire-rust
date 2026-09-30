//! Active Storage route helpers (`activestorage/config/routes.rb`) and blob signed ids.
//!
//! `rails_blob_path` and `url_for(representation)` resolve through
//! `resolve_model_to_route = :rails_storage_redirect`; `rails_storage_proxy_path` gives the
//! proxy variants. `urls_expire_in` is unset in Campfire, so these signed ids never expire.

use crate::blob::Blob;
use crate::disposition::{escape_path, escape_segment};
use crate::filename::Filename;
use crate::json::Json;
use crate::variation::Variation;
use crate::verifier::Verifier;

pub const PREFIX: &str = "/rails/active_storage";

/// `blob.signed_id`: the Active Storage verifier with purpose "blob_id".
pub fn signed_blob_id(verifier: &dyn Verifier, blob_id: i64, expires_at: Option<jiff::Timestamp>) -> String {
    verifier.generate(&blob_id.to_string(), "blob_id", expires_at)
}

/// `ActiveStorage::Blob.find_signed(id)`'s verification half.
pub fn verify_signed_blob_id(verifier: &dyn Verifier, signed_id: &str, now: jiff::Timestamp) -> Option<i64> {
    Json::parse(&verifier.verified(signed_id, "blob_id", now)?).ok()?.as_i64()
}

/// `rails_blob_path(blob, disposition:)` → `/rails/active_storage/blobs/redirect/:signed_id/*filename`.
pub fn blob_redirect_path(verifier: &dyn Verifier, blob: &Blob, disposition: Option<&str>) -> String {
    blob_path("redirect", verifier, blob.id, &blob.filename, disposition)
}

/// `rails_storage_proxy_path(blob)` → `/rails/active_storage/blobs/proxy/:signed_id/*filename`.
pub fn blob_proxy_path(verifier: &dyn Verifier, blob: &Blob, disposition: Option<&str>) -> String {
    blob_path("proxy", verifier, blob.id, &blob.filename, disposition)
}

/// `url_for(blob.representation(...))`/`url_for(variant)`/`url_for(preview)` → the path of
/// `/rails/active_storage/representations/redirect/:signed_blob_id/:variation_key/*filename`.
/// `blob` is the *original* blob (the video for previews), `variation` is exactly the one the
/// representation holds (defaulted for variants, as given for previews).
pub fn representation_redirect_path(verifier: &dyn Verifier, blob: &Blob, variation: &Variation) -> String {
    representation_path("redirect", verifier, blob, variation)
}

pub fn representation_proxy_path(verifier: &dyn Verifier, blob: &Blob, variation: &Variation) -> String {
    representation_path("proxy", verifier, blob, variation)
}

fn blob_path(kind: &str, verifier: &dyn Verifier, blob_id: i64, filename: &Filename, disposition: Option<&str>) -> String {
    let mut path = format!(
        "{PREFIX}/blobs/{kind}/{}/{}",
        escape_segment(&signed_blob_id(verifier, blob_id, None)),
        escape_path(&filename.sanitized())
    );
    if let Some(disposition) = disposition {
        path.push_str("?disposition=");
        path.push_str(&query_escape(disposition));
    }
    path
}

fn representation_path(kind: &str, verifier: &dyn Verifier, blob: &Blob, variation: &Variation) -> String {
    format!(
        "{PREFIX}/representations/{kind}/{}/{}/{}",
        escape_segment(&signed_blob_id(verifier, blob.id, None)),
        escape_segment(&variation.key(verifier)),
        escape_path(&blob.filename.sanitized())
    )
}

/// `CGI.escape` as `Hash#to_query` applies it: everything but `a-zA-Z0-9_.-~` is escaped, and a
/// space is `+`.
fn query_escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b' ' => out.push('+'),
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_escape_is_cgi_escape() {
        // `CGI.escape("a*~ b-._")` in the reference; `rails_service_blob_path(..., disposition:
        // "a*~ b")` ends in `?disposition=a%2A~+b`.
        assert_eq!(query_escape("a*~ b-._"), "a%2A~+b-._");
        assert_eq!(query_escape("attachment"), "attachment");
    }
}
