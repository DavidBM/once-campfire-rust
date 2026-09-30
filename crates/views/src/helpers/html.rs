//! `ERB::Util.html_escape` and `ActiveSupport::SafeBuffer` semantics.
//!
//! Templates escape with [`ErbEscaper`] (configured in `askama.toml` for html, svg and json), so
//! output bytes match ERB's `&amp; &lt; &gt; &quot; &#39;`. Helpers return [`Html`], askama's
//! `Safe<String>`, which templates print without escaping — the equivalent of an html_safe
//! SafeBuffer. Build helper output with [`escape`] for untrusted text and `.0` for safe parts.

use std::fmt::{self, Write};

pub use askama::filters::Safe;
/// `ERB::Util.html_escape`, into a new string or onto the end of one.
pub use ruby_compat::erb::{html_escape as escape, push_html_escaped as push_escaped};

/// An html_safe string.
pub type Html = Safe<String>;

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
        ruby_compat::erb::write_html_escaped(&mut dest, string)
    }
}
