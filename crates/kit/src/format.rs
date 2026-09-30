//! MIME types and format negotiation: `Mime::Type`, `Mime::Type.parse` (the `Accept` header),
//! `ActionDispatch::Http::MimeNegotiation#formats` and `respond_to`'s `negotiate_mime`.

/// A registered MIME type (`Mime[:html]` and friends).
#[derive(Debug)]
pub struct Mime {
    pub symbol: &'static str,
    pub string: &'static str,
    pub synonyms: &'static [&'static str],
    pub extensions: &'static [&'static str],
}

pub type Format = &'static Mime;

impl PartialEq for Mime {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for Mime {}

impl std::fmt::Display for Mime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.string)
    }
}

impl Mime {
    pub fn is(&self, symbol: &str) -> bool {
        self.symbol == symbol
    }

    /// `Mime::Type#match?`: `pattern` appears in the type or one of its synonyms.
    fn matches(&self, pattern: &str) -> bool {
        self.string.contains(pattern) || self.synonyms.iter().any(|s| s.contains(pattern))
    }
}

macro_rules! mime {
    ($name:ident, $symbol:literal, $string:literal, [$($syn:literal),*], [$($ext:literal),*]) => {
        pub static $name: Mime = Mime { symbol: $symbol, string: $string, synonyms: &[$($syn),*], extensions: &[$($ext),*] };
    };
}

// `action_dispatch/http/mime_types.rb`, then turbo-rails' `:turbo_stream`, in registration order
// (the order matters for `text/*` expansion).
mime!(HTML, "html", "text/html", ["application/xhtml+xml"], ["xhtml"]);
mime!(TEXT, "text", "text/plain", [], ["txt"]);
mime!(JS, "js", "text/javascript", ["application/javascript", "application/x-javascript"], []);
mime!(CSS, "css", "text/css", [], []);
mime!(ICS, "ics", "text/calendar", [], []);
mime!(CSV, "csv", "text/csv", [], []);
mime!(VCF, "vcf", "text/vcard", [], []);
mime!(VTT, "vtt", "text/vtt", [], ["vtt"]);
mime!(MD, "md", "text/markdown", [], ["md", "markdown"]);
mime!(PNG, "png", "image/png", [], ["png"]);
mime!(JPEG, "jpeg", "image/jpeg", [], ["jpg", "jpeg", "jpe", "pjpeg"]);
mime!(GIF, "gif", "image/gif", [], ["gif"]);
mime!(BMP, "bmp", "image/bmp", [], ["bmp"]);
mime!(TIFF, "tiff", "image/tiff", [], ["tif", "tiff"]);
mime!(SVG, "svg", "image/svg+xml", [], []);
mime!(WEBP, "webp", "image/webp", [], ["webp"]);
mime!(MPEG, "mpeg", "video/mpeg", [], ["mpg", "mpeg", "mpe"]);
mime!(MP3, "mp3", "audio/mpeg", [], ["mp1", "mp2", "mp3"]);
mime!(OGG, "ogg", "audio/ogg", [], ["oga", "ogg", "spx", "opus"]);
mime!(M4A, "m4a", "audio/aac", ["audio/mp4"], ["m4a", "mpg4", "aac"]);
mime!(WEBM, "webm", "video/webm", [], ["webm"]);
mime!(MP4, "mp4", "video/mp4", [], ["mp4", "m4v"]);
mime!(OTF, "otf", "font/otf", [], ["otf"]);
mime!(TTF, "ttf", "font/ttf", [], ["ttf"]);
mime!(WOFF, "woff", "font/woff", [], ["woff"]);
mime!(WOFF2, "woff2", "font/woff2", [], ["woff2"]);
mime!(XML, "xml", "application/xml", ["text/xml", "application/x-xml"], []);
mime!(RSS, "rss", "application/rss+xml", [], []);
mime!(ATOM, "atom", "application/atom+xml", [], []);
mime!(YAML, "yaml", "application/x-yaml", ["text/yaml"], ["yml", "yaml"]);
mime!(MULTIPART_FORM, "multipart_form", "multipart/form-data", [], []);
mime!(URL_ENCODED_FORM, "url_encoded_form", "application/x-www-form-urlencoded", [], []);
mime!(JSON, "json", "application/json", ["text/x-json", "application/jsonrequest", "application/problem+json"], []);
mime!(PDF, "pdf", "application/pdf", [], ["pdf"]);
mime!(ZIP, "zip", "application/zip", [], ["zip"]);
mime!(GZIP, "gzip", "application/gzip", ["application/x-gzip"], ["gz"]);
mime!(TURBO_STREAM, "turbo_stream", "text/vnd.turbo-stream.html", [], []);
/// `Mime::ALL`: the `*/*` wildcard, only meaningful in negotiation.
pub static ALL: Mime = Mime { symbol: "*/*", string: "*/*", synonyms: &[], extensions: &[] };

