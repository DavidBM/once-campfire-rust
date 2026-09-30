/// Ruby's `ISSPACE`: what `String#to_i` and `#to_f` skip, and `\s` in a regexp.
pub(crate) const SPACE: [char; 6] = [' ', '\t', '\n', '\u{b}', '\u{c}', '\r'];

/// `String#strip`: NUL and ASCII whitespace off both ends (not Unicode spaces like U+00A0).
pub fn strip(s: &str) -> &str {
    s.trim_matches(|c| c == '\0' || SPACE.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_nul_and_ascii_whitespace() {
        assert_eq!(strip("\0 \t\u{b} x \0\n"), "x");
        assert_eq!(strip(" \u{a0}x\u{a0} "), "\u{a0}x\u{a0}");
    }
}
