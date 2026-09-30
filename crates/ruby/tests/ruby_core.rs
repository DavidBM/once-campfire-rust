//! Every function against what Ruby, Rack, Active Record and Addressable answer in the reference
//! (`vectors/ruby_core.json`, written by `reference-tools/ruby_core.rb`). Each test lists every
//! input it gets wrong, not just the first.

use std::fmt::Debug;
use std::sync::LazyLock;

use serde_json::Value;

static VECTORS: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../../vectors/ruby_core.json")).expect("vectors/ruby_core.json"));

fn cases(section: &str) -> &'static [Value] {
    VECTORS[section].as_array().expect(section)
}

/// Each string, with what Ruby's version of `function` made of it.
fn strings(function: &'static str) -> impl Iterator<Item = (&'static str, &'static Value)> {
    cases("strings").iter().map(move |case| (case["input"].as_str().unwrap(), &case[function]))
}

fn assert_matches_ruby<T: PartialEq + Debug>(function: &str, results: impl IntoIterator<Item = (String, T, T)>) {
    let mismatches: Vec<String> = results
        .into_iter()
        .filter(|(_, rust, ruby)| rust != ruby)
        .map(|(input, rust, ruby)| format!("{function}({input}) is {rust:?}, Ruby's {ruby:?}"))
        .collect();
    assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
}

fn same_strings(function: &'static str, rust: impl Fn(&str) -> String) {
    assert_matches_ruby(
        function,
        strings(function).map(|(input, ruby)| (format!("{input:?}"), rust(input), ruby.as_str().unwrap().to_string())),
    );
}

/// A float compared bit for bit, so -0.0 isn't 0.0.
#[derive(Debug)]
struct Bits(f64);

impl PartialEq for Bits {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

#[test]
fn to_i() {
    assert_matches_ruby(
        "to_i",
        strings("to_i").map(|(input, ruby)| {
            let ruby = ruby.as_str().unwrap();
            let checked = ruby.parse::<i64>().ok();
            let saturated = checked.unwrap_or(if ruby.starts_with('-') { i64::MIN } else { i64::MAX });
            (format!("{input:?}"), (ruby_compat::to_i(input), ruby_compat::to_i_checked(input)), (saturated, checked))
        }),
    );
}

#[test]
fn integer_cast() {
    // `SQLite3Integer#serialize`: nil, or a RangeError past 8 bytes.
    assert_matches_ruby(
        "integer_cast",
        strings("integer_cast").map(|(input, ruby)| {
            let ruby = ruby.as_str().and_then(|s| s.parse::<i64>().ok());
            (format!("{input:?}"), ruby_compat::integer_cast(input), ruby)
        }),
    );
}

#[test]
fn to_f() {
    assert_matches_ruby(
        "to_f",
        strings("to_f").map(|(input, ruby)| {
            let ruby: f64 = ruby.as_str().unwrap().parse().unwrap();
            (format!("{input:?}"), Bits(ruby_compat::to_f(input)), Bits(ruby))
        }),
    );
}

#[test]
fn strip() {
    same_strings("strip", |s| ruby_compat::strip(s).to_string());
}

#[test]
fn html_escape() {
    same_strings("html_escape", ruby_compat::erb::html_escape);
}

#[test]
fn cgi_escape() {
    same_strings("cgi_escape", ruby_compat::cgi_escape);
}

#[test]
fn url_encode() {
    same_strings("url_encode", ruby_compat::url_encode);
    // And Addressable's `encode_component(s, UNRESERVED)`, which pagination links use.
    same_strings("addressable_unreserved", ruby_compat::url_encode);
}

#[test]
fn float_to_s() {
    assert_matches_ruby(
        "float_to_s",
        cases("floats").iter().map(|case| {
            let float = f64::from_bits(u64::from_str_radix(case["bits"].as_str().unwrap(), 16).unwrap());
            (format!("{float:e}"), ruby_compat::float_to_s(float), case["to_s"].as_str().unwrap().to_string())
        }),
    );
}

#[test]
fn byte_ranges() {
    assert_matches_ruby(
        "byte_ranges",
        cases("byte_ranges").iter().map(|case| {
            let header = case["header"].as_str();
            let size = case["size"].as_u64().unwrap();
            let ruby = case["ranges"]
                .as_array()
                .map(|ranges| ranges.iter().map(|range| (range[0].as_u64().unwrap(), range[1].as_u64().unwrap())).collect::<Vec<_>>());
            (format!("{header:?}, {size}"), ruby_compat::rack::byte_ranges(header, size), ruby)
        }),
    );
}
