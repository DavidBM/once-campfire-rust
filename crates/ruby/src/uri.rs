/// `CGI.escape`: everything but `A-Za-z0-9_.-~` is percent-encoded, and a space becomes `+`.
pub fn cgi_escape(s: &str) -> String {
    percent_encode(s, true)
}

/// `ERB::Util.url_encode`: everything but `A-Za-z0-9_.-~` is percent-encoded, a space as `%20`.
/// Addressable's `encode_component(s, UNRESERVED)` keeps the same set.
pub fn url_encode(s: &str) -> String {
    percent_encode(s, false)
}

fn percent_encode(s: &str, space_as_plus: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(byte as char),
            b' ' if space_as_plus => out.push('+'),
            _ => {
                out.push('%');
                out.push(HEX[usize::from(byte >> 4)] as char);
                out.push(HEX[usize::from(byte & 0xf)] as char);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_ruby() {
        assert_eq!(cgi_escape("a*~ b-._+é"), "a%2A~+b-._%2B%C3%A9");
        assert_eq!(url_encode("a*~ b-._+é"), "a%2A~%20b-._%2B%C3%A9");
    }
}
