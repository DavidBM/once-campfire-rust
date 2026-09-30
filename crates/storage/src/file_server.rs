//! `ActiveStorage::FileServer#serve_file` on top of `Rack::Files#serving` (rack 3.2): conditional
//! GET on mtime, single and multipart byte ranges, and 416s. HTTP-framework agnostic: the result
//! describes the response and the caller streams the body parts.

use std::path::{Path, PathBuf};

use crate::Result;

pub const MULTIPART_BOUNDARY: &str = "AaB03x";

#[derive(Debug, PartialEq)]
pub struct Served {
    pub status: u16,
    /// In Rack's order; `content-type` and `content-disposition` are set last by FileServer.
    pub headers: Vec<(String, String)>,
    pub body: Vec<BodyPart>,
}

#[derive(Debug, PartialEq)]
pub enum BodyPart {
    Bytes(Vec<u8>),
    /// Inclusive byte range of `path`.
    File {
        path: PathBuf,
        start: u64,
        end: u64,
    },
}

pub struct Request<'a> {
    pub method: &'a str,
    pub range: Option<&'a str>,
    pub if_modified_since: Option<&'a str>,
}

/// `serve_file(path, content_type:, disposition:)` as `DiskController#show` calls it.
pub fn serve_file(request: &Request, path: &Path, content_type: Option<&str>, disposition: Option<&str>) -> Result<Served> {
    let mut served = serving(request, path)?;
    if served.status == 416 {
        served.headers.retain(|(name, _)| !name.eq_ignore_ascii_case("x-cascade"));
    }
    set_header(&mut served.headers, "content-type", content_type.unwrap_or("application/octet-stream"));
    set_header(&mut served.headers, "content-disposition", disposition.unwrap_or("attachment"));
    Ok(served)
}

fn serving(request: &Request, path: &Path) -> Result<Served> {
    if request.method == "OPTIONS" {
        return Ok(Served {
            status: 200,
            headers: vec![("allow".into(), "GET, HEAD, OPTIONS".into()), ("content-length".into(), "0".into())],
            body: vec![],
        });
    }
    let metadata = std::fs::metadata(path)?;
    let last_modified = httpdate(metadata.modified()?);
    if request.if_modified_since == Some(last_modified.as_str()) {
        return Ok(Served { status: 304, headers: vec![], body: vec![] });
    }

    // Disk keys have no extension, so Rack's mime lookup falls back to its default.
    let mime_type = "text/plain";
    let mut headers = vec![("last-modified".to_string(), last_modified), ("content-type".to_string(), mime_type.to_string())];
    let size = metadata.len();

    let (status, body, length) = match byte_ranges(request.range, size) {
        None => (200, vec![BodyPart::File { path: path.to_path_buf(), start: 0, end: size.saturating_sub(1) }], size),
        Some(ranges) if ranges.is_empty() => {
            let body = "Byte range unsatisfiable\n";
            return Ok(Served {
                status: 416,
                headers: vec![
                    ("content-type".into(), "text/plain".into()),
                    ("content-length".into(), body.len().to_string()),
                    ("x-cascade".into(), "pass".into()),
                    ("content-range".into(), format!("bytes */{size}")),
                ],
                body: vec![BodyPart::Bytes(body.as_bytes().to_vec())],
            });
        }
        Some(ranges) => {
            let mut parts = Vec::new();
            if ranges.len() == 1 {
                let (start, end) = ranges[0];
                headers.push(("content-range".into(), format!("bytes {start}-{end}/{size}")));
                parts.push(BodyPart::File { path: path.to_path_buf(), start, end });
            } else {
                set_header(&mut headers, "content-type", &format!("multipart/byteranges; boundary={MULTIPART_BOUNDARY}"));
                for &(start, end) in &ranges {
                    let heading = format!(
                        "\r\n--{MULTIPART_BOUNDARY}\r\ncontent-type: {mime_type}\r\ncontent-range: bytes {start}-{end}/{size}\r\n\r\n"
                    );
                    parts.push(BodyPart::Bytes(heading.into_bytes()));
                    parts.push(BodyPart::File { path: path.to_path_buf(), start, end });
                }
                parts.push(BodyPart::Bytes(format!("\r\n--{MULTIPART_BOUNDARY}--\r\n").into_bytes()));
            }
            let length = parts
                .iter()
                .map(|part| match part {
                    BodyPart::Bytes(bytes) => bytes.len() as u64,
                    BodyPart::File { start, end, .. } => end - start + 1,
                })
                .sum();
            (206, parts, length)
        }
    };

    headers.push(("content-length".into(), length.to_string()));
    let body = if request.method == "HEAD" || size == 0 { vec![] } else { body };
    Ok(Served { status, headers, body })
}

