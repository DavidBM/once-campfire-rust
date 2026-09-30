//! The slice of Ruby's `URI.parse` (uri 1.1, RFC 3986 parser) that the opengraph URL checks and
//! tweet URL normalization rely on, including which inputs raise which error.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    /// `URI::InvalidURIError`, which callers rescue.
    InvalidUri,
    /// `URI::InvalidComponentError` (from `URI::MailTo`), which nothing rescues.
    InvalidComponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uri {
    pub scheme: Option<String>,
    pub userinfo: Option<String>,
    pub host: Option<String>,
    pub port: Option<u64>,
    pub path: Option<String>,
    pub opaque: Option<String>,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl Uri {
    /// `uri.is_a?(URI::HTTP)`, which includes `URI::HTTPS`.
    pub fn is_http(&self) -> bool {
        self.scheme.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("http") || s.eq_ignore_ascii_case("https"))
    }

    fn default_port(&self) -> Option<u64> {
        match self.scheme.as_deref().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("HTTP") | Some("WS") => Some(80),
            Some("HTTPS") | Some("WSS") => Some(443),
            Some("FTP") => Some(21),
            Some("LDAP") => Some(389),
            Some("LDAPS") => Some(636),
            _ => None,
        }
    }

    /// `URI::Generic#to_s`.
    pub fn to_s(&self) -> String {
        let mut s = String::new();
        if let Some(scheme) = &self.scheme {
            s.push_str(scheme);
            s.push(':');
        }
        if let Some(opaque) = &self.opaque {
            s.push_str(opaque);
        } else {
            if self.host.is_some() || matches!(self.scheme.as_deref(), Some("file") | Some("postgres")) {
                s.push_str("//");
            }
            if let Some(userinfo) = &self.userinfo {
                s.push_str(userinfo);
                s.push('@');
            }
            if let Some(host) = &self.host {
                s.push_str(host);
            }
            if let Some(port) = self.port
                && Some(port) != self.default_port()
            {
                s.push(':');
                s.push_str(&port.to_string());
            }
            s.push_str(self.path.as_deref().unwrap_or(""));
            if let Some(query) = &self.query {
                s.push('?');
                s.push_str(query);
            }
        }
        if let Some(fragment) = &self.fragment {
            s.push('#');
            s.push_str(fragment);
        }
        s
    }
}

/// `URI.parse(value)`.
pub fn parse(value: &str) -> Result<Uri, UriError> {
    if !value.is_ascii() {
        return Err(UriError::InvalidUri);
    }
    let mut uri = split_absolute(value).or_else(|| split_relative(value)).ok_or(UriError::InvalidUri)?;
    // URI::Generic#initialize assigns the query through `query=`, which rejects bad escapes
    if let Some(query) = &uri.query {
        uri.query = Some(escape_query(query)?);
    }
    uri.port = uri.port.or_else(|| uri.default_port());
    check_scheme_class(&uri)?;
    Ok(uri)
}

