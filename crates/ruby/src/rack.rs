//! What Rack (3.2) does with request headers, where Ruby's string behaviour shows through.

use crate::integer::to_i128;

/// `Rack::Utils.get_byte_ranges(http_range, size)`: `None` means "serve everything" and an empty
/// list means 416. The ranges are inclusive and within `size`.
pub fn byte_ranges(header: Option<&str>, size: u64) -> Option<Vec<(u64, u64)>> {
    if size == 0 {
        return None;
    }
    let spec = range_spec(header?)?;
    if spec.matches(',').count() >= 100 {
        return None;
    }
    let size = i128::from(size);
    let mut ranges = Vec::new();
    for range_spec in ruby_split(spec, split_comma) {
        if !range_spec.contains('-') {
            return None;
        }
        // Split on "-" first, so neither end can be negative.
        let parts = ruby_split(range_spec, |s| s.find('-').map(|i| (i, i + 1)));
        let (r0, r1) = (parts.first().copied(), parts.get(1).copied());
        let (r0, r1) = match r0 {
            None | Some("") => {
                let r1 = r1?;
                ((size - to_i128(r1)).max(0), size - 1)
            }
            Some(r0) => {
                let r0 = to_i128(r0);
                match r1 {
                    None => (r0, size - 1),
                    Some(r1) => {
                        let r1 = to_i128(r1);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_ranges_like_rack() {
        // `Rack::Utils.get_byte_ranges(header, 10)` in the reference (rack 3.2.6).
        let ranges = |header: &str| byte_ranges(Some(header), 10);
        assert_eq!(ranges("bytes=0-4"), Some(vec![(0, 4)]));
        assert_eq!(ranges("bytes=-3"), Some(vec![(7, 9)]));
        assert_eq!(ranges("bytes=0-1, 3-4"), Some(vec![(0, 1), (3, 4)]));
        assert_eq!(ranges("bytes=0-99999999999999999999"), Some(vec![(0, 9)]));
        assert_eq!(ranges("bytes=99999999999999999999-"), Some(vec![]));
        assert_eq!(ranges("bytes=--5"), Some(vec![]));
        assert_eq!(ranges("bytes=0-4,5-9,0-0"), Some(vec![]));
        assert_eq!(ranges("bytes=1-0"), None);
        assert_eq!(byte_ranges(None, 10), None);
        assert_eq!(byte_ranges(Some("bytes=0-4"), 0), None);
    }
}
