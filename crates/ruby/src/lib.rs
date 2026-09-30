//! Ruby's own string behaviour, as Rails and Rack apply it, in one place for every crate: ERB
//! escaping, `String#to_i`, `#to_f` and `#strip`, `Float#to_s`, `CGI.escape`,
//! `ERB::Util.url_encode`, how Active Record binds an integer and Rack's byte ranges. Each is
//! checked against Ruby itself on every character from U+0000 to U+00FF and a list of edge cases
//! (`vectors/ruby_core.json`, written by `reference-tools/ruby_core.rb`).
//!
//! It has no dependencies, so the crates that use it don't wait on anything to compile.

pub mod erb;
pub mod rack;

mod float;
mod integer;
mod string;
mod uri;

pub use float::{float_to_s, to_f};
pub use integer::{integer_cast, to_i, to_i_checked};
pub use string::strip;
pub use uri::{cgi_escape, url_encode};
