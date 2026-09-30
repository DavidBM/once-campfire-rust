//! Views for `reference/app/views/layouts`.
//!
//! A page template starts with `{% extends "layouts/application.html" %}`, fills the `content`
//! block (and `head`, `nav`, `footer`, `sidebar` for `content_for`), has a `ctx: &ViewContext`
//! field, and implements [`Page`] for what the ERB sets as `@page_title` and `@body_class`.

use askama::Template;

use crate::helpers as h;

/// The instance variables a page template hands to the application layout.
pub trait Page {
    /// `@page_title`; the layout falls back to "Campfire".
    fn page_title(&self) -> Option<String> {
        None
    }

    /// `@body_class`.
    fn body_class(&self) -> Option<&str> {
        None
    }

    /// What the page renders beyond its template's own text, when that's a lot: see
    /// [`render_page`].
    fn extra_capacity(&self) -> usize {
        0
    }
}

/// Renders `page` into a buffer with room for [`Page::extra_capacity`] more than its template's
/// own text (askama's `SIZE_HINT`, where `render` starts).
pub fn render_page<T: Template + Page>(page: &T) -> askama::Result<String> {
    render_with_capacity(page, page.extra_capacity())
}

/// Renders `template` into a buffer with room for `extra` more bytes than its own text. A page of
/// cached messages is ~400 KB; grown from the template's text, it would be copied into a buffer
/// twice the size each time one fills up.
pub fn render_with_capacity<T: Template>(template: &T, extra: usize) -> askama::Result<String> {
    let mut html = String::with_capacity(T::SIZE_HINT + extra);
    template.render_into(&mut html)?;
    Ok(html)
}

/// [`Page::extra_capacity`] for a page of messages: the messages, the layout's asset tags and
/// custom styles, and a margin for everything else the page interpolates (the parity seed's room
/// page has ~5 KB of that). Too much costs only memory: `Bytes` takes the `String` as it is.
pub fn messages_page_capacity(ctx: &crate::ViewContext, messages: &[crate::messages::MessageItem]) -> usize {
    crate::messages::MessageItem::html_len(messages) + ctx.layout_len() + 16 * 1024
}

/// The application layout around page parts rendered elsewhere, for templates that don't extend
/// the layout themselves (Rails picks the layout per request). Each part is what the ERB's
/// `content_for` / `yield` would have produced.
#[derive(Template)]
#[template(path = "layouts/application_wrapper.html")]
pub struct Application<'a> {
    pub ctx: &'a crate::ViewContext<'a>,
    /// `@page_title`.
    pub page_title: Option<String>,
    /// `@body_class`.
    pub body_class: Option<String>,
    pub head: h::Html,
    pub nav: h::Html,
    pub content: h::Html,
    pub footer: h::Html,
    pub sidebar: h::Html,
}

impl<'a> Application<'a> {
    /// Just a page body, with no title, body class or `content_for` regions.
    pub fn new(ctx: &'a crate::ViewContext<'a>, content: h::Html) -> Self {
        Application {
            ctx,
            page_title: None,
            body_class: None,
            head: h::empty(),
            nav: h::empty(),
            content,
            footer: h::empty(),
            sidebar: h::empty(),
        }
    }
}

impl Page for Application<'_> {
    fn page_title(&self) -> Option<String> {
        self.page_title.clone()
    }

    fn body_class(&self) -> Option<&str> {
        self.body_class.as_deref()
    }
}

/// turbo-rails' `layouts/turbo_rails/frame.html.erb`, used instead of the application layout
/// whenever a request carries a `Turbo-Frame` header. Pages expose their blocks for it through
/// askama's `blocks = ["head", "content"]` (see [`frame`]).
#[derive(Template)]
#[template(path = "layouts/turbo_rails/frame.html")]
pub struct FrameLayout<'a> {
    pub ctx: &'a crate::ViewContext<'a>,
    /// The page's `:head` content.
    pub head: h::Html,
    /// The page itself.
    pub content: h::Html,
}

/// Renders a page's `head` and `content` blocks in the Turbo-Frame layout:
/// `frame(ctx, page.as_head(), page.as_content())`.
pub fn frame(ctx: &crate::ViewContext, head: impl Template, content: impl Template) -> askama::Result<String> {
    FrameLayout { ctx, head: h::raw(head.render()?), content: h::raw(content.render()?) }.render()
}
