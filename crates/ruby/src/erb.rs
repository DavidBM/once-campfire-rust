//! `ERB::Util.html_escape` (and `h`): `& < > " '` become `&amp; &lt; &gt; &quot; &#39;`.

use std::fmt;

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    push_html_escaped(&mut out, s);
    out
}

pub fn push_html_escaped(out: &mut String, s: &str) {
    // Writing to a String can't fail.
    let _ = write_html_escaped(out, s);
}

/// Writes the text between the escaped bytes a run at a time. The five are ASCII, so they're
/// never part of a multibyte character.
pub fn write_html_escaped<W: fmt::Write + ?Sized>(dest: &mut W, s: &str) -> fmt::Result {
    let mut last = 0;
    for (index, byte) in s.bytes().enumerate() {
        let replacement = match byte {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        dest.write_str(&s[last..index])?;
        dest.write_str(replacement)?;
        last = index + 1;
    }
    dest.write_str(&s[last..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_erb_util() {
        assert_eq!(html_escape(r#"<&>"'x é"#), "&lt;&amp;&gt;&quot;&#39;x é");
        let mut out = String::from("a");
        push_html_escaped(&mut out, "<b>");
        assert_eq!(out, "a&lt;b&gt;");
    }
}
