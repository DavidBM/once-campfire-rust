//! Responses as controllers build them, before the kit finishes them (cookies, default headers,
//! ETags, conditional GET, HEAD) and hands them to hyper.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Bytes;
use axum::http::header::{self, HeaderName, HeaderValue};
use axum::http::{HeaderMap, StatusCode};

use crate::deflater::splice::PageParts;

pub const HTML_UTF8: &str = "text/html; charset=utf-8";
pub const JSON_UTF8: &str = "application/json; charset=utf-8";
pub const TURBO_STREAM_UTF8: &str = "text/vnd.turbo-stream.html; charset=utf-8";

#[derive(Debug)]
pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Body,
    /// The body's SHA-256, when `Rack::ETag` digested it.
    pub(crate) body_digest: Option<crate::deflater::BodyDigest>,
}

pub enum Body {
    Empty,
    Bytes(Bytes),
    /// A page with cached fragments, in parts: the ETag and gzip reuse the fragments' digests and
    /// compressed pieces instead of working through the whole body, which is never joined.
    Parts(Arc<PageParts>),
    File(FileBody),
    /// A streaming body (e.g. a proxied blob); never ETagged.
    Stream(axum::body::Body),
}

impl Body {
    /// The body `text` makes with each of `fragments` (cached HTML, in order) spliced in at its
    /// byte offset in `text`, as a template recorded them while it rendered.
    ///
    /// # Panics
    ///
    /// If an offset is past the end of `text` or before the previous one.
    pub fn spliced(text: impl Into<Bytes>, fragments: Vec<(usize, Arc<String>)>) -> Self {
        match PageParts::splice(&text.into(), fragments) {
            Ok(parts) => Body::Parts(Arc::new(parts)),
            Err(whole) => Body::Bytes(whole),
        }
    }
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Body::Empty => f.write_str("Empty"),
            Body::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
            Body::Parts(parts) => write!(f, "Parts({} bytes)", parts.body_len()),
            Body::File(file) => write!(f, "File({:?})", file),
            Body::Stream(_) => f.write_str("Stream"),
        }
    }
}

/// A file (or a byte range of one) to stream from disk.
#[derive(Debug, Clone)]
pub struct FileBody {
    pub path: PathBuf,
    pub offset: u64,
    pub len: u64,
}

impl Response {
    pub fn new(status: StatusCode) -> Self {
        Self { status, headers: HeaderMap::new(), body: Body::Empty, body_digest: None }
    }

    pub fn with_body(status: StatusCode, content_type: &str, body: impl Into<Bytes>) -> Self {
        Self::new(status).content_type(content_type).body(body)
    }

    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Body::Bytes(body.into());
        self
    }

    pub fn content_type(self, content_type: &str) -> Self {
        self.header(header::CONTENT_TYPE, content_type)
    }

    /// Set (replace) a header, as Puma would write the value (see `set_header_value`). Panics on
    /// an invalid header name, which is a programming error.
    pub fn header(mut self, name: impl TryInto<HeaderName>, value: &str) -> Self {
        let name = name.try_into().unwrap_or_else(|_| panic!("invalid header name"));
        set_header_value(&mut self.headers, name, value);
        self
    }

    pub fn get_header(&self, name: impl axum::http::header::AsHeaderName) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    pub fn location(&self) -> Option<&str> {
        self.get_header(header::LOCATION)
    }

    /// The body bytes, for tests and middleware; `None` for files and streams.
    pub fn body_bytes(&self) -> Option<&Bytes> {
        match &self.body {
            Body::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }
}

/// Replaces `name` with `value` the way Puma writes a Rack header (puma 7.2.1,
/// `Puma::Request#str_headers`). A value with a line break goes out one line per header line
/// (`split("\n")`, which drops trailing empty lines), and a line holding any other control
/// character (`ILLEGAL_HEADER_VALUE_REGEX`) is left out. Values come from the request at times,
/// like `?disposition=` on a proxied blob. Puma writes a DEL, which Thruster then refuses with a
/// 502; that line is left out too.
pub(crate) fn set_header_value(headers: &mut HeaderMap, name: HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
        return;
    }
    headers.remove(&name);
    let mut lines: Vec<&str> = value.split('\n').collect();
    while lines.last() == Some(&"") {
        lines.pop();
    }
    for line in lines.into_iter().filter_map(|line| HeaderValue::from_str(line).ok()) {
        headers.append(name.clone(), line);
    }
}

/// `Content-Disposition` as `ActionDispatch::Http::ContentDisposition#to_s` builds it.
fn content_disposition(disposition: &str, filename: Option<&str>) -> String {
    filename.map_or_else(|| disposition.to_string(), |filename| rails_compat::content_disposition::format(disposition, filename))
}

/// Options for `send_file` / `send_data`.
#[derive(Debug, Clone)]
pub struct SendOptions {
    pub filename: Option<String>,
    /// `type:`; defaults to the filename's MIME type, else `application/octet-stream`.
    pub content_type: Option<String>,
    /// `disposition:`; `None` omits the header. Defaults to `attachment`.
    pub disposition: Option<String>,
    pub status: StatusCode,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self { filename: None, content_type: None, disposition: Some("attachment".into()), status: StatusCode::OK }
    }
}

impl SendOptions {
    pub fn inline(content_type: &str) -> Self {
        Self { content_type: Some(content_type.into()), disposition: Some("inline".into()), ..Self::default() }
    }
}

