//! Builds the `campfire_views::ViewContext` every page renders with: what the application
//! layout and the helpers read from `Current`, `request`, `flash`, the session and the config
//! (`reference/app/views/layouts/application.html.erb` and `app/helpers`).
//!
//! Gather the per-request data with [`Layout::load`] (it reads the database), then render inside
//! [`Layout::render`], which lends the templates a `ViewContext` for this request:
//!
//! ```ignore
//! let layout = Layout::load(c).await?;
//! let html = layout.render(c, |ctx| sessions::New { ctx, email_address, help_contact }.render())?;
//! Ok(layout.page(c, StatusCode::OK, html))
//! ```

use std::sync::LazyLock;

use campfire_db::{Account, User};
use campfire_kit::{Ctx, Error, Response, Result, StatusCode, format};
use campfire_views::{AccountSummary, CurrentUser, Platform, ViewContext};

use crate::app::AppCtx;
use crate::concerns;

/// Everything the layout needs, loaded before rendering.
#[derive(Debug, Clone)]
pub struct Layout {
    pub current_user: Option<CurrentUser>,
    pub account: AccountSummary,
    pub custom_styles: Option<String>,
    pub platform: Platform,
    pub last_room_visited_id: Option<i64>,
    pub vapid_public_key: Option<String>,
    pub app_version: String,
}

impl Layout {
    /// `Current.account`, `Current.user`, `last_room_visited` and the platform. With no account
    /// yet (first run) the account summary is blank: the pages that reference the account raise
    /// in Rails then, the others don't read it.
    pub async fn load(c: &Ctx) -> Result<Self> {
        let app = c.app();
        let secrets = app.secrets.clone();
        let user = concerns::current_user(c).cloned();
        let user_id = user.as_ref().map(|user| user.id);
        let last_room = concerns::last_room_cookie(c);
        // One trip to a reader for all three: each trip is a hand-off to a reader thread and back.
        let (account, has_logo, last_room_visited_id) = app
            .read(move |conn| {
                let account = Account::first(conn)?;
                let has_logo = match &account {
                    Some(account) => super::attachments::attached_blob(conn, "Account", account.id, "logo")?.is_some(),
                    None => false,
                };
                let last_room_visited_id = match user_id {
                    Some(user_id) => concerns::last_room_visited_in(conn, user_id, last_room)?.map(|room| room.id),
                    None => None,
                };
                Ok((account, has_logo, last_room_visited_id))
            })
            .await?;

        Ok(Self {
            current_user: user.as_ref().map(|user| current_user(&secrets, user)),
            account: account_summary(account.as_ref(), has_logo),
            custom_styles: account.and_then(|account| account.custom_styles),
            platform: super::accounts::platform(c),
            last_room_visited_id,
            vapid_public_key: app.vapid_public_key(),
            app_version: app.config.app_version.clone(),
        })
    }

    /// Renders with a `ViewContext` for this request. The flash is read (and so swept at the end
    /// of the request) the way the layout's `flash[:notice]` / `flash[:alert]` read it.
    pub fn render(&self, c: &mut Ctx, render: impl FnOnce(&ViewContext) -> askama::Result<String>) -> Result<String> {
        let flash_notice = c.flash().notice().map(str::to_string);
        let flash_alert = c.flash().alert().map(str::to_string);
        let base_url = c.url_for("");
        let request_url = c.request.url();
        let referrer = c.request.referer().map(str::to_string);
        let stylesheets = stylesheet_tags();

        let asset_path = |path: &str| campfire_assets::asset_path(path);
        let ctx = ViewContext {
            current_user: self.current_user.clone(),
            account: self.account.clone(),
            flash_notice,
            flash_alert,
            platform: self.platform.clone(),
            vapid_public_key: self.vapid_public_key.clone(),
            asset_path: &asset_path,
            importmap_tags: campfire_assets::javascript_importmap_tags(),
            stylesheet_tags: &stylesheets.html,
            custom_styles: self.custom_styles.clone(),
            cable_url: "/cable".into(),
            base_url,
            request_url,
            referrer,
            last_room_visited_id: self.last_room_visited_id,
            app_version: self.app_version.clone(),
        };
        render(&ctx).map_err(Error::internal)
    }

