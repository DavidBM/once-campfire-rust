//! `ActiveStorage::FileServer#serve_file` on top of `Rack::Files#serving` (rack 3.2): conditional
//! GET on mtime, single and multipart byte ranges, and 416s. HTTP-framework agnostic: the result
//! describes the response and the caller streams the body parts.

use ruby_compat::rack::byte_ranges;
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
