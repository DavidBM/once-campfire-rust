//! `link_to`, `link_to_if` and `mail_to` (`UrlHelper`).

use ruby_compat::url_encode;

use super::html::{Html, Safe, escape};
use super::tag::{Attrs, attrs, content_tag};

/// `link_to(url, options) { content }`: `href` goes after the given options.
pub fn link_to(url: &str, options: Attrs, content: &str) -> Html {
    let options = options.attr("href", url);
    content_tag("a", &options, content)
}

/// `link_to(text, url, options)` with a plain-text name.
pub fn link_to_text(text: &str, url: &str, options: Attrs) -> Html {
    link_to(url, options, &escape(text))
}

/// `link_to_if(condition, name, url, options)`: just the escaped name when false.
pub fn link_to_if(condition: bool, text: &str, url: &str, options: Attrs) -> Html {
    if condition { link_to_text(text, url, options) } else { Safe(escape(text)) }
}

/// `mail_to(email)`: the address percent-escaped (`ERB::Util.url_encode`, keeping "@") in the
/// href, and the plain address as the link text.
pub fn mail_to(email: &str) -> Html {
    let encoded = url_encode(email).replace("%40", "@");
    content_tag("a", attrs().attr("href", format!("mailto:{encoded}")), &escape(email))
}
