//! Active Storage's `Content-Disposition` for a blob, and the Journey path escaping used for the
//! `*filename` glob in Active Storage routes.

/// `ActiveStorage::Service#content_disposition_with`: anything but "attachment" is "inline".
pub fn content_disposition_with(disposition: &str, sanitized_filename: &str) -> String {
    let disposition = if disposition == "attachment" { "attachment" } else { "inline" };
    rails_compat::content_disposition::format(disposition, sanitized_filename)
}

fn percent_escape(s: &str, keep: impl Fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();
        if bytes.len() == 1 && keep(bytes[0]) {
            out.push(c);
        } else {
            for b in bytes {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// `Journey::Router::Utils.escape_path`: keeps unreserved, sub-delims, ":", "@" and "/".
pub fn escape_path(s: &str) -> String {
    percent_escape(s, |b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&b))
}

/// `Journey::Router::Utils.escape_segment`: like `escape_path`, but "/" is escaped too.
pub fn escape_segment(s: &str) -> String {
    percent_escape(s, |b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&b))
}
