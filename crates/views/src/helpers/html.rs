//! `ERB::Util.html_escape` and `ActiveSupport::SafeBuffer` semantics.
//!
//! Templates escape with [`ErbEscaper`] (configured in `askama.toml` for html, svg and json), so
//! output bytes match ERB's `&amp; &lt; &gt; &quot; &#39;`. Helpers return [`Html`], askama's
//! `Safe<String>`, which templates print without escaping — the equivalent of an html_safe
//! SafeBuffer. Build helper output with [`escape`] for untrusted text and `.0` for safe parts.

use std::fmt::{self, Write};

pub use askama::filters::Safe;

/// An html_safe string.
pub type Html = Safe<String>;

/// Escapes like `ERB::Util.html_escape`.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    push_escaped(&mut out, text);
    out
}

pub fn push_escaped(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
}

/// `raw` / `String#html_safe`.
pub fn raw(html: impl AsRef<str>) -> Html {
    Safe(html.as_ref().to_string())
}

/// `h(text)` as an html_safe value.
pub fn text(text: &str) -> Html {
    Safe(escape(text))
}

pub fn empty() -> Html {
    Safe(String::new())
}

/// The askama escaper for ERB-compatible output.
#[derive(Clone, Copy, Debug, Default)]
pub struct ErbEscaper;

impl askama::filters::Escaper for ErbEscaper {
    fn write_escaped_str<W: Write>(&self, mut dest: W, string: &str) -> fmt::Result {
        let mut last = 0;
        for (index, byte) in string.bytes().enumerate() {
            let replacement = match byte {
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
                b'"' => "&quot;",
                b'\'' => "&#39;",
                _ => continue,
            };
            dest.write_str(&string[last..index])?;
            dest.write_str(replacement)?;
            last = index + 1;
        }
        dest.write_str(&string[last..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_erb_util() {
        assert_eq!(escape(r#"<&>"'x"#), "&lt;&amp;&gt;&quot;&#39;x");
    }
}
