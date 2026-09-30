//! `SearchesController` (reference/app/controllers/searches_controller.rb). `set_messages` runs
//! before every action, so a query that FTS5 rejects fails `create` and `clear` too.

use campfire_db::{Message, Search};
use campfire_kit::{Ctx, Error, Result, StatusCode};
use campfire_views::searches::{Index, IndexView, search_path};

use crate::app::AppCtx;
use crate::concerns::{self, Before, before_actions, require_current_user};
use crate::controllers::messages::present;
use crate::controllers::presenters::page;

pub async fn index(c: &mut Ctx) -> Result {
    before_actions(c, Before::default()).await?;
    let q = query_param(c)?;
    let messages = set_messages(c, q.as_deref()).await?;
    let user_id = require_current_user(c)?.id;
    let query = query(q.as_deref()).filter(|query| is_present(query));
    let recent_searches: Vec<String> =
        c.app().read(move |conn| Ok(Search::ordered_for_user(conn, user_id)?.into_iter().map(|search| search.query).collect())).await?;
    let return_to_room_id = concerns::last_room_visited(c).await?.map(|room| room.id).unwrap_or_default();
    let messages = present(c, move |presenter| presenter.messages(&messages)).await?;
    let index = IndexView { query, q, messages, recent_searches, return_to_room_id };
    let response = page::framed_page!(c, StatusCode::OK, |ctx| Index { ctx, index: &index }).await?;
    let fragments = campfire_views::messages::MessageItem::cached_fragments(&c.app().fragment_cache, &index.messages);
    Ok(response.with_cached_fragments(fragments))
}

pub async fn create(c: &mut Ctx) -> Result {
    before_actions(c, Before::default()).await?;
    let q = query_param(c)?;
    set_messages(c, q.as_deref()).await?;
    let user_id = require_current_user(c)?.id;
    let query = query(q.as_deref());
    // Current.user.searches.record(query): a nil query violates `query`'s NOT NULL.
    let recorded = query.clone().ok_or_else(|| Error::internal(anyhow::anyhow!("NOT NULL constraint failed: searches.query")))?;
    c.app().write(move |tx| Search::record(tx, user_id, &recorded).map(|_| ())).await?;
    let path = match &query {
        Some(query) => search_path(query),
        None => campfire_routes::searches(),
    };
    let url = c.url_for(&path);
    c.redirect_to(&url)
}

pub async fn clear(c: &mut Ctx) -> Result {
    before_actions(c, Before::default()).await?;
    let q = query_param(c)?;
    set_messages(c, q.as_deref()).await?;
    let user_id = require_current_user(c)?.id;
    c.app().write(move |tx| Search::destroy_all_for_user(tx, user_id)).await?;
    let url = c.url_for(&campfire_routes::searches());
    c.redirect_to(&url)
}

/// `params[:q]`; anything but a string makes `gsub` raise.
fn query_param(c: &Ctx) -> Result<Option<String>> {
    match c.param("q") {
        None => Ok(None),
        Some(param) if param.is_null() => Ok(None),
        Some(param) => {
            param.as_str().map(|q| Some(q.to_string())).ok_or_else(|| Error::internal(anyhow::anyhow!("undefined method 'gsub'")))
        }
    }
}

/// `params[:q]&.gsub(/[^[:word:]]/, " ")`
pub fn query(q: Option<&str>) -> Option<String> {
    crate::integrations::search::sanitize_query(q)
}

fn is_present(value: &str) -> bool {
    !value.chars().all(char::is_whitespace)
}

/// `set_messages`: `Current.user.reachable_messages.search(query).last(100)` when there's a query.
async fn set_messages(c: &Ctx, q: Option<&str>) -> Result<Vec<Message>> {
    let Some(query) = query(q).filter(|query| is_present(query)) else { return Ok(Vec::new()) };
    let user_id = require_current_user(c)?.id;
    c.app().read(move |conn| Message::search_reachable(conn, user_id, &query)).await
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};

    use super::query;
    use crate::controllers::presenters::test_support::*;

    #[tokio::test]
    async fn searching_records_and_clears_recent_searches() {
        let Some(app) = TestApp::boot().await else { return };
        let mut david = app.david();
        let empty = david.get("/searches").await;
        assert_eq!(empty.status, StatusCode::OK);
        assert!(empty.text().contains("<title>Search</title>"));

        let recorded = david.write(Req::new(Method::POST, "/searches").form(&[("q", "hello, world")])).await;
        assert_eq!(recorded.location(), Some("http://campfire.test/searches?q=hello++world"));
        let results = david.get("/searches?q=hello++world").await;
        assert_eq!(results.status, StatusCode::OK);
        assert!(results.text().contains("“hello  world”"), "{}", results.text());

        let cleared = david.write(Req::new(Method::DELETE, "/searches/clear")).await;
        assert_eq!(cleared.location(), Some("http://campfire.test/searches"));
        let count = app.db().read(|conn| campfire_db::Search::count_for_user(conn, DAVID)).await.unwrap();
        assert_eq!(count, 0);

        let missing = david.write(Req::new(Method::POST, "/searches")).await;
        assert_eq!(missing.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn non_word_characters_become_spaces() {
        assert_eq!(query(Some("hello, world!")).as_deref(), Some("hello  world "));
        assert_eq!(query(Some("café_1 日本")).as_deref(), Some("café_1 日本"));
        assert_eq!(query(Some("\"quoted\" AND-x")).as_deref(), Some(" quoted  AND x"));
        assert_eq!(query(None), None);
    }
}
