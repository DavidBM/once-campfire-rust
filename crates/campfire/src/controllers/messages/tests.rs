//! Request-level tests for the message, boost and bot API controllers, against the `default`
//! parity seed.

use axum::http::{Method, StatusCode};
use campfire_db::{Boost, Message, PushSubscription, Webhook};

use crate::controllers::presenters::test_support::*;

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 4, 0, 0, 0, 3, 8, 2, 0, 0, 0, 59, 150, 57, 145, 0, 0, 0, 16, 73,
    68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 71, 12, 56, 57, 0, 245, 49, 11, 245, 53, 123, 251, 130, 0, 0, 0, 0, 73, 69, 78, 68, 174,
    66, 96, 130,
];

const TURBO_STREAM_ACCEPT: &str = "text/vnd.turbo-stream.html, text/html, application/xhtml+xml";

async fn messages_in(app: &TestApp, room_id: i64) -> Vec<Message> {
    let mut messages = app.db().read(move |conn| Message::for_room(conn, room_id)).await.unwrap();
    messages.sort_by_key(|m| (m.created_at, m.id));
    messages
}

#[tokio::test]
async fn index_pages_with_conditional_gets() {
    let Some(app) = TestApp::boot().await else { return };
    let messages = messages_in(&app, ALL_TALK).await;
    let mut david = app.david();

    let reply = david.get(&format!("/rooms/{ALL_TALK}/messages?before={}", messages[50].id)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    assert_eq!(reply.text().matches(r#"data-controller="reply""#).count(), 40);
    assert!(!reply.text().contains("<html"), "layout false");
    let etag = reply.header("etag").unwrap().to_string();
    assert!(etag.starts_with("W/\""));
    assert!(reply.header("last-modified").is_some());

    let cached = david
        .send(Req::new(Method::GET, &format!("/rooms/{ALL_TALK}/messages?before={}", messages[50].id)).header("if-none-match", &etag))
        .await;
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);

    let after_last = david.get(&format!("/rooms/{ALL_TALK}/messages?after={}", messages.last().unwrap().id)).await;
    assert_eq!(after_last.status, StatusCode::NO_CONTENT);
    assert_eq!(david.get(&format!("/rooms/{ALL_TALK}/messages?before=0")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(david.get(&format!("/rooms/{DIRECT_KEVIN_BENDER}/messages")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(david.get("/messages").await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_appends_the_message_as_a_turbo_stream() {
    let Some(app) = TestApp::boot().await else { return };
    let mut david = app.david();
    let reply = david
        .write(
            Req::new(Method::POST, &format!("/rooms/{ALL_TALK}/messages"))
                .header("accept", TURBO_STREAM_ACCEPT)
                .form(&[("message[body]", "<p>Hello <strong>there</strong></p>"), ("message[client_message_id]", "abc-123")]),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    assert_eq!(reply.content_type(), Some("text/vnd.turbo-stream.html; charset=utf-8"));
    assert!(reply.text().contains(r#"<turbo-stream action="append" target="messages_rooms_closed_486777696">"#), "{}", reply.text());
    assert!(reply.text().contains(r#"id="message_abc-123""#));
    assert!(reply.text().contains("Hello <strong>there</strong>"));

    let message = messages_in(&app, ALL_TALK).await.pop().unwrap();
    assert_eq!((message.client_message_id.as_str(), message.creator_id), ("abc-123", DAVID));
}

#[tokio::test]
async fn create_in_a_room_you_left_renders_room_not_found() {
    let Some(app) = TestApp::boot().await else { return };
    let mut david = app.david();
    let reply =
        david.write(Req::new(Method::POST, &format!("/rooms/{DIRECT_KEVIN_BENDER}/messages")).form(&[("message[body]", "hi")])).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.text().contains("This room was deleted."));
    assert!(reply.text().contains("<html"), "in the application layout");

    let missing = david.write(Req::new(Method::POST, &format!("/rooms/{ALL_TALK}/messages")).form(&[("body", "hi")])).await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    let html = david.write(Req::new(Method::POST, &format!("/rooms/{ALL_TALK}/messages")).form(&[("message[body]", "hi")])).await;
    assert_eq!(html.status, StatusCode::NOT_ACCEPTABLE, "only a turbo stream template");
}

#[tokio::test]
async fn uploads_attach_and_process_the_file() {
    let Some(app) = TestApp::boot().await else { return };
    let mut david = app.david();
    let reply = david
        .write(
            Req::new(Method::POST, &format!("/rooms/{ALL_TALK}/messages"))
                .header("accept", "*/*")
                .multipart(&[("message[client_message_id]", "upload-1")], ("message[attachment]", "red.png", "image/png", PNG)),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    assert!(reply.text().contains("/rails/active_storage/representations/redirect/"), "{}", reply.text());
    assert!(reply.text().contains(r#"width="4" height="3""#), "analyzed dimensions: {}", reply.text());

    let message = messages_in(&app, ALL_TALK).await.pop().unwrap();
    assert_eq!(message.client_message_id, "upload-1");
    let (_, blob) = app.db().read(move |conn| message.attachment(conn)).await.unwrap().unwrap();
    assert_eq!(blob.filename, "red.png");
    let variants: i64 = app
        .db()
        .read(move |conn| {
            Ok(conn.query_row("SELECT count(*) FROM active_storage_variant_records WHERE blob_id = ?", [blob.id], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(variants, 1, "the :thumb variant is processed");
}

/// Rails raises reading this body's plain text (`ArgumentError: invalid base64`), after the
/// create commits; here it's saved with no plain text, or its attachment's filename (see "Known
/// differences" in the README).
const MENTION_WITH_A_BAD_SGID: &str =
    r#"<p>Hey <action-text-attachment sgid="!!!" content-type="application/vnd.campfire.mention"></action-text-attachment></p>"#;

const UNRENDERABLE: &str = "Failed to load message content";

/// What search, a push and a bot's webhook get for a message.
async fn plain_texts(app: &TestApp, message: Message) -> (String, String, String) {
    let db = app.db().clone();
    app.db()
        .read(move |conn| {
            let rich_text = &*db.env().rich_text;
            let indexed = conn.query_row("SELECT body FROM message_search_index WHERE rowid = ?", [message.id], |r| r.get(0))?;
            let (push, _, _) = PushSubscription::pushes_for(conn, rich_text, &message, db.env().now())?;
            let webhook = Webhook::find_by_user(conn, BENDER)?.unwrap().payload(conn, rich_text, &message, "/bot", "/message")?;
            let webhook: serde_json::Value = serde_json::from_str(&webhook).unwrap();
            Ok((indexed, push.body, webhook["message"]["body"]["plain"].as_str().unwrap().to_string()))
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn a_message_whose_plain_text_raises_goes_out_without_one() {
    let Some(app) = TestApp::boot().await else { return };
    let mut david = app.david();
    let room = format!("/rooms/{ALL_TALK}");
    let unrenderable_before = david.get(&room).await.text().matches(UNRENDERABLE).count();

    let reply = david
        .write(
            Req::new(Method::POST, &format!("/rooms/{ALL_TALK}/messages"))
                .header("accept", TURBO_STREAM_ACCEPT)
                .form(&[("message[body]", MENTION_WITH_A_BAD_SGID), ("message[client_message_id]", "bad-sgid")]),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    assert!(reply.text().contains(UNRENDERABLE), "{}", reply.text());
    let message = messages_in(&app, ALL_TALK).await.pop().unwrap();
    assert_eq!(message.client_message_id, "bad-sgid");
    assert_eq!(plain_texts(&app, message.clone()).await, ("".into(), "David: ".into(), "".into()));

    let page = david.get(&room).await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.text().matches(UNRENDERABLE).count(), unrenderable_before + 1, "{}", page.text());
    let shown = david.get(&format!("{room}/messages/{}", message.id)).await;
    assert_eq!(shown.status, StatusCode::OK);
    assert!(shown.text().contains(UNRENDERABLE), "{}", shown.text());

    let reply = david
        .write(Req::new(Method::POST, &format!("/rooms/{ALL_TALK}/messages")).header("accept", TURBO_STREAM_ACCEPT).multipart(
            &[("message[body]", MENTION_WITH_A_BAD_SGID), ("message[client_message_id]", "bad-sgid-upload")],
            ("message[attachment]", "red.png", "image/png", PNG),
        ))
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    let message = messages_in(&app, ALL_TALK).await.pop().unwrap();
    assert_eq!(message.client_message_id, "bad-sgid-upload");
    assert_eq!(plain_texts(&app, message).await, ("red.png".into(), "David: red.png".into(), "red.png".into()));
}

#[tokio::test]
async fn show_edit_update_and_destroy() {
    let Some(app) = TestApp::boot().await else { return };
    let message = messages_in(&app, ALL_TALK).await.into_iter().rev().find(|m| m.creator_id == DAVID).unwrap();
    let mut david = app.david();
    let path = format!("/rooms/{ALL_TALK}/messages/{}", message.id);

    let shown = david.get(&path).await;
    assert_eq!(shown.status, StatusCode::OK);
    assert!(shown.text().contains("<html"));
    let framed = david.send(Req::new(Method::GET, &path).header("turbo-frame", "message_x")).await;
    // MessagesController declares its own layout, so Turbo-Frame requests get the application
    // layout too (not turbo-rails' frame layout).
    assert!(framed.text().starts_with("<!DOCTYPE html>"), "{}", framed.text());

    let edit = david.get(&format!("{path}/edit")).await;
    assert_eq!(edit.status, StatusCode::OK);
    assert!(edit.text().contains("<lexxy-editor"));

    let updated = david.write(Req::new(Method::PATCH, &path).form(&[("message[body]", "<p>Edited</p>")])).await;
    assert_eq!(updated.status, StatusCode::FOUND, "{}", updated.text());
    assert_eq!(updated.location(), Some(format!("http://campfire.test{path}").as_str()));
    let body = app.db().read(move |conn| Message::find(conn, message.id)?.body_html(conn)).await.unwrap();
    assert_eq!(body.as_deref(), Some("<p>Edited</p>"));

    let json = david.write(Req::new(Method::PATCH, &format!("{path}.json")).form(&[("message[body]", "x")])).await;
    assert_eq!(json.status, StatusCode::INTERNAL_SERVER_ERROR, "no messages/show.json");

    let destroyed = david.write(Req::new(Method::DELETE, &path).header("accept", TURBO_STREAM_ACCEPT)).await;
    assert_eq!(destroyed.status, StatusCode::OK);
    assert_eq!(
        destroyed.text().trim(),
        format!(r#"<turbo-stream action="remove" target="message_{}"></turbo-stream>"#, message.client_message_id)
    );
    assert!(app.db().read(move |conn| Message::find_by_id(conn, message.id)).await.unwrap().is_none());
    assert_eq!(david.get(&path).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn boosts_are_listed_created_and_removed() {
    let Some(app) = TestApp::boot().await else { return };
    let message = messages_in(&app, ALL_TALK).await.pop().unwrap();
    let mut david = app.david();
    let path = format!("/messages/{}/boosts", message.id);
    assert_eq!(david.get(&path).await.status, StatusCode::OK);
    assert_eq!(david.get(&format!("{path}/new")).await.status, StatusCode::OK);
    assert_eq!(david.get(&format!("{path}/1")).await.status, StatusCode::NOT_FOUND);

    let created = david.write(Req::new(Method::POST, &path).form(&[("boost[content]", "🔥")])).await;
    assert_eq!(created.location(), Some(format!("http://campfire.test{path}").as_str()));
    let boost = app.db().read(move |conn| Boost::for_message(conn, message.id)).await.unwrap().pop().unwrap();
    assert_eq!((boost.content.as_str(), boost.booster_id), ("🔥", DAVID));

    let destroyed = david.write(Req::new(Method::DELETE, &format!("{path}/{}", boost.id))).await;
    assert_eq!(destroyed.status, StatusCode::NO_CONTENT);
    let again = david.write(Req::new(Method::DELETE, &format!("{path}/{}", boost.id))).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert_eq!(david.get(&format!("/messages/{}/boosts", i64::MAX)).await.status, StatusCode::NOT_FOUND);
}

/// Message fragments and the bot API's JSON are cached for every request, so a request with a
/// forged Host mustn't leave its URLs in them for the next one.
#[tokio::test]
async fn a_forged_host_stays_out_of_the_caches() {
    let Some(app) = TestApp::boot().await else { return };
    let forged = |path: &str| Req::new(Method::GET, path).header("x-forwarded-host", "evil.example");

    let mut bot = app.anonymous();
    let api = format!("/rooms/{ALL_TALK}/{BENDER_KEY}/messages");
    assert!(bot.send(forged(&api)).await.text().contains("http://evil.example/"));
    let honest = bot.get(&api).await;
    assert_eq!(honest.status, StatusCode::OK);
    assert!(!honest.text().contains("evil.example"), "{}", honest.text());

    let mut david = app.david();
    let room = format!("/rooms/{ALL_TALK}");
    assert_eq!(david.send(forged(&room)).await.status, StatusCode::OK);
    let honest = app.david().get(&room).await;
    assert_eq!(honest.status, StatusCode::OK);
    assert!(honest.text().contains("data-copy-to-clipboard-url-value=\"/rooms/"));
    assert!(!honest.text().contains("evil.example"));
}

#[tokio::test]
async fn the_bot_api() {
    let Some(app) = TestApp::boot().await else { return };
    let mut bot = app.anonymous();
    let base = format!("/rooms/{ALL_TALK}/{BENDER_KEY}/messages");

    let index = bot.get(&base).await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.text());
    assert_eq!(index.content_type(), Some("application/json; charset=utf-8"));
    assert_eq!(index.header("x-total-count"), Some("131"));
    let page = index.json();
    assert_eq!(page.as_array().unwrap().len(), 40);
    let first_id = page[0]["id"].as_i64().unwrap();
    assert_eq!(index.header("link"), Some(format!("<http://campfire.test{base}?before={first_id}>; rel=\"next\"").as_str()));
    let keys: Vec<&str> = page[0].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["id", "created_at", "body", "creator", "room", "url"]);

    let created = bot.send(Req::new(Method::POST, &base).body("Beep boop")).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let message = messages_in(&app, ALL_TALK).await.pop().unwrap();
    assert_eq!(created.location(), Some(format!("http://campfire.test/messages/{}", message.id).as_str()));
    assert_eq!(message.creator_id, BENDER);

    assert_eq!(bot.send(Req::new(Method::POST, &base).body("  \n")).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let upload = bot.send(Req::new(Method::POST, &base).multipart(&[], ("attachment", "red.png", "image/png", PNG))).await;
    assert_eq!(upload.status, StatusCode::CREATED);

    let updated = bot.send(Req::new(Method::PUT, &format!("{base}/{}", message.id)).body("Beep edited")).await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text());
    assert_eq!(updated.json()["body"]["plain_text"], "Beep edited");

    // An attachment replaces the message's attachment (and keeps the body), like Rails'
    // `update!(attachment:)`; a second one replaces the first; "" removes it.
    let attached = |app: &TestApp, id: i64| {
        let db = app.db().clone();
        async move { db.read(move |conn| Message::find(conn, id)?.attachment(conn)).await.unwrap().map(|(_, blob)| blob) }
    };
    let put_file = |name: &'static str| {
        Req::new(Method::PUT, &format!("{base}/{}", message.id)).multipart(&[], ("attachment", name, "image/png", PNG))
    };
    let with_file = bot.send(put_file("red.png")).await;
    assert_eq!(with_file.status, StatusCode::OK, "{}", with_file.text());
    assert_eq!(with_file.json()["body"]["plain_text"], "Beep edited");
    let first = attached(&app, message.id).await.expect("attached");
    assert_eq!(first.filename, "red.png");
    assert_eq!(bot.send(put_file("again.png")).await.status, StatusCode::OK);
    let second = attached(&app, message.id).await.expect("replaced");
    assert_eq!(second.filename, "again.png");
    assert_ne!(first.id, second.id);
    let removed = bot.send(Req::new(Method::PUT, &format!("{base}/{}", message.id)).form(&[("attachment", "")])).await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.text());
    assert!(attached(&app, message.id).await.is_none());

    let boost = bot.send(Req::new(Method::POST, &format!("{base}/{}/boosts", message.id)).body("🤖")).await;
    assert_eq!(boost.status, StatusCode::CREATED, "{}", boost.text());
    assert_eq!(boost.json()["content"], "🤖");
    let boost_id = boost.json()["id"].as_i64().unwrap();
    let removed = bot.send(Req::new(Method::DELETE, &format!("{base}/{}/boosts/{boost_id}", message.id))).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let missing = bot.send(Req::new(Method::DELETE, &format!("{base}/{}/boosts/{boost_id}", message.id))).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let destroyed = bot.send(Req::new(Method::DELETE, &format!("{base}/{}", message.id))).await;
    assert_eq!(destroyed.status, StatusCode::NO_CONTENT);

    // Rooms the bot isn't in, other people's messages, and bad keys.
    assert_eq!(bot.get(&format!("/rooms/{DIRECT_DAVID_JASON}/{BENDER_KEY}/messages")).await.status, StatusCode::NOT_FOUND);
    let not_mine = messages_in(&app, ALL_TALK).await.into_iter().find(|m| m.creator_id == DAVID).unwrap();
    let forbidden = bot.send(Req::new(Method::DELETE, &format!("{base}/{}", not_mine.id))).await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
    let bad_key = bot.get(&format!("/rooms/{ALL_TALK}/1-nope/messages")).await;
    assert_eq!(bad_key.status, StatusCode::FOUND);
    // A bot key doesn't open the rest of the app.
    let denied = bot.get(&format!("/rooms/{ALL_TALK}/messages?bot_key={BENDER_KEY}")).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
}