/// `Rack::Utils.get_byte_ranges`: `None` means "serve everything", an empty list means 416.
pub fn byte_ranges(header: Option<&str>, size: u64) -> Option<Vec<(u64, u64)>> {
    if size == 0 {
        return None;
    }
    let spec = range_spec(header?)?;
    if spec.matches(',').count() >= 100 {
        return None;
    }
    let size = size as i128;
    let mut ranges = Vec::new();
    for range_spec in ruby_split(spec, split_comma) {
        if !range_spec.contains('-') {
            return None;
        }
        let parts = ruby_split(range_spec, |s| s.find('-').map(|i| (i, i + 1)));
        let (r0, r1) = (parts.first().copied(), parts.get(1).copied());
        let (r0, r1) = match r0 {
            None | Some("") => {
                let r1 = r1?;
                ((size - ruby_to_i(r1)).max(0), size - 1)
            }
            Some(r0) => {
                let r0 = ruby_to_i(r0);
                match r1 {
                    None => (r0, size - 1),
                    Some(r1) => {
                        let r1 = ruby_to_i(r1);
                        if r1 < r0 {
                            return None;
                        }
                        (r0, r1.min(size - 1))
                    }
                }
            }
        };
        if r0 <= r1 {
            ranges.push((r0, r1));
        }
    }
    if ranges.iter().map(|(a, b)| b - a + 1).sum::<i128>() > size {
        return Some(vec![]);
    }
    Some(ranges.into_iter().map(|(a, b)| (a as u64, b as u64)).collect())
}

/// `http_range =~ /bytes=([^;]+)/`: after the first `bytes=` that something other than `;`
/// follows, up to the next `;`.
fn range_spec(header: &str) -> Option<&str> {
    header.match_indices("bytes=").find_map(|(i, _)| {
        let rest = &header[i + 6..];
        let spec = &rest[..rest.find(';').unwrap_or(rest.len())];
        (!spec.is_empty()).then_some(spec)
    })
}

/// `/,[ \t]*/`
fn split_comma(s: &str) -> Option<(usize, usize)> {
    let i = s.find(',')?;
    let rest = &s[i + 1..];
    let skipped = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    Some((i, i + 1 + skipped))
}

/// `String#split` with a separator finder: trailing empty fields are dropped.
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

/// `String#to_i`: leading whitespace, an optional sign and `0d`, then digits (underscores between
/// them). Saturating: past i128 is past any file size.
fn ruby_to_i(s: &str) -> i128 {
    let s = s.trim_start_matches([' ', '\t', '\n', '\u{b}', '\u{c}', '\r']);
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let digits = digits.strip_prefix("0d").or_else(|| digits.strip_prefix("0D")).unwrap_or(digits);
    let mut value: i128 = 0;
    let mut previous_underscore = false;
    for (i, c) in digits.char_indices() {
        match c {
            '0'..='9' => {
                value = value.saturating_mul(10).saturating_add((c as u8 - b'0') as i128);
                previous_underscore = false;
            }
            '_' if i > 0 && !previous_underscore => previous_underscore = true,
            _ => break,
        }
    }
    if negative { -value } else { value }
}

fn set_header(headers: &mut Vec<(String, String)>, name: &str, value: &str) {
    match headers.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        Some(entry) => entry.1 = value.to_string(),
        None => headers.push((name.to_string(), value.to_string())),
    }
}

