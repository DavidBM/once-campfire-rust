//! `ActionDispatch::Http::ContentDisposition.format` (actionpack's
//! `action_dispatch/http/content_disposition.rb`), as `send_file`, `send_data` and Active Storage
//! name a download: an ASCII `filename=` for old clients, transliterated with I18n's default
//! approximations, and the full name in RFC 5987's `filename*=`.
use std::fmt::Write;

#[rustfmt::skip]
mod approximations;

use approximations::APPROXIMATIONS;

/// `ContentDisposition.format(disposition:, filename:)` for a filename (without one it's just the
/// disposition).
pub fn format(disposition: &str, filename: &str) -> String {
    format!(
        "{disposition}; filename=\"{}\"; filename*=UTF-8''{}",
        percent_escape(&transliterate(filename), traditional),
        percent_escape(filename, rfc_5987)
    )
}

/// What `TRADITIONAL_ESCAPED_CHAR` leaves alone: ``[ A-Za-z0-9!#$+.^_`|~-]``.
fn traditional(byte: u8) -> bool {
    byte == b' ' || byte.is_ascii_alphanumeric() || b"!#$+.^_`|~-".contains(&byte)
}

/// What `RFC_5987_ESCAPED_CHAR` leaves alone: ``[A-Za-z0-9!#$&+.^_`|~-]``.
fn rfc_5987(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&+.^_`|~-".contains(&byte)
}

/// `percent_escape(string, pattern)`: each escaped character's bytes as `%XX`. The kept
/// characters are all ASCII, so going byte by byte is the same.
fn percent_escape(s: &str, keep: fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        if keep(byte) {
            out.push(byte as char);
        } else {
            write!(out, "%{byte:02X}").unwrap();
        }
    }
    out
}

/// `I18n.transliterate` with the default approximations, and "?" for anything else non-ASCII.
fn transliterate(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            match APPROXIMATIONS.binary_search_by(|(k, _)| k.cmp(&c)) {
                Ok(i) => out.push_str(APPROXIMATIONS[i].1),
                Err(_) => out.push('?'),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The filenames in `vectors/storage.json` (`reference-tools/storage/generate.rb`), as the
    /// reference formats them after `ActiveStorage::Filename#sanitized`.
    #[test]
    fn formats_like_rails() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors/storage.json");
        let vectors: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect(path)).unwrap();
        for f in vectors["filenames"].as_array().unwrap() {
            let sanitized = f["sanitized"].as_str().unwrap();
            assert_eq!(format("inline", sanitized), f["inline"], "{sanitized}");
            assert_eq!(format("attachment", sanitized), f["attachment"], "{sanitized}");
        }
    }

    #[test]
    fn transliterates_beyond_latin_1() {
        assert_eq!(format("inline", "Łódź ×.pdf"), "inline; filename=\"Lodz x.pdf\"; filename*=UTF-8''%C5%81%C3%B3d%C5%BA%20%C3%97.pdf");
    }
}
