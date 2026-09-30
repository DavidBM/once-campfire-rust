//! Ruby and Active Support string behaviors the pipeline depends on, beyond `ruby_compat`'s.

use serde_json::Value;

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
                ruby_compat::float_to_s(n.as_f64().unwrap_or(0.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ruby() {
        assert_eq!(chomp_newlines("a\r\n\r\n"), "a");
        assert_eq!(chomp_newlines("a\n\r"), "a\n\r");
        assert_eq!(json_value_inspect(&serde_json::json!({"a": [1.5e16, 0.1]})), r#"{"a" => [1.5e+16, 0.1]}"#);
        assert_eq!(string_inspect("#{a} #$b #@c # #"), r##""\#{a} \#$b \#@c # #""##);
        assert_eq!(json_parse("{\"a\":1/*c*/}").unwrap()["a"], 1);
        assert!(json_parse("{\"a\":1,}").is_none());
        assert_eq!(truncate(&"a".repeat(300), 280, "…").chars().count(), 280);
    }
}
