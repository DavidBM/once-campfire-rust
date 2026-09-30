//! Ruby and Active Support string behaviors the pipeline depends on.

use serde_json::Value;

/// `ERB::Util.html_escape` (and `h`): escapes `& < > " '`.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Ruby's whitespace for `String#strip`: NUL, `\t`, `\n`, `\v`, `\f`, `\r` and space.
fn is_strip_whitespace(c: char) -> bool {
    matches!(c, '\0' | '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r' | ' ')
}

/// `String#strip`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_strip_whitespace)
}

/// Active Support's `String#blank?`: empty or only Unicode whitespace (`/\A[[:space:]]*\z/`).
pub fn is_blank(s: &str) -> bool {
    s.chars().all(char::is_whitespace)
}

/// `Object#present?` for an optional string.
pub fn presence(s: Option<&str>) -> Option<&str> {
    s.filter(|v| !is_blank(v))
}

/// `String#chomp("")`: removes every trailing `\n` or `\r\n`, but not a lone `\r`.
pub fn chomp_newlines(s: &str) -> &str {
    let mut end = s.len();
    loop {
        let rest = &s[..end];
        if rest.ends_with("\r\n") {
            end -= 2;
        } else if rest.ends_with('\n') {
            end -= 1;
        } else {
            return rest;
        }
    }
}

/// `String#chomp` with no argument: removes one trailing `\r\n`, `\n` or `\r`.
pub fn chomp(s: &str) -> &str {
    s.strip_suffix("\r\n").or_else(|| s.strip_suffix('\n')).or_else(|| s.strip_suffix('\r')).unwrap_or(s)
}

/// Action View's `truncate(text, length:, omission:)` with the default separator, before escaping.
pub fn truncate(text: &str, length: usize, omission: &str) -> String {
    let chars = text.chars().count();
    if chars <= length {
        return text.to_string();
    }
    let keep = length.saturating_sub(omission.chars().count());
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(omission);
    out
}

/// `ERB::Util.url_encode`: percent-encodes everything but unreserved characters.
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Ruby's `JSON.parse`, which (unlike serde_json) also skips `/* */` and `//` comments.
pub fn json_parse(s: &str) -> Option<Value> {
    serde_json::from_str(&strip_json_comments(s)?).ok()
}

fn strip_json_comments(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                out.push(chars.next()?);
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('*')) => {
                chars.next();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return None;
                }
                out.push(' ');
            }
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    Some(out)
}

/// `Object#to_s` of a parsed JSON value, as Nokogiri's `create_element` applies to attribute values.
pub fn json_value_to_s(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => json_value_inspect(other),
    }
}

