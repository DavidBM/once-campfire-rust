//! Dumps the pages the database round-trip changes touch (every page's layout, the search index,
//! the back links) to $PAGEDUMP_DIR, for comparing builds byte for byte: the status, content type,
//! ETag, Link and Location headers and the body of each, as David with no `last_room` cookie, with
//! one for a room he's in, with one for a room he isn't in, and as Turbo-Frame requests, then two
//! anonymous pages. Copied into crates/campfire/src/pagedump.rs, with `#[cfg(test)] mod pagedump;`
//! added to main.rs (the test browser's `set_cookie` came with f1638d0; add it to test_support.rs
//! for older builds), to run:
//!
//!   PAGEDUMP_DIR=... CAMPFIRE_REQUIRE_SEED=1 cargo test -p campfire pagedump -- --ignored
//!
//! The profile page's sign-in link carries its expiry time, so it (and its ETag) differs from run
//! to run of the same build.

use axum::http::{Method, StatusCode};

use crate::controllers::presenters::test_support::*;

#[tokio::test]
#[ignore]
async fn dump_pages() {
    let dir = std::path::PathBuf::from(std::env::var("PAGEDUMP_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let app = TestApp::boot().await.unwrap();
    let mut n = 0;
    let mut dump = |name: &str, reply: Reply| {
        n += 1;
        let head = format!(
            "status {}\ncontent-type {:?}\netag {:?}\nlink {:?}\nlocation {:?}\n\n",
            reply.status,
            reply.content_type(),
            reply.header("etag"),
            reply.header("link"),
            reply.location()
        );
        std::fs::write(dir.join(format!("{n:02}-{name}.html")), [head.as_bytes(), &reply.body].concat()).unwrap();
    };

    let pages = [
        ("messages_page", format!("/rooms/{ALL_TALK}/messages?before=933434569")),
        ("sidebar", "/users/me/sidebar".to_string()),
        ("search_empty", "/searches".to_string()),
        ("search_coffee", "/searches?q=coffee".to_string()),
        ("search_hello", "/searches?q=hello".to_string()),
        ("open_new", "/rooms/opens/new".to_string()),
        ("closed_new", "/rooms/closeds/new".to_string()),
        ("account_edit", "/account/edit".to_string()),
        ("push_subscriptions", "/users/me/push_subscriptions".to_string()),
        ("closed_edit", format!("/rooms/closeds/{ALL_TALK}/edit")),
        ("open_edit", format!("/rooms/opens/{HQ}/edit")),
        ("direct_edit", format!("/rooms/directs/{DIRECT_DAVID_JASON}/edit")),
        ("message_show", format!("/rooms/{ALL_TALK}/messages/933434569")),
        ("user_profile", "/users/me/profile".to_string()),
        ("root", "/".to_string()),
        // Last: it sets the `last_room` cookie.
        ("room_show", format!("/rooms/{ALL_TALK}")),
    ];
    let mut david = app.david();
    for (name, path) in &pages {
        dump(&format!("{name}-nocookie"), david.get(path).await);
    }
    david.get(&format!("/rooms/{QUIET_CORNER}")).await;
    for (name, path) in &pages[..pages.len() - 1] {
        dump(&format!("{name}-quiet"), david.get(path).await);
    }
    david.set_cookie("last_room", &DIRECT_KEVIN_BENDER.to_string());
    for (name, path) in &pages[..pages.len() - 1] {
        dump(&format!("{name}-notmember"), david.get(path).await);
    }
    for (name, path) in &pages {
        dump(&format!("{name}-frame"), david.send(Req::new(Method::GET, path).header("turbo-frame", "messages")).await);
    }
    let mut anonymous = app.anonymous();
    dump("session_new", anonymous.get("/session/new").await);
    dump("anonymous_root", anonymous.get("/").await);
    assert_eq!(david.get("/up").await.status, StatusCode::OK);
}