/// `send_file_headers!` then the body: file or bytes, whole, whatever the `Range` header asks for
/// (as Rails' `send_file` and `send_data` do).
pub(crate) fn send(options: &SendOptions, body: SendBody) -> Response {
    let content_type = options.content_type.clone().unwrap_or_else(|| {
        options
            .filename
            .as_deref()
            .and_then(|f| Path::new(f).extension())
            .and_then(|ext| crate::format::lookup_by_extension(&ext.to_string_lossy().to_lowercase()))
            .map(|m| m.string.to_string())
            .unwrap_or_else(|| "application/octet-stream".into())
    });
    let mut response = Response::new(options.status).content_type(&content_type);
    if let Some(disposition) = &options.disposition {
        response = response.header(header::CONTENT_DISPOSITION, &content_disposition(disposition, options.filename.as_deref()));
    }
    response = response.header("content-transfer-encoding", "binary");
    response.body = match body {
        SendBody::File(path, len) => Body::File(FileBody { path, offset: 0, len }),
        SendBody::Bytes(bytes) => Body::Bytes(bytes),
    };
    response
}

pub(crate) enum SendBody {
    File(PathBuf, u64),
    Bytes(Bytes),
}

/// `Cache-Control` directives set by `expires_in`, `fresh_when(public:)`, `no_store` and friends,
/// normalized like `ActionDispatch::Http::Cache::Response#merge_and_normalize_cache_control!`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CacheControl {
    pub max_age: Option<u64>,
    pub public: bool,
    pub private: bool,
    pub must_revalidate: bool,
    pub no_cache: bool,
    pub no_store: bool,
    pub must_understand: bool,
    pub stale_while_revalidate: Option<u64>,
    pub stale_if_error: Option<u64>,
    pub immutable: bool,
    pub extras: Vec<String>,
}

impl CacheControl {
    pub fn is_empty(&self) -> bool {
        *self == CacheControl::default()
    }

    pub fn to_header(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut options: Vec<String> = Vec::new();
        if self.no_store {
            if self.private {
                options.push("private".into());
            }
            if self.must_understand {
                options.push("must-understand".into());
            }
            options.push("no-store".into());
        } else if self.no_cache {
            if self.public {
                options.push("public".into());
            }
            options.push("no-cache".into());
            options.extend(self.extras.iter().cloned());
        } else {
            if let Some(max_age) = self.max_age {
                options.push(format!("max-age={max_age}"));
            }
            options.push(if self.public { "public" } else { "private" }.into());
            if self.must_revalidate {
                options.push("must-revalidate".into());
            }
            if let Some(swr) = self.stale_while_revalidate {
                options.push(format!("stale-while-revalidate={swr}"));
            }
            if let Some(sie) = self.stale_if_error {
                options.push(format!("stale-if-error={sie}"));
            }
            if self.immutable {
                options.push("immutable".into());
            }
            options.extend(self.extras.iter().cloned());
        }
        Some(options.join(", "))
    }
}

/// Options for `expires_in`.
#[derive(Debug, Clone, Default)]
pub struct ExpiresIn {
    pub public: bool,
    pub must_revalidate: bool,
    pub stale_while_revalidate: Option<u64>,
    pub stale_if_error: Option<u64>,
    pub immutable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_values_with_control_characters_go_out_as_puma_writes_them() {
        let values = |value: &str| {
            let response = Response::new(StatusCode::OK).header("x-test", "replaced").header("x-test", value);
            response.headers.get_all("x-test").iter().map(|v| v.as_bytes().to_vec()).collect::<Vec<_>>()
        };
        assert_eq!(values("a\tb"), [b"a\tb".to_vec()]);
        assert_eq!(values("é"), ["é".as_bytes().to_vec()]);
        assert_eq!(values(""), [b"".to_vec()]);
        // Each line on its own, and a line with a control character left out.
        assert_eq!(values("x\ny; z"), [b"x".to_vec(), b"y; z".to_vec()]);
        assert_eq!(values("\na\n\n"), [b"".to_vec(), b"a".to_vec()]);
        assert_eq!(values("x\r\ny"), [b"y".to_vec()]);
        assert!(values("x\ry").is_empty());
        assert!(values("x\u{1}y").is_empty());
        assert!(values("x\u{7f}y").is_empty());
        assert!(values("\n").is_empty());
    }

    #[test]
    fn content_disposition_like_rails() {
        assert_eq!(content_disposition("inline", None), "inline");
        assert_eq!(content_disposition("attachment", Some("logo.png")), "attachment; filename=\"logo.png\"; filename*=UTF-8''logo.png");
        assert_eq!(
            content_disposition("inline", Some("résumé 1.pdf")),
            "inline; filename=\"resume 1.pdf\"; filename*=UTF-8''r%C3%A9sum%C3%A9%201.pdf"
        );
        assert_eq!(
            content_disposition("inline", Some("日本\"x\".txt")),
            "inline; filename=\"%3F%3F%22x%22.txt\"; filename*=UTF-8''%E6%97%A5%E6%9C%AC%22x%22.txt"
        );
        // I18n's whole table, not just Latin-1 letters.
        assert_eq!(
            content_disposition("attachment", Some("Łódź ×.pdf")),
            "attachment; filename=\"Lodz x.pdf\"; filename*=UTF-8''%C5%81%C3%B3d%C5%BA%20%C3%97.pdf"
        );
    }

    #[test]
    fn cache_control_normalization() {
        let cc = CacheControl { max_age: Some(300), public: true, stale_while_revalidate: Some(604800), ..Default::default() };
        assert_eq!(cc.to_header().unwrap(), "max-age=300, public, stale-while-revalidate=604800");
        let cc = CacheControl { max_age: Some(0), must_revalidate: true, ..Default::default() };
        assert_eq!(cc.to_header().unwrap(), "max-age=0, private, must-revalidate");
        let cc = CacheControl { no_store: true, max_age: Some(5), ..Default::default() };
        assert_eq!(cc.to_header().unwrap(), "no-store");
        assert_eq!(CacheControl::default().to_header(), None);
    }
}