pub static REGISTERED: [&Mime; 37] = [
    &HTML, &TEXT, &JS, &CSS, &ICS, &CSV, &VCF, &VTT, &MD, &PNG, &JPEG, &GIF, &BMP, &TIFF, &SVG, &WEBP, &MPEG, &MP3, &OGG, &M4A, &WEBM,
    &MP4, &OTF, &TTF, &WOFF, &WOFF2, &XML, &RSS, &ATOM, &YAML, &MULTIPART_FORM, &URL_ENCODED_FORM, &JSON, &PDF, &ZIP, &GZIP, &TURBO_STREAM,
];

/// `Mime[ext]` / `Mime::Type.lookup_by_extension`.
pub fn lookup_by_extension(extension: &str) -> Option<Format> {
    REGISTERED.iter().copied().find(|m| m.symbol == extension || m.extensions.contains(&extension))
}

/// `Mime::Type.lookup`: exact type string or synonym, else the part before `;`.
/// `Ok(None)` is a valid but unregistered type; `Err` is `InvalidMimeType`.
pub fn lookup(string: &str) -> Result<Option<Format>, InvalidMimeType> {
    if let Some(mime) = lookup_exact(string) {
        return Ok(Some(mime));
    }
    let base = string.split(';').next().unwrap_or("").trim_end();
    if let Some(mime) = lookup_exact(base) {
        return Ok(Some(mime));
    }
    if base == "*/*" {
        return Ok(Some(&ALL));
    }
    if valid_mime_type(string) { Ok(None) } else { Err(InvalidMimeType(string.to_string())) }
}

fn lookup_exact(string: &str) -> Option<Format> {
    REGISTERED.iter().copied().find(|m| m.string == string || m.synonyms.contains(&string))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a valid MIME type")]
pub struct InvalidMimeType(pub String);

/// `Mime::Type::MIME_REGEXP`, loosely: `type/subtype` with name characters, optional params.
fn valid_mime_type(string: &str) -> bool {
    let base = string.split(';').next().unwrap_or("").trim_end();
    if base == "*/*" {
        return true;
    }
    let name_ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 127
            && s.as_bytes()[0].is_ascii_alphanumeric()
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&b))
    };
    match base.split_once('/') {
        Some((kind, sub)) => name_ok(kind) && (sub == "*" || name_ok(sub)),
        None => false,
    }
}

struct AcceptItem {
    index: usize,
    name: String,
    /// `(q.to_f * 100).to_i`, kept in a float so that q-values past `i64` still order as Ruby's
    /// Integers do. An infinite one, where Rails raises FloatDomainError, sorts first (or last).
    q: f64,
}

/// `Mime::Type.parse(accept_header)`, keeping only registered types and `*/*` (the
/// `formats.select!` that follows in `MimeNegotiation#formats`).
pub fn parse_accept(header: &str) -> Result<Vec<Format>, InvalidMimeType> {
    let mut formats: Vec<Format> = Vec::new();
    if !header.contains(',') {
        let header = match find_q_separator(header) {
            Some((index, _)) => header[..index].trim(),
            None => header,
        };
        if header.trim().is_empty() {
            return Ok(vec![]);
        }
        if let Some(expanded) = trailing_star(header) {
            return Ok(expanded);
        }
        return Ok(lookup(header)?.into_iter().collect());
    }

    let mut list = Vec::new();
    let mut index = 0;
    for item in scan_accept_items(header) {
        // `params, q = header.split(PARAMETER_SEPARATOR_REGEXP)`
        let fields = ruby_split(item, find_q_separator);
        let Some(params) = fields.first() else { continue };
        let q = fields.get(1).copied();
        let params = params.trim();
        if params.is_empty() {
            continue;
        }
        let names: Vec<String> = match trailing_star(params) {
            Some(expanded) => expanded.iter().map(|m| m.string.to_string()).collect(),
            None => vec![params.to_string()],
        };
        for name in names {
            let q = match q {
                Some(q) => ruby_compat::to_f(q),
                None if name == "*/*" => 0.0,
                None => 1.0,
            };
            list.push(AcceptItem { index, name, q: (q * 100.0).trunc() });
            index += 1;
        }
    }
    // -0.0 ties with 0.0, as both are Ruby's 0 (`to_f` never gives NaN).
    list.sort_by(|a, b| b.q.partial_cmp(&a.q).unwrap_or(std::cmp::Ordering::Equal).then(a.index.cmp(&b.index)));
    sort_xml(&mut list);

    for item in &list {
        if let Some(mime) = lookup(&item.name)?
            && !formats.contains(&mime)
        {
            formats.push(mime);
        }
    }
    Ok(formats)
}