/// `Object#inspect` for parsed JSON values, in Ruby 3.4's format (`{"a" => 1}`).
pub fn json_value_inspect(v: &Value) -> String {
    match v {
        Value::Null => "nil".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                ruby_float_to_s(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => string_inspect(s),
        Value::Array(items) => format!("[{}]", items.iter().map(json_value_inspect).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => {
            if map.is_empty() {
                "{}".into()
            } else {
                format!(
                    "{{{}}}",
                    map.iter().map(|(k, v)| format!("{} => {}", string_inspect(k), json_value_inspect(v))).collect::<Vec<_>>().join(", ")
                )
            }
        }
    }
}

fn string_inspect(s: &str) -> String {
    let mut out = String::from("\"");
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{0c}' => out.push_str("\\f"),
            '\u{0b}' => out.push_str("\\v"),
            '\u{08}' => out.push_str("\\b"),
            '\u{07}' => out.push_str("\\a"),
            '\u{1b}' => out.push_str("\\e"),
            // What would start an interpolation in a double-quoted literal: `#{`, `#$`, `#@`
            '#' if chars.peek().is_some_and(|next| matches!(next, '{' | '$' | '@')) => out.push_str("\\#"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => out.push_str(&format!("\\x{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `Float#to_s`: plain decimals from 1e-4 up to (not including) 1e15, and above that while the
/// shortest digits still reach past the decimal point (`1000000000000000.1`); the exponent form
/// otherwise (`flo_to_s` in Ruby 3.4's numeric.c).
pub fn ruby_float_to_s(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let (digits, decpt) = shortest_digits(f.abs());
    let sign = if f < 0.0 { "-" } else { "" };
    if decpt < -3 || (decpt > 15 && digits.len() as i32 <= decpt) {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        let e = decpt - 1;
        format!("{sign}{first}.{rest}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else if decpt <= 0 {
        format!("{sign}0.{}{}", "0".repeat((-decpt) as usize), digits)
    } else if decpt as usize >= digits.len() {
        format!("{sign}{}{}.0", digits, "0".repeat(decpt as usize - digits.len()))
    } else {
        let (int, frac) = digits.split_at(decpt as usize);
        format!("{sign}{int}.{frac}")
    }
}

/// The shortest digits that read back as `magnitude`, and where the decimal point goes in them.
/// When two such forms are equally close, Ruby's dtoa takes the even one and Rust the upper one:
/// `667020902720176.25.to_s` is "667020902720176.2". Those ties take 16 or 17 digits, and Rust's
/// fixed-precision formatting breaks them to even.
fn shortest_digits(magnitude: f64) -> (String, i32) {
    let shortest = scientific_digits(&format!("{magnitude:e}"));
    let (digits, _) = &shortest;
    if digits.len() >= 16 && digits.ends_with(['1', '3', '5', '7', '9']) {
        let even = format!("{:.*e}", digits.len() - 1, magnitude);
        if even.parse::<f64>() == Ok(magnitude) {
            return scientific_digits(&even);
        }
    }
    shortest
}

/// The digits of `1.2345e6` and where the decimal point goes in them (7).
fn scientific_digits(formatted: &str) -> (String, i32) {
    let (mantissa, exponent) = formatted.split_once('e').unwrap();
    let digits = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    (digits, exponent.parse::<i32>().unwrap() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_to_s_like_ruby_3_4() {
        // `Float#to_s` in the reference (Ruby 3.4.10): from 1e15, the exponent form unless there
        // are digits after the decimal point.
        for (f, s) in [
            (100.0, "100.0"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.00012345, "0.00012345"),
            (1e-5, "1.0e-05"),
            (1.5e-7, "1.5e-07"),
            (5e-324, "5.0e-324"),
            (1e14, "100000000000000.0"),
            (123456789012345.6, "123456789012345.6"),
            (999999999999999.0, "999999999999999.0"),
            (999999999999999.9, "999999999999999.9"),
            (-999999999999999.0, "-999999999999999.0"),
            (1e15, "1.0e+15"),
            (-1e15, "-1.0e+15"),
            (1.5e15, "1.5e+15"),
            (1234567890123456.0, "1.234567890123456e+15"),
            (9007199254740992.0, "9.007199254740992e+15"),
            (1000000000000001.0, "1.000000000000001e+15"),
            (1963684456584958.8, "1963684456584958.8"),
            (1000000000000000.1, "1000000000000000.1"),
            (2251799813685248.5, "2251799813685248.5"),
            (-2551800308696183.5, "-2551800308696183.5"),
            (1e16, "1.0e+16"),
            (1e20, "1.0e+20"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(ruby_float_to_s(f), s, "{f:e}");
        }
    }

    #[test]
    fn float_to_s_breaks_ties_to_even_like_ruby() {
        // `Float#to_s` in the reference: of two shortest forms equally close, the even one, as long
        // as it reads back. Doubles are twice as dense just below 2^-24, so its even form
        // (5.960464477539062e-08) reads back as a different double.
        for (f, s) in [
            (667020902720176.0 + 0.25, "667020902720176.2"),
            (667020902720176.0 + 0.75, "667020902720176.8"),
            (1000000000000000.2, "1000000000000000.2"),
            (1125899906842624.0 + 0.25, "1125899906842624.2"),
            (-(2074704973491874.0 + 0.25), "-2074704973491874.2"),
            (24603114260468.0 + 0.0625, "24603114260468.062"),
            (210745403561986.0 + 0.125, "210745403561986.12"),
            (2f64.powi(-24), "5.960464477539063e-08"),
            (2f64.powi(-25), "2.9802322387695312e-08"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
        ] {
            assert_eq!(ruby_float_to_s(f), s, "{f:e}");
        }
    }

    #[test]
    fn matches_ruby() {
        assert_eq!(strip("\0 \t x \0\n"), "x");
        assert_eq!(strip(" \u{a0}x\u{a0} "), "\u{a0}x\u{a0}");
        assert_eq!(chomp_newlines("a\r\n\r\n"), "a");
        assert_eq!(chomp_newlines("a\n\r"), "a\n\r");
        assert_eq!(ruby_float_to_s(1e20), "1.0e+20");
        assert_eq!(ruby_float_to_s(1.0e-5), "1.0e-05");
        assert_eq!(ruby_float_to_s(100.0), "100.0");
        assert_eq!(ruby_float_to_s(1.5e16), "1.5e+16");
        assert_eq!(ruby_float_to_s(0.1), "0.1");
        assert_eq!(ruby_float_to_s(0.0001), "0.0001");
        assert_eq!(string_inspect("#{a} #$b #@c # #"), r##""\#{a} \#$b \#@c # #""##);
        assert_eq!(json_parse("{\"a\":1/*c*/}").unwrap()["a"], 1);
        assert!(json_parse("{\"a\":1,}").is_none());
        assert_eq!(truncate(&"a".repeat(300), 280, "…").chars().count(), 280);
    }
}
