//! Views for `reference/app/views/autocompletable`.

use askama::Template;
use serde::Serialize;

use crate::ViewContext;
use crate::helpers as h;
use crate::users::MentionUser;

/// `autocompletable/users/index.html.erb`: `<lexxy-prompt-item>`s for the mentions prompt,
/// rendered without a layout.
#[derive(Template)]
#[template(path = "autocompletable/users/index.html")]
pub struct UsersIndex<'a> {
    pub ctx: &'a ViewContext<'a>,
    pub users: Vec<MentionUser>,
}

/// `autocompletable/users/_prompt_item.html.erb` on its own.
#[derive(Template)]
#[template(path = "autocompletable/users/_prompt_item.html")]
pub struct PromptItem<'a> {
    pub ctx: &'a ViewContext<'a>,
    pub user: MentionUser,
}

/// `autocompletable/users/_user.json.jbuilder`; `index.json.jbuilder` is a JSON array of these.
#[derive(Clone, Debug, Serialize)]
pub struct UserJson {
    /// `h(user.name)`: HTML-escaped.
    pub name: String,
    pub value: i64,
    /// `fresh_user_avatar_url(user)`: absolute.
    pub avatar_url: String,
    pub sgid: String,
}

impl UserJson {
    pub fn new(user: &MentionUser, base_url: &str) -> Self {
        UserJson {
            name: h::escape(&user.name),
            value: user.id,
            avatar_url: format!("{base_url}{}", user.avatar_path),
            sgid: user.attachable_sgid.clone(),
        }
    }
}

/// `autocompletable/users/index.json.jbuilder`.
pub fn users_index_json(users: &[MentionUser], base_url: &str) -> String {
    let users: Vec<UserJson> = users.iter().map(|user| UserJson::new(user, base_url)).collect();
    rails_compat::json::encode(&users)
}
