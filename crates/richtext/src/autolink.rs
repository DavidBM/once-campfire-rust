//! rails_autolink 1.1.8's `auto_link(text, html: { target: "_blank" }, sanitize_options: ...)`, as
//! `MessagesHelper#message_presentation` calls it. It works on the serialized HTML with regular
//! expressions, so the exact serialization from the earlier steps matters.
//!
//! One deliberate difference closes a stored XSS in rails_autolink: the sanitized HTML is
//! serialized with `<` and `>` escaped in attribute values. Nokogiri leaves them raw, so a URL after
//! a `>` in a `title` looked like text to `auto_linked?`, and the `<a href="...">` inserted there
//! closed the attribute and turned the rest of its value into markup. With them escaped, every `<`
//! and `>` in the text is a tag's, so auto_link only ever inserts links between tags.

use regex::Regex;
use ruby_compat::erb::html_escape;
use ruby_compat::url_encode;
use std::sync::LazyLock;

use crate::dom::ParseError;
use crate::ruby::is_blank;
use crate::sanitizer::{SafeList, sanitize, sanitize_with_escaped_attribute_brackets};

/// `AUTO_LINK_RE`. Ruby's `\s` and `\w` are ASCII-only.
static AUTO_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:(?i:((?:ed2k|ftp|http|https|irc|mailto|news|gopher|nntp|telnet|webcal|xmpp|callto|feed|svn|urn|aim|rsync|tag|ssh|sftp|rtsp|afs|file):))//|(?i:www)\.[a-zA-Z0-9_])[^ \t\r\n\x0B\x0C<\u{A0}"]+"#,
    )
    .unwrap()
});

/// `AUTO_EMAIL_RE` without its lookbehind, which is checked separately.
static AUTO_EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[a-zA-Z0-9_.!#$%+-]\.?[a-zA-Z0-9_.!#$%&'*/=?^`{|}~+-]*@[a-zA-Z0-9_-]+(?:\.[a-zA-Z0-9_-]+)+").unwrap());

fn is_email_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.!#$%&'*/=?^`{|}~+-".contains(c)
}

/// `AUTO_LINK_CRE[2]`: `/<a\b.*?>/i`, where `.` stops at newlines; anchored, as it is only tried
/// at a `<`.
static OPEN_ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\A<a\b.*?>").unwrap());

/// What `auto_linked?(left, right)` asks about the text around a match, where `left` is everything
/// before it and `right` everything after. rails_autolink runs its regular expressions over the
/// whole of `left` for every match, which is quadratic (cubic for `rindex`); this indexes the text
/// once so that each question is a binary search. The answers are the same.
struct TagIndex {
    /// Positions of `<` and of `>`, ascending.
    lts: Vec<usize>,
    gts: Vec<usize>,
    /// The first `\n` that ends a line inside an unclosed tag, after which `AUTO_LINK_CRE[0]`
    /// (`/<[^>]+$/`, whose `$` also matches before a newline) matches every `left`.
    first_dangling_newline: Option<usize>,
    /// `(start, end)` of each `AUTO_LINK_CRE[2]` match: ascending and disjoint.
    open_anchors: Vec<(usize, usize)>,
    /// Positions of `</a>` (`AUTO_LINK_CRE[3]`, in any case), ascending.
    close_anchors: Vec<usize>,
}