/// `Time#httpdate`: `Sun, 06 Nov 1994 08:49:37 GMT`.
fn httpdate(time: std::time::SystemTime) -> String {
    let ts = jiff::Timestamp::try_from(time).unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    ts.strftime("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte ranges, `None` for the whole file.
    type Ranges = Option<&'static [(u64, u64)]>;

    /// `Rack::Utils.get_byte_ranges(header, size)` in the reference (rack 3.2.6).
    #[rustfmt::skip]
    const RACK_RANGES: &[(&str, u64, Ranges)] = &[
        ("bytes=0-4", 10, Some(&[(0, 4)])),
        ("bytes=5-", 10, Some(&[(5, 9)])),
        ("bytes=-3", 10, Some(&[(7, 9)])),
        ("bytes=-30", 10, Some(&[(0, 9)])),
        ("bytes=0-99999999999999999999", 10, Some(&[(0, 9)])),
        ("bytes=99999999999999999999-", 10, Some(&[])),
        ("bytes=-99999999999999999999", 10, Some(&[(0, 9)])),
        ("bytes=0-1,", 10, Some(&[(0, 1)])),
        ("bytes= -5", 10, Some(&[(0, 5)])),
        ("bytes=0-1, 3-4", 10, Some(&[(0, 1), (3, 4)])),
        ("bytes=0-1,\t 3-4", 10, Some(&[(0, 1), (3, 4)])),
        ("bytes=3-5,1-2", 10, Some(&[(3, 5), (1, 2)])),
        ("bytes=+1-2", 10, Some(&[(1, 2)])),
        ("bytes=1-+2", 10, Some(&[(1, 2)])),
        ("bytes=1_0-2_0", 100, Some(&[(10, 20)])),
        ("bytes=0d5-0d9", 100, Some(&[(5, 9)])),
        ("bytes=\u{a0}1-2", 10, Some(&[(0, 2)])),
        ("bytes=a-b", 10, Some(&[(0, 0)])),
        ("bytes=0-0x5", 10, Some(&[(0, 0)])),
        ("bytes=0-1 ", 10, Some(&[(0, 1)])),
        ("bytes=0 -1", 10, Some(&[(0, 1)])),
        ("bytes=1-2-3", 10, Some(&[(1, 2)])),
        ("bytes=9-9", 10, Some(&[(9, 9)])),
        ("bytes=10-", 10, Some(&[])),
        ("bytes=10-12", 10, Some(&[])),
        ("bytes=-0", 10, Some(&[])),
        ("bytes=--5", 10, Some(&[])),
        ("bytes=0-4,5-9,0-0", 10, Some(&[])),
        ("bytes=;bytes=2-3", 10, Some(&[(2, 3)])),
        ("bytes=0-1;bytes=2-3", 10, Some(&[(0, 1)])),
        ("xbytes=0-1", 10, Some(&[(0, 1)])),
        ("bytes=;0-1", 10, None),
        ("bytes=", 10, None),
        ("bytes=-", 10, None),
        ("bytes=5", 10, None),
        ("bytes=1-0", 10, None),
        ("bytes=0-1,,2-3", 10, None),
        ("items=0-1", 10, None),
        ("bytes=0-4", 0, None),
    ];

    #[test]
    fn byte_ranges_match_rack() {
        for &(header, size, expected) in RACK_RANGES {
            assert_eq!(byte_ranges(Some(header), size), expected.map(<[_]>::to_vec), "{header:?} of {size}");
        }
        assert_eq!(byte_ranges(None, 10), None);
        // 99 commas are read, and 100 aren't (`max_ranges`).
        assert_eq!(byte_ranges(Some(&format!("bytes=0-1,{}", "0-0,".repeat(98))), 10), Some(vec![]));
        assert_eq!(byte_ranges(Some(&format!("bytes=0-1,{}", "0-0,".repeat(99))), 10), None);
    }
}