/// `Mime::Type::AcceptList.sort!`'s XML juggling: `text/xml` folds into `application/xml`, which
/// then yields to any `+xml` type of the same quality listed after it.
fn sort_xml(list: &mut Vec<AcceptItem>) {
    let find = |list: &Vec<AcceptItem>, name: &str| list.iter().position(|i| i.name == name);
    let text_xml = find(list, "text/xml");
    let mut app_xml = find(list, "application/xml");
    match (text_xml, app_xml) {
        (Some(mut text_idx), Some(mut app_idx)) => {
            list[app_idx].q = list[app_idx].q.max(list[text_idx].q);
            if app_idx > text_idx {
                list.swap(app_idx, text_idx);
                std::mem::swap(&mut app_idx, &mut text_idx);
            }
            list.remove(text_idx);
            app_xml = Some(app_idx);
        }
        (Some(text_idx), None) => list[text_idx].name = "application/xml".into(),
        _ => {}
    }
    if let Some(mut app_idx) = app_xml {
        let app_q = list[app_idx].q;
        let mut idx = app_idx;
        while idx < list.len() {
            if list[idx].q < app_q {
                break;
            }
            if list[idx].name.ends_with("+xml") {
                list.swap(app_idx, idx);
                app_idx = idx;
            }
            idx += 1;
        }
    }
}

/// `PARAMETER_SEPARATOR_REGEXP = /;\s*q="?/`: where the first one starts and ends.
fn find_q_separator(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    (0..bytes.len()).find_map(|i| {
        if bytes[i] != b';' {
            return None;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if bytes.get(j) != Some(&b'q') || bytes.get(j + 1) != Some(&b'=') {
            return None;
        }
        Some((i, if bytes.get(j + 2) == Some(&b'"') { j + 3 } else { j + 2 }))
    })
}

/// `String#split` with a separator finder. It drops trailing empty fields, so a `q=` with nothing
/// after it leaves `q` nil (1.0), not "" (0.0).
fn ruby_split(s: &str, find: impl Fn(&str) -> Option<(usize, usize)>) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut rest = s;
    while let Some((start, end)) = find(rest) {
        fields.push(&rest[..start]);
        rest = &rest[end..];
    }
    fields.push(rest);
    while fields.last() == Some(&"") {
        fields.pop();
    }
    fields
}

/// `ACCEPT_HEADER_REGEXP = /[^,\s"](?:[^,"]|"[^"]*")*/`
fn scan_accept_items(header: &str) -> Vec<&str> {
    let bytes = header.as_bytes();
    let mut items = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b',' || bytes[i].is_ascii_whitespace() || bytes[i] == b'"' {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && bytes[i] != b',' {
            if bytes[i] == b'"' {
                match header[i + 1..].find('"') {
                    Some(close) => i += close + 2,
                    None => break,
                }
            } else {
                i += 1;
            }
        }
        items.push(&header[start..i]);
    }
    items
}

/// `TRAILING_STAR_REGEXP = /^(text|application)\/\*/`: every registered type matching `text/`
/// or `application/`, in registration order.
fn trailing_star(accept: &str) -> Option<Vec<Format>> {
    let kind = ["text/*", "application/*"].into_iter().find(|p| accept.starts_with(p))?;
    let prefix = &kind[..kind.len() - 1];
    Some(REGISTERED.iter().copied().filter(|m| m.matches(prefix)).collect())
}

/// Everything `MimeNegotiation#formats` looks at.
#[derive(Debug, Default, Clone)]
pub struct NegotiationInput<'a> {
    /// `params[:format]` (path extension captured by the router, or `?format=`).
    pub format_param: Option<&'a str>,
    pub accept: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub path: &'a str,
    pub xhr: bool,
}

/// `request.formats`.
pub fn formats(input: &NegotiationInput) -> Result<Vec<Format>, InvalidMimeType> {
    if let Some(format) = input.format_param {
        return Ok(lookup_by_extension(format).into_iter().collect());
    }
    if valid_accept_header(input) {
        let accept = input.accept.unwrap_or("").trim();
        return if accept.is_empty() { Ok(content_mime_type(input.content_type)?.into_iter().collect()) } else { parse_accept(accept) };
    }
    if let Some(format) = format_from_path_extension(input.path) {
        return Ok(vec![format]);
    }
    Ok(vec![if input.xhr { &JS } else { &HTML }])
}