impl TagIndex {
    fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut index = TagIndex {
            lts: Vec::new(),
            gts: Vec::new(),
            first_dangling_newline: None,
            open_anchors: Vec::new(),
            close_anchors: Vec::new(),
        };
        // The first `<` since the last `>`
        let mut unclosed_lt: Option<usize> = None;
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'<' => {
                    index.lts.push(i);
                    unclosed_lt.get_or_insert(i);
                    if bytes.get(i + 1..i + 4).is_some_and(|s| s.eq_ignore_ascii_case(b"/a>")) {
                        index.close_anchors.push(i);
                    }
                    let after_previous = index.open_anchors.last().is_none_or(|&(_, end)| end <= i);
                    if after_previous && let Some(m) = OPEN_ANCHOR_RE.find(&text[i..]) {
                        index.open_anchors.push((i, i + m.end()));
                    }
                }
                b'>' => {
                    index.gts.push(i);
                    unclosed_lt = None;
                }
                b'\n' if index.first_dangling_newline.is_none() && unclosed_lt.is_some_and(|lt| lt + 2 <= i) => {
                    index.first_dangling_newline = Some(i);
                }
                _ => {}
            }
        }
        index
    }

    /// `auto_linked?(text[..start], text[end..])`: inside a tag, or inside an unclosed `<a>`.
    fn auto_linked(&self, start: usize, end: usize) -> bool {
        (self.open_tag_at_line_end(start) && self.closes_tag(end)) || self.inside_anchor(start)
    }

    /// `left =~ /<[^>]+$/`
    fn open_tag_at_line_end(&self, start: usize) -> bool {
        if self.first_dangling_newline.is_some_and(|newline| newline < start) {
            return true;
        }
        // At the end of `left`: a `<` after the last `>`, with at least one character after it
        let last_gt = self.gts[..self.gts.partition_point(|&p| p < start)].last();
        let first_lt_after = self.lts.get(self.lts.partition_point(|&p| last_gt.is_some_and(|&gt| p <= gt)));
        first_lt_after.is_some_and(|&lt| lt + 2 <= start)
    }

    /// `right =~ /^[^>]*>/`, which matches whenever `right` has a `>` at all.
    fn closes_tag(&self, end: usize) -> bool {
        self.gts.last().is_some_and(|&gt| gt >= end)
    }

    /// `(i = left.rindex(/<a\b.*?>/i)) && left[i..] !~ /<\/a>/i`: the last `<a ...>` wholly in
    /// `left` isn't closed before `left` ends.
    fn inside_anchor(&self, start: usize) -> bool {
        let before = self.open_anchors.partition_point(|&(_, end)| end <= start);
        let Some(&(_, anchor_end)) = before.checked_sub(1).map(|i| &self.open_anchors[i]) else { return false };
        let close = self.close_anchors.get(self.close_anchors.partition_point(|&p| p < anchor_end));
        !close.is_some_and(|&p| p + 4 <= start)
    }
}

/// How many of each bracket a URL has, kept up to date as trailing punctuation is stripped (rather
/// than recounted for each character stripped, as rails_autolink does).
struct BracketCounts([usize; 6]);

impl BracketCounts {
    const BRACKETS: [char; 6] = ['[', ']', '(', ')', '{', '}'];

    fn of(s: &str) -> Self {
        let mut counts = BracketCounts([0; 6]);
        for c in s.chars() {
            counts.adjust(c, |n| n + 1);
        }
        counts
    }

    fn count(&self, bracket: char) -> usize {
        Self::BRACKETS.iter().position(|&b| b == bracket).map_or(0, |i| self.0[i])
    }

    fn remove(&mut self, c: char) {
        self.adjust(c, |n| n - 1);
    }

    fn adjust(&mut self, c: char, f: impl Fn(usize) -> usize) {
        if let Some(i) = Self::BRACKETS.iter().position(|&b| b == c) {
            self.0[i] = f(self.0[i]);
        }
    }
}

/// Brackets whose closing half may end a URL when the URL opened it.
fn opening_bracket(closing: char) -> Option<char> {
    match closing {
        ']' => Some('['),
        ')' => Some('('),
        '}' => Some('{'),
        _ => None,
    }
}

/// `auto_link(html, html: { target: "_blank" }, sanitize_options: { tags:, attributes: })`
pub fn auto_link(text: &str, sanitize_options: &SafeList) -> Result<String, ParseError> {
    if is_blank(text) {
        return Ok(String::new());
    }
    let text = sanitize_with_escaped_attribute_brackets(text, sanitize_options)?;
    let text = auto_link_urls(&text)?;
    auto_link_email_addresses(&text)
}

/// Ruby's `\p{Word}`, which is Unicode's word class (as the regex crate's `\w` is).
fn is_word_char(c: char) -> bool {
    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A\w\z").unwrap());
    WORD.is_match(c.encode_utf8(&mut [0; 4]))
}

