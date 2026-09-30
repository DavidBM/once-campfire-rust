//! Ruby's Base64 flavors as Rails uses them.
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};

/// `Base64.strict_encode64`.
pub fn strict_encode(data: &[u8]) -> String {
    STANDARD.encode(data)
}

/// `Base64.urlsafe_encode64(data, padding: false)`.
pub fn urlsafe_encode_unpadded(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

/// `Base64.urlsafe_encode64(data)` (padded).
pub fn urlsafe_encode_padded(data: &[u8]) -> String {
    URL_SAFE.encode(data)
}

/// `Base64.strict_decode64`: standard alphabet, canonical padding, no whitespace.
pub fn strict_decode(encoded: &str) -> Option<Vec<u8>> {
    STANDARD.decode(encoded).ok()
}

/// `Base64.urlsafe_decode64`, which pads a short unpadded string and then translates `-_` to
/// `+/` before a strict decode. So it accepts either alphabet (even mixed) and optional padding,
/// and it's `STANDARD` after the translation, not `URL_SAFE`, which would refuse `+` and `/`.
/// `STANDARD` also refuses partial padding (`"ab="`) and non-zero trailing bits, as Ruby does.
pub fn urlsafe_decode(encoded: &str) -> Option<Vec<u8>> {
    let mut translated: String = encoded
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    if !encoded.ends_with('=') && !encoded.len().is_multiple_of(4) {
        while !translated.len().is_multiple_of(4) {
            translated.push('=');
        }
    }
    strict_decode(&translated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlsafe_decode_is_lenient_like_ruby() {
        assert_eq!(urlsafe_decode("QQ"), Some(b"A".to_vec()));
        assert_eq!(urlsafe_decode("QQ=="), Some(b"A".to_vec()));
        assert_eq!(urlsafe_decode("QQ="), None);
        assert_eq!(urlsafe_decode("QR"), None);
        assert_eq!(urlsafe_decode("a+b/"), urlsafe_decode("a-b_"));
    }
}