/// `request.content_mime_type`.
pub fn content_mime_type(content_type: Option<&str>) -> Result<Option<Format>, InvalidMimeType> {
    match content_type {
        Some(ct) => {
            let base = ct.split([',', ';']).next().unwrap_or("").trim().to_ascii_lowercase();
            if base.is_empty() { Ok(None) } else { lookup(&base) }
        }
        None => Ok(None),
    }
}

/// `request.should_apply_vary_header?`: `!params_readable? && use_accept_header &&
/// valid_accept_header`, i.e. the format came from the `Accept` header, not a format param.
pub fn should_apply_vary_header(input: &NegotiationInput) -> bool {
    input.format_param.is_none() && valid_accept_header(input)
}

fn valid_accept_header(input: &NegotiationInput) -> bool {
    let accept = input.accept.unwrap_or("");
    let present = !accept.trim().is_empty();
    (input.xhr && (present || input.content_type.is_some_and(|ct| !ct.is_empty()))) || (present && !browser_like(accept))
}

/// `BROWSER_LIKE_ACCEPTS = /,\s*\*\/\*|\*\/\*\s*,/`
fn browser_like(accept: &str) -> bool {
    let compact: String = accept.split(char::is_whitespace).collect();
    compact.contains(",*/*") || compact.contains("*/*,")
}

fn format_from_path_extension(path: &str) -> Option<Format> {
    let (_, ext) = path.rsplit_once('.')?;
    if ext.is_empty() || !ext.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    lookup_by_extension(ext)
}

