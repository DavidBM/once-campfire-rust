//! Page renders into a buffer sized from the last render at the same call site. Askama's
//! `render()` reserves the template's `SIZE_HINT`, a guess from its literal text, so a room page's
//! `String` would grow by `realloc` several times. Only the page's text goes into the buffer: its
//! cached fragments are [`recorded`](crate::recorded) instead.

use std::sync::atomic::{AtomicUsize, Ordering};

use askama::Template;

use crate::recorded::{self, RecordedPage};

/// The most a render reserves up front. Only the last render's length is kept, so one huge page
/// sizes just the next render; this cap bounds even that one.
const MAX_RESERVE: usize = 1 << 20;

/// The length of the last page rendered at one call site (see [`render_sized!`]).
#[derive(Debug, Default)]
pub struct RenderSize(AtomicUsize);

impl RenderSize {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    /// `template.render()`, recorded into a text buffer sized from the last render here.
    pub fn render<T: Template>(&self, template: &T) -> askama::Result<RecordedPage> {
        let page = recorded::render(template, self.capacity(T::SIZE_HINT))?;
        self.0.store(page.text().len(), Ordering::Relaxed);
        Ok(page)
    }

    /// The last length plus an eighth, so a page a little longer than the last still fits.
    fn capacity(&self, size_hint: usize) -> usize {
        let last = self.0.load(Ordering::Relaxed);
        (last + last / 8).min(MAX_RESERVE).max(size_hint)
    }
}

/// [`RenderSize::render`] at this call site (each expansion has its own [`RenderSize`]):
/// `render_sized!(rooms::Show { ctx, show: &show })`.
#[macro_export]
macro_rules! render_sized {
    ($template:expr) => {{
        static SIZE: $crate::sized::RenderSize = $crate::sized::RenderSize::new();
        SIZE.render(&$template)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Template)]
    #[template(source = "<p>{{ body }}</p>", ext = "html")]
    struct Page<'a> {
        body: &'a str,
    }

    #[test]
    fn reserves_the_last_length_with_headroom_up_to_a_cap() {
        let size = RenderSize::new();
        let body = "x".repeat(8000);
        size.render(&Page { body: &body }).unwrap();
        assert_eq!(size.capacity(0), 8007 + 8007 / 8);
        // A shorter page sizes the next render: one long page doesn't stay reserved.
        size.render(&Page { body: &body[..7000] }).unwrap();
        assert_eq!(size.capacity(0), 7007 + 7007 / 8);
        size.render(&Page { body: &"x".repeat(2 * MAX_RESERVE) }).unwrap();
        assert_eq!(size.capacity(0), MAX_RESERVE);
    }

    #[test]
    fn each_call_site_has_its_own_size() {
        let short = || render_sized!(Page { body: "short" }).unwrap();
        let long = "x".repeat(4000);
        render_sized!(Page { body: &long }).unwrap();
        assert!(short().into_parts().0.capacity() < 100);
    }
}
