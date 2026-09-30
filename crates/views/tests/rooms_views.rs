//! DOM parity of the room views with the reference app (goldens in tests/golden/b).

mod messages_support;

use std::sync::Arc;

use askama::Template;
use campfire_views::layouts::{Page, render_page};
use campfire_views::messages::MessageItem;
use campfire_views::rooms::{self, ClosedFormView, DirectEditView, InvolvementView, OpenFormView, RefreshView, ShowView};
use messages_support::golden;

fn show(name: &str) {
    let g = golden(name);
    let show: ShowView = g.input();
    g.assert_dom(&g.render(|ctx| {
        let page = rooms::Show { ctx, show: &show };
        let html = render_page(&page).unwrap();
        assert_eq!(html, page.render().unwrap(), "the same page, however the buffer starts");
        html
    }));
}

#[test]
fn a_room_of_cached_messages_renders_into_a_buffer_it_fits() {
    let g = golden("rooms_show_member");
    let mut show: ShowView = g.input();
    show.messages = (0..40)
        .map(|n| MessageItem::Fragment {
            client_message_id: n.to_string(),
            room_id: show.room.id,
            html: Arc::new(format!("<div id=\"message_{n}\">{}</div>\n", "x".repeat(9_000 + n * 50))),
        })
        .collect();
    g.render(|ctx| {
        let page = rooms::Show { ctx, show: &show };
        let html = render_page(&page).unwrap();
        assert_eq!(html, page.render().unwrap());
        assert_eq!(html.capacity(), rooms::Show::SIZE_HINT + page.extra_capacity(), "never regrown");
        html
    });
}

#[test]
fn show_closed_room() {
    show("rooms_show_closed");
}

#[test]
fn show_original_room_with_invitation() {
    show("rooms_show_original");
}

#[test]
fn show_direct_room() {
    show("rooms_show_direct");
}

#[test]
fn show_room_as_member() {
    show("rooms_show_member");
}

#[test]
fn show_room_around_message() {
    show("rooms_show_at_message");
}

#[test]
fn new_open_room() {
    let g = golden("rooms_opens_new");
    let form: OpenFormView = g.input();
    g.assert_dom(&g.render(|ctx| rooms::OpensNew { ctx, form: &form }.render().unwrap()));
}

#[test]
fn edit_open_room() {
    for name in ["rooms_opens_edit", "rooms_opens_edit_member"] {
        let g = golden(name);
        let form: OpenFormView = g.input();
        g.assert_dom(&g.render(|ctx| rooms::OpensEdit { ctx, form: &form }.render().unwrap()));
    }
}

#[test]
fn new_closed_room() {
    let g = golden("rooms_closeds_new");
    let form: ClosedFormView = g.input();
    g.assert_dom(&g.render(|ctx| rooms::ClosedsNew { ctx, form: &form }.render().unwrap()));
}

#[test]
fn edit_closed_room() {
    for name in ["rooms_closeds_edit", "rooms_closeds_edit_member"] {
        let g = golden(name);
        let form: ClosedFormView = g.input();
        g.assert_dom(&g.render(|ctx| rooms::ClosedsEdit { ctx, form: &form }.render().unwrap()));
    }
}

#[test]
fn new_direct_room() {
    let g = golden("rooms_directs_new");
    g.assert_dom(&g.render(|ctx| rooms::DirectsNew { ctx }.render().unwrap()));
}

#[test]
fn edit_direct_room() {
    let g = golden("rooms_directs_edit");
    let edit: DirectEditView = g.input();
    g.assert_dom(&g.render(|ctx| rooms::DirectsEdit { ctx, edit: &edit }.render().unwrap()));
}

#[test]
fn involvement() {
    for name in ["rooms_involvements_show", "rooms_involvements_show_direct"] {
        let g = golden(name);
        let involvement: InvolvementView = g.input();
        g.assert_content(&g.render(|ctx| rooms::InvolvementShow { ctx, involvement: &involvement }.render().unwrap()));
    }
}

#[test]
fn refresh_stream() {
    let g = golden("rooms_refreshes_show");
    let refresh: RefreshView = g.input();
    g.assert_dom(&g.render(|ctx| rooms::RefreshShow { ctx, refresh: &refresh }.render().unwrap()));
}

#[test]
fn display_names() {
    let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(rooms::room_display_name(Some("HQ"), false, &[], None), "HQ");
    assert_eq!(rooms::room_display_name(None, true, &names(&["Jason", "JZ"]), Some("David")), "Jason and JZ");
    assert_eq!(rooms::room_display_name(None, true, &[], Some("David")), "David");
}
