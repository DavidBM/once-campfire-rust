//! The Jbuilder views: `messages/_message.json`, `messages/by_bots/{index,show}.json`,
//! `messages/boosts/_boost.json` and `messages/boosts/by_bots/show.json`. Field order is the
//! JSON key order Jbuilder emits. Serialize with [`rails_compat::json::encode`].

use serde::{Deserialize, Serialize};

/// `users/_user.json.jbuilder`: `json.(user, :id, :name, :role)` and `avatar_url`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct UserJson {
    pub id: i64,
    pub name: String,
    /// "member", "administrator" or "bot".
    pub role: String,
    /// `fresh_user_avatar_url(user)`: a full URL.
    pub avatar_url: String,
}

/// `messages/_message.json.jbuilder`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MessageJson {
    pub id: i64,
    /// `message.created_at.utc`, formatted by [`super::support::json_time`].
    pub created_at: String,
    pub body: MessageBodyJson,
    pub creator: UserJson,
    pub room: IdJson,
    /// `room_message_url(message.room, message)`.
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MessageBodyJson {
    /// `message.plain_text_body`.
    pub plain_text: String,
    /// `message.body.to_s`: the rich text rendered with its layout.
    pub html: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IdJson {
    pub id: i64,
}

/// `messages/boosts/_boost.json.jbuilder`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BoostJson {
    pub id: i64,
    pub content: String,
    /// `boost.created_at.utc`, formatted by [`super::support::json_time`].
    pub created_at: String,
    pub booster: UserJson,
    pub message: BoostMessageJson,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BoostMessageJson {
    pub id: i64,
    /// `room_message_url(boost.message.room, boost.message)`.
    pub url: String,
}

/// `messages/by_bots/index.json.jbuilder`: `json.array! @messages, partial: "messages/message"`.
pub fn by_bots_index(messages: &[MessageJson]) -> String {
    rails_compat::json::encode(&messages)
}

/// `messages/by_bots/show.json.jbuilder`.
pub fn by_bots_show(message: &MessageJson) -> String {
    rails_compat::json::encode(message)
}

/// `messages/boosts/by_bots/show.json.jbuilder`.
pub fn boosts_by_bots_show(boost: &BoostJson) -> String {
    rails_compat::json::encode(boost)
}

// A Jbuilder fragment's payload is its JSON.
impl crate::fragment_cache::CacheSize for UserJson {
    fn cache_size(&self) -> usize {
        crate::fragment_cache::serialized_size(self)
    }
}

impl crate::fragment_cache::CacheSize for MessageJson {
    fn cache_size(&self) -> usize {
        crate::fragment_cache::serialized_size(self)
    }
}

impl crate::fragment_cache::CacheSize for BoostJson {
    fn cache_size(&self) -> usize {
        crate::fragment_cache::serialized_size(self)
    }
}