/// The initializers of the scheme classes `URI.for` picks that can raise.
fn check_scheme_class(uri: &Uri) -> Result<(), UriError> {
    match uri.scheme.as_deref().map(|s| s.to_ascii_uppercase()).as_deref() {
        Some("MAILTO") => {
            let opaque = uri.opaque.clone().or_else(|| uri.query.as_ref().map(|q| format!("?{q}")));
            let Some(opaque) = opaque else { return Err(UriError::InvalidComponent) };
            let to = opaque.split_once('?').map(|(to, _)| to).unwrap_or(&opaque);
            if mailto_to_valid(to) { Ok(()) } else { Err(UriError::InvalidComponent) }
        }
        Some("LDAP") | Some("LDAPS") => {
            if uri.fragment.is_some() || uri.path.is_none() {
                Err(UriError::InvalidUri)
            } else {
                Ok(())
            }
        }
        Some("FTP") => {
            if uri.path.is_none() {
                Err(UriError::InvalidUri)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// `/\A(?:[^@,;]+@[^@,;]+(?:\z|[,;]))*\z/`
fn mailto_to_valid(to: &str) -> bool {
    let bytes = to.as_bytes();
    let mut i = 0;
    let special = |b: u8| matches!(b, b'@' | b',' | b';');
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && !special(bytes[i]) {
            i += 1;
        }
        if i == start || i >= bytes.len() || bytes[i] != b'@' {
            return false;
        }
        i += 1;
        let domain = i;
        while i < bytes.len() && !special(bytes[i]) {
            i += 1;
        }
        if i == domain {
            return false;
        }
        if i < bytes.len() {
            if bytes[i] == b'@' {
                return false;
            }
            i += 1;
        }
    }
    true
}

/// `URI::Generic#query=`: rejects `%` followed by two non-hex characters and escapes the rest.
fn escape_query(query: &str) -> Result<String, UriError> {
    let cleaned: Vec<u8> = query.bytes().filter(|b| !matches!(b, b'\t' | b'\r' | b'\n')).collect();
    for w in cleaned.windows(3) {
        if w[0] == b'%' && !w[1].is_ascii_hexdigit() && !w[2].is_ascii_hexdigit() {
            return Err(UriError::InvalidUri);
        }
    }
    let mut out = String::new();
    let mut i = 0;
    while i < cleaned.len() {
        let b = cleaned[i];
        let escape = i + 2 < cleaned.len() && b == b'%' && cleaned[i + 1].is_ascii_hexdigit() && cleaned[i + 2].is_ascii_hexdigit();
        if escape || matches!(b, b'!' | b'$'..=b'&' | b'('..=b';' | b'=' | b'?'..=b'_' | b'a'..=b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
        i += 1;
    }
    Ok(out)
}

// --- RFC 3986 matching, mirroring URI::RFC3986_Parser::RFC3986_URI ------------------------------

fn is_unreserved_or_sub(b: u8) -> bool {
    // [!$&-.0-9;=A-Z_a-z~] — note `&-.` covers & ' ( ) * + , - .
    matches!(b, b'!' | b'$' | b'&'..=b'.' | b'0'..=b'9' | b';' | b'=' | b'A'..=b'Z' | b'_' | b'a'..=b'z' | b'~')
}

fn pct_at(s: &[u8], i: usize) -> bool {
    i + 2 < s.len() && s[i] == b'%' && s[i + 1].is_ascii_hexdigit() && s[i + 2].is_ascii_hexdigit()
}

/// Consumes `(?:%\h\h|[class])*` possessively and returns the end index.
fn take_while_class(s: &[u8], mut i: usize, class: impl Fn(u8) -> bool) -> usize {
    loop {
        if pct_at(s, i) {
            i += 3;
        } else if i < s.len() && class(s[i]) {
            i += 1;
        } else {
            return i;
        }
    }
}

fn seg_char(b: u8) -> bool {
    is_unreserved_or_sub(b) || matches!(b, b':' | b'@' | b'/')
}

fn seg_nc_char(b: u8) -> bool {
    is_unreserved_or_sub(b) || b == b'@'
}

fn fragment_char(b: u8) -> bool {
    is_unreserved_or_sub(b) || matches!(b, b':' | b'@' | b'/' | b'?')
}

fn userinfo_char(b: u8) -> bool {
    is_unreserved_or_sub(b) || b == b':'
}

/// Matches the IP-literal alternative of HOST (a bracketed address); returns its end.
fn ip_literal(s: &[u8], i: usize) -> Option<usize> {
    if s.get(i) != Some(&b'[') {
        return None;
    }
    let close = s[i..].iter().position(|&b| b == b']')? + i;
    let inner = std::str::from_utf8(&s[i + 1..close]).ok()?;
    let valid = if let Some(future) = inner.strip_prefix('v').or_else(|| inner.strip_prefix('V')) {
        let (hex, rest) = future.split_once('.')?;
        !hex.is_empty()
            && hex.bytes().all(|b| b.is_ascii_hexdigit())
            && !rest.is_empty()
            && rest.bytes().all(|b| is_unreserved_or_sub(b) || b == b':')
    } else {
        inner.parse::<std::net::Ipv6Addr>().is_ok() && !inner.contains('%')
    };
    valid.then_some(close + 1)
}

fn split_absolute(value: &str) -> Option<Uri> {
    let s = value.as_bytes();
    // scheme
    if !s.first()?.is_ascii_alphabetic() {
        return None;
    }
    let mut i = 1;
    while i < s.len() && (s[i].is_ascii_alphanumeric() || matches!(s[i], b'+' | b'-' | b'.')) {
        i += 1;
    }
    if s.get(i) != Some(&b':') {
        return None;
    }
    // `URI::Generic#set_scheme` downcases it.
    let scheme = value[..i].to_ascii_lowercase();
    let rest_start = i + 1;
    let tail = parse_query_fragment_positions(s, rest_start)?;
    let hier = &s[rest_start..tail.hier_end];
    let mut uri = Uri {
        scheme: Some(scheme),
        userinfo: None,
        host: None,
        port: None,
        path: None,
        opaque: None,
        query: tail.query.map(|(a, b)| value[a..b].to_string()),
        fragment: tail.fragment.map(|(a, b)| value[a..b].to_string()),
    };
    if hier.starts_with(b"//") {
        let authority = parse_authority(value, rest_start + 2, tail.hier_end)?;
        uri.userinfo = authority.userinfo;
        uri.host = Some(authority.host);
        uri.port = authority.port;
        uri.path = Some(value[authority.end..tail.hier_end].to_string());
        // path-abempty: (?:/seg*)?
        let path = &s[authority.end..tail.hier_end];
        if !path.is_empty() && (path[0] != b'/' || take_while_class(s, authority.end, seg_char) != tail.hier_end) {
            return None;
        }
    } else if hier.starts_with(b"/") {
        // path-absolute: /((?!/)seg++)?
        if take_while_class(s, rest_start, seg_char) != tail.hier_end {
            return None;
        }
        uri.path = Some(value[rest_start..tail.hier_end].to_string());
    } else if !hier.is_empty() {
        // path-rootless becomes the opaque part, with the query folded back in
        if take_while_class(s, rest_start, seg_char) != tail.hier_end {
            return None;
        }
        let mut opaque = value[rest_start..tail.hier_end].to_string();
        if let Some(q) = uri.query.take() {
            opaque.push('?');
            opaque.push_str(&q);
        }
        uri.opaque = Some(opaque);
    } else {
        uri.path = Some(String::new());
    }
    Some(uri)
}

struct Tail {
    hier_end: usize,
    query: Option<(usize, usize)>,
    fragment: Option<(usize, usize)>,
}

/// Splits off `(?:\?(?<query>[^#]*+))?(?:\#(?<fragment>FRAGMENT))?\z` from the end of the hier-part.
fn parse_query_fragment_positions(s: &[u8], start: usize) -> Option<Tail> {
    let hier_end = (start..s.len()).find(|&i| s[i] == b'?' || s[i] == b'#').unwrap_or(s.len());
    let mut i = hier_end;
    let mut query = None;
    if s.get(i) == Some(&b'?') {
        let q_end = (i + 1..s.len()).find(|&j| s[j] == b'#').unwrap_or(s.len());
        query = Some((i + 1, q_end));
        i = q_end;
    }
    let mut fragment = None;
    if s.get(i) == Some(&b'#') {
        let f_end = take_while_class(s, i + 1, fragment_char);
        if f_end != s.len() {
            return None;
        }
        fragment = Some((i + 1, s.len()));
        i = s.len();
    }
    (i == s.len()).then_some(Tail { hier_end, query, fragment })
}

struct Authority {
    userinfo: Option<String>,
    host: String,
    port: Option<u64>,
    end: usize,
}

fn parse_authority(value: &str, start: usize, limit: usize) -> Option<Authority> {
    let s = &value.as_bytes()[..limit];
    let mut i = start;
    let mut userinfo = None;
    let ui_end = take_while_class(s, i, userinfo_char);
    if s.get(ui_end) == Some(&b'@') {
        userinfo = Some(value[i..ui_end].to_string());
        i = ui_end + 1;
    }
    let host_end = ip_literal(s, i).unwrap_or_else(|| take_while_class(s, i, is_unreserved_or_sub));
    let host = value[i..host_end].to_string();
    let mut end = host_end;
    let mut port = None;
    if s.get(end) == Some(&b':') {
        let digits_end = (end + 1..s.len()).find(|&j| !s[j].is_ascii_digit()).unwrap_or(s.len());
        let digits = &value[end + 1..digits_end];
        port = if digits.is_empty() { None } else { Some(digits.parse::<u64>().unwrap_or(u64::MAX)) };
        end = digits_end;
    }
    if end < s.len() && s[end] != b'/' {
        return None;
    }
    Some(Authority { userinfo, host, port, end })
}

/// RFC3986_relative_ref: only validity matters here, since a relative reference is never HTTP.
fn split_relative(value: &str) -> Option<Uri> {
    let s = value.as_bytes();
    let tail = parse_query_fragment_positions(s, 0)?;
    let hier = &s[..tail.hier_end];
    let valid = if hier.starts_with(b"//") {
        match parse_authority(value, 2, tail.hier_end) {
            Some(a) => take_while_class(s, a.end, seg_char) == tail.hier_end,
            None => false,
        }
    } else if hier.starts_with(b"/") || hier.is_empty() {
        take_while_class(s, 0, seg_char) == tail.hier_end
    } else {
        let first = take_while_class(s, 0, seg_nc_char);
        first > 0 && (first == tail.hier_end || (s[first] == b'/' && take_while_class(s, first, seg_char) == tail.hier_end))
    };
    valid.then(|| Uri {
        scheme: None,
        userinfo: None,
        host: None,
        port: None,
        path: Some(value[..tail.hier_end].to_string()),
        opaque: None,
        query: tail.query.map(|(a, b)| value[a..b].to_string()),
        fragment: tail.fragment.map(|(a, b)| value[a..b].to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_ruby() {
        let uri = parse("https://x.com/dhh/status/1?s=20").unwrap();
        assert_eq!(uri.host.as_deref(), Some("x.com"));
        assert_eq!(uri.query.as_deref(), Some("s=20"));
        assert!(parse("http://exa mple.com/ ").is_err());
        assert_eq!(parse("https:/rooms/1").unwrap().host, None);
        assert_eq!(parse("https:rooms/1").unwrap().opaque.as_deref(), Some("rooms/1"));
        assert_eq!(parse("https://").unwrap().host.as_deref(), Some(""));
        assert_eq!(parse("http://[::1]/x").unwrap().host.as_deref(), Some("[::1]"));
        assert_eq!(parse("mailto:foo"), Err(UriError::InvalidComponent));
        assert!(parse("mailto:a@b.com").is_ok());
        assert_eq!(parse("https://x.com:443/a").unwrap().to_s(), "https://x.com/a");
    }

    #[test]
    fn downcases_the_scheme_like_ruby() {
        // `URI.parse(s)` in the reference: `scheme=` downcases.
        assert_eq!(parse("HTTPS://x.com/a").unwrap().scheme.as_deref(), Some("https"));
        assert_eq!(parse("HTTP://X.com:80/a").unwrap().to_s(), "http://X.com/a");
        assert_eq!(parse("FILE:///etc").unwrap().to_s(), "file:///etc");
        assert_eq!(parse("MailTo:a@b.com").unwrap().to_s(), "mailto:a@b.com");
    }
}