/// `request.negotiate_mime(order)`: the first acceptable format `order` offers.
pub fn negotiate(formats: &[Format], order: &[Format]) -> Option<Format> {
    for &priority in formats {
        if *priority == ALL {
            return order.first().copied();
        } else if order.contains(&priority) {
            return Some(priority);
        }
    }
    if order.contains(&&ALL) { formats.first().copied() } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols(formats: &[Format]) -> Vec<&'static str> {
        formats.iter().map(|m| m.symbol).collect()
    }

    fn fmts(accept: Option<&str>, path: &str, xhr: bool) -> Vec<&'static str> {
        symbols(&formats(&NegotiationInput { accept, path, xhr, ..Default::default() }).unwrap())
    }

    #[test]
    fn browser_accept_falls_back_to_html() {
        let chrome = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8";
        assert_eq!(fmts(Some(chrome), "/rooms/1", false), vec!["html"]);
        assert_eq!(fmts(None, "/rooms/1", false), vec!["html"]);
        assert_eq!(fmts(Some("*/*"), "/rooms/1", false), vec!["*/*"]);
    }

    #[test]
    fn turbo_form_submission() {
        let turbo = "text/vnd.turbo-stream.html, text/html, application/xhtml+xml";
        assert_eq!(fmts(Some(turbo), "/rooms/1/messages", false), vec!["turbo_stream", "html"]);
    }

    #[test]
    fn quality_ordering() {
        assert_eq!(fmts(Some("text/html;q=0.5, application/json"), "/", false), vec!["json", "html"]);
        assert_eq!(fmts(Some("application/json, */*"), "/", false), vec!["html"]);
        assert_eq!(fmts(Some("*/*, application/json;q=0.1"), "/", false), vec!["html"]);
        assert_eq!(fmts(Some("application/json;q=0.1,text/html;q=0.1"), "/", false), vec!["json", "html"]);
    }

    #[test]
    fn q_values_read_like_rails() {
        // `Mime::Type.parse(header)` in the reference.
        for (accept, order) in [
            ("text/html;q=, application/json", ["html", "json"]),
            ("text/html; q=, application/json;q=0.5", ["html", "json"]),
            ("text/html;q=;q=, application/json;q=0.5", ["html", "json"]),
            ("text/html;q=0.5, application/json;q=", ["json", "html"]),
            ("text/html;q=;x=1, application/json", ["json", "html"]),
            ("text/html;q=;q=0.9, application/json;q=0.5", ["json", "html"]),
            ("text/html;q=0.4;q=0.9, application/json;q=0.5", ["json", "html"]),
            ("text/html;q=0.5.1, application/json;q=0.6", ["json", "html"]),
            ("text/html;q=0.5.1.2, application/json;q=0.49", ["html", "json"]),
            ("text/html;q=\"0.9\", application/json;q=0.5", ["html", "json"]),
            ("text/html;q=\"\"0.9, application/json;q=0.5", ["json", "html"]),
            ("text/html;q=1e-1, application/json;q=0.5", ["json", "html"]),
            ("text/html;q=1_0, application/json;q=5", ["html", "json"]),
            ("text/html;q=abc, application/json;q=0.1", ["json", "html"]),
            ("text/html;q=+0.3, application/json;q=0.2", ["html", "json"]),
            ("text/html;q= 0.3, application/json;q=0.2", ["html", "json"]),
            ("text/html;q=1e17, application/json;q=1e18", ["json", "html"]),
            ("text/html;q=1e19, application/json;q=1e20", ["json", "html"]),
            ("text/html;q=-0.001, application/json;q=0", ["html", "json"]),
            ("application/json;q=0, text/html;q=-0.001", ["json", "html"]),
            // `String#to_f` reads hexadecimal after a sign only.
            ("text/html;q=0.5, application/json;q=+0x1", ["json", "html"]),
            ("text/html;q=0.5, application/json;q=0x1", ["html", "json"]),
        ] {
            assert_eq!(symbols(&parse_accept(accept).unwrap()), order, "{accept}");
        }
        assert_eq!(symbols(&parse_accept("*/*;q=, application/json;q=0.5").unwrap()), ["json", "*/*"]);
        // Rails raises FloatDomainError (a 500) on an infinite q-value.
        assert_eq!(symbols(&parse_accept("text/html;q=1e400, application/json").unwrap()), ["html", "json"]);
        assert_eq!(symbols(&parse_accept("text/html;q=-1e400, application/json").unwrap()), ["json", "html"]);
    }

    #[test]
    fn single_types() {
        assert_eq!(fmts(Some("application/json"), "/", false), vec!["json"]);
        assert_eq!(fmts(Some("application/json; charset=utf-8"), "/", false), vec!["json"]);
        assert_eq!(fmts(Some("image/svg+xml"), "/", false), vec!["svg"]);
        assert_eq!(fmts(Some("application/x-unknown"), "/", false), Vec::<&str>::new());
        assert!(formats(&NegotiationInput { accept: Some("garbage"), path: "/", ..Default::default() }).is_err());
    }

    #[test]
    fn wildcards_expand_in_registration_order() {
        assert_eq!(&fmts(Some("text/*"), "/", false)[..3], &["html", "text", "js"]);
        assert!(fmts(Some("text/*"), "/", false).contains(&"turbo_stream"));
    }

    #[test]
    fn xml_folding() {
        assert_eq!(fmts(Some("text/xml, application/rss+xml"), "/", false), vec!["xml", "rss"]);
        assert_eq!(fmts(Some("application/xml, application/rss+xml"), "/", false), vec!["rss", "xml"]);
        assert_eq!(fmts(Some("text/xml;q=0.9, application/xml;q=0.5, text/html"), "/", false), vec!["html", "xml"]);
    }

    #[test]
    fn path_extension_and_format_param() {
        assert_eq!(fmts(None, "/users/1/avatar.svg", false), vec!["svg"]);
        assert_eq!(fmts(None, "/messages.json", false), vec!["json"]);
        assert_eq!(fmts(None, "/x.unknownext", false), vec!["html"]);
        let input = NegotiationInput { format_param: Some("json"), accept: Some("text/html"), path: "/", ..Default::default() };
        assert_eq!(symbols(&formats(&input).unwrap()), vec!["json"]);
        let input = NegotiationInput { format_param: Some("nope"), path: "/", ..Default::default() };
        assert!(formats(&input).unwrap().is_empty());
    }

    #[test]
    fn xhr_defaults_to_js() {
        assert_eq!(fmts(None, "/", true), vec!["js"]);
        let input = NegotiationInput { xhr: true, content_type: Some("application/json"), path: "/", ..Default::default() };
        assert_eq!(symbols(&formats(&input).unwrap()), vec!["json"]);
    }

    #[test]
    fn negotiation() {
        assert_eq!(negotiate(&[&TURBO_STREAM, &HTML], &[&HTML, &JSON]), Some(&HTML));
        assert_eq!(negotiate(&[&ALL], &[&HTML, &JSON]), Some(&HTML));
        assert_eq!(negotiate(&[&PNG], &[&HTML, &JSON]), None);
        assert_eq!(negotiate(&[&PNG], &[&HTML, &ALL]), Some(&PNG));
        assert_eq!(negotiate(&[], &[&HTML]), None);
    }
}