fn auto_link_urls(text: &str) -> Result<String, ParseError> {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let tags = TagIndex::new(text);
    for caps in AUTO_LINK_RE.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        out.push_str(&text[last..whole.start()]);
        last = whole.end();
        let scheme = caps.get(1);
        let mut href = whole.as_str().to_string();
        if tags.auto_linked(whole.start(), whole.end()) {
            out.push_str(&href);
            continue;
        }
        let mut punctuation: Vec<char> = Vec::new();
        let mut brackets = BracketCounts::of(&href);
        while let Some(c) = href.chars().last().filter(|&c| !(is_word_char(c) || matches!(c, '/' | '-' | '=' | ';'))) {
            href.pop();
            punctuation.push(c);
            brackets.remove(c);
            if let Some(opening) = opening_bracket(c)
                && brackets.count(opening) > brackets.count(c)
            {
                href.push(punctuation.pop().unwrap());
                break;
            }
        }
        let mut trailing_gt = "";
        if let Some(stripped) = href.strip_suffix("&gt;") {
            href = stripped.to_string();
            trailing_gt = "&gt;";
        }
        let link_text = href.clone();
        if scheme.is_none() {
            href = format!("http://{href}");
        }
        let link_text = sanitize(&link_text, &SafeList::defaults())?;
        let href = sanitize(&href, &SafeList::defaults())?;
        // content_tag(:a, link_text, attrs, false): nothing escaped but double quotes in attributes
        out.push_str(&format!("<a target=\"_blank\" href=\"{}\">{}</a>", href.replace('"', "&quot;"), link_text));
        // SafeBuffer#+ escapes the (unsafe) punctuation string
        let trailing: String = punctuation.iter().rev().collect();
        out.push_str(&html_escape(&trailing));
        out.push_str(trailing_gt);
    }
    out.push_str(&text[last..]);
    Ok(out)
}

fn auto_link_email_addresses(text: &str) -> Result<String, ParseError> {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut position = 0;
    let tags = TagIndex::new(text);
    while position < text.len() {
        let preceded_by_local_char = text[..position].chars().last().is_some_and(is_email_local_char);
        let found = if preceded_by_local_char { None } else { AUTO_EMAIL_RE.find(&text[position..]) };
        let Some(m) = found else {
            position += text[position..].chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let (start, end) = (position + m.start(), position + m.end());
        let email = &text[start..end];
        out.push_str(&text[copied..start]);
        if tags.auto_linked(start, end) {
            out.push_str(email);
        } else {
            let sanitized = sanitize(email, &SafeList::defaults())?;
            // display_text is only sanitized (and so marked safe) when sanitizing changed the address
            let display = if sanitized == email { html_escape(email) } else { sanitize(email, &SafeList::defaults())? };
            let href = format!("mailto:{}", url_encode(&sanitized).replace("%40", "@"));
            out.push_str(&format!("<a target=\"_blank\" href=\"{}\">{}</a>", html_escape(&href), display));
        }
        copied = end;
        position = end.max(position + 1);
    }
    out.push_str(&text[copied..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// rails_autolink's `auto_linked?`, as regular expressions over the whole of `left`.
    fn auto_linked_by_regex(left: &str, right: &str) -> bool {
        static OPEN_TAG_AT_LINE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)<[^>]+$").unwrap());
        static CLOSES_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[^>]*>").unwrap());
        static OPEN_ANCHOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<a\b.*?>").unwrap());
        static CLOSE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</a>").unwrap());
        if OPEN_TAG_AT_LINE_END.is_match(left) && CLOSES_TAG.is_match(right) {
            return true;
        }
        let last = left.char_indices().rev().find_map(|(i, _)| OPEN_ANCHOR.find_at(left, i).filter(|m| m.start() == i));
        last.is_some_and(|m| !CLOSE_ANCHOR.is_match(&left[m.end()..]))
    }

    #[test]
    fn tag_index_answers_as_the_regular_expressions_do() {
        for text in [
            "<p>www.a.com</p><p>b</p>",
            "<p title=\"a\nb\">x</p> y <p>z</p>",
            "<p title=\"a\n\">x</p>",
            "<a href=\"x\">in <b>link</b></a> out <A\nhref=\"y\">z</A> <a>q</a>",
            "<ab>x</ab><a\tclass=\"c\">y",
            "x<\ny>z<é>",
            "<",
            "<a>",
            "",
        ] {
            let index = TagIndex::new(text);
            for start in (0..=text.len()).filter(|&i| text.is_char_boundary(i)) {
                for end in (start..=text.len()).filter(|&i| text.is_char_boundary(i)) {
                    assert_eq!(
                        index.auto_linked(start, end),
                        auto_linked_by_regex(&text[..start], &text[end..]),
                        "{text:?} at {start}..{end}"
                    );
                }
            }
        }
    }
}