    /// A page rendered in the application layout: `text/html`, plus the `Link` preload header
    /// `stylesheet_link_tag` adds (`config.action_view.preload_links_header`).
    pub fn page(&self, c: &mut Ctx, status: StatusCode, html: String) -> Response {
        let links = &stylesheet_tags().preload_links;
        let existing = c.headers.get("link").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        c.set_header("link", &campfire_assets::append_preload_links(&existing, links));
        c.render(status, &format::HTML, html)
    }

    /// A page rendered in turbo-rails' frame layout (no stylesheets, so no `Link` header).
    pub fn frame(&self, c: &mut Ctx, status: StatusCode, html: String) -> Response {
        c.render(status, &format::HTML, html)
    }
}

/// The layout's `stylesheet_link_tag :all, "data-turbo-track": "reload"`: the assets are fixed at
/// build time, so it renders once per process.
pub fn stylesheet_tags() -> &'static campfire_assets::StylesheetTags {
    static TAGS: LazyLock<campfire_assets::StylesheetTags> =
        LazyLock::new(|| campfire_assets::stylesheet_link_tag_all(&[("data-turbo-track", "reload")]));
    &TAGS
}

/// `Current.user` as the layout's meta tags and helpers see it.
pub fn current_user(secrets: &rails_compat::Secrets, user: &User) -> CurrentUser {
    CurrentUser {
        id: user.id,
        name: user.name.clone(),
        administrator: user.can_administer(None, false),
        bot: user.is_bot(),
        avatar_url: super::avatar_path(secrets, user),
    }
}

/// `Current.account` for the layout: its name, `fresh_account_logo_path` and whether a logo is
/// attached.
pub fn account_summary(account: Option<&Account>, has_logo: bool) -> AccountSummary {
    AccountSummary {
        name: account.map(|account| account.name.clone()).unwrap_or_default(),
        logo_url: super::accounts::fresh_account_logo_path(account, None),
        has_logo,
    }
}

/// Renders a page in the application layout without the implicit render's template lookup: an
/// explicit `render template:` answers HTML whatever the request's format.
pub async fn page_in_any_format(c: &mut Ctx, status: StatusCode, full: impl FnOnce(&ViewContext) -> askama::Result<String>) -> Result {
    let layout = Layout::load(c).await?;
    let html = layout.render(c, full)?;
    Ok(layout.page(c, status, html))
}

/// Renders a page in the application layout, or, for templates that expose their `head`/`content`
/// blocks, turbo-rails' frame layout for a Turbo-Frame request
/// (`layout -> { "turbo_rails/frame" if turbo_frame_request? }`).
pub async fn page_or_frame(
    c: &mut Ctx,
    status: StatusCode,
    full: impl FnOnce(&ViewContext) -> askama::Result<String>,
    frame: impl FnOnce(&ViewContext) -> askama::Result<String>,
) -> Result {
    find_template(c, &format::HTML)?;
    page_or_frame_in_any_format(c, status, full, frame).await
}

/// [`page_or_frame`] without the template lookup (see [`page_in_any_format`]).
pub async fn page_or_frame_in_any_format(
    c: &mut Ctx,
    status: StatusCode,
    full: impl FnOnce(&ViewContext) -> askama::Result<String>,
    frame: impl FnOnce(&ViewContext) -> askama::Result<String>,
) -> Result {
    let layout = Layout::load(c).await?;
    if c.is_turbo_frame_request() {
        let html = layout.render(c, frame)?;
        Ok(layout.frame(c, status, html))
    } else {
        let html = layout.render(c, full)?;
        Ok(layout.page(c, status, html))
    }
}

/// The implicit render's template lookup (`default_render`): an action whose only template is
/// `<action>.html.erb` can't answer a request that doesn't accept HTML, which is
/// `ActionController::UnknownFormat` (406), and the response carries the template's format
/// whatever the `Accept` header preferred.
pub fn find_template(c: &mut Ctx, template: campfire_kit::Format) -> Result<()> {
    c.respond_to(&[template]).map(|_| ())
}
