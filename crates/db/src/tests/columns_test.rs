//! Reading the hot models' rows.

use std::hint::black_box;
use std::time::{Duration, Instant};

use rails_compat::clock::SystemClock;
use rusqlite::params;

use crate::{Connection, Membership, Message, Room, Timestamp, schema};

/// A room page's worth of rows and more: 200 messages in room 1, and user 1 in 12 rooms.
fn timing_database() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    schema::prepare(&mut conn, "test", &SystemClock).unwrap();
    let at = |n: i64| Timestamp::from_microsecond(1_790_000_000_000_000 + n * 1_000_123);
    conn.execute(r#"INSERT INTO "users" ("id", "name", "created_at", "updated_at") VALUES (1, 'User', ?, ?)"#, params![at(0), at(0)])
        .unwrap();
    for room in 1..=12 {
        conn.execute(
            r#"INSERT INTO "rooms" ("id", "name", "type", "creator_id", "created_at", "updated_at") VALUES (?, ?, 'Rooms::Open', 1, ?, ?)"#,
            params![room, format!("Room {room}"), at(room), at(room + 1)],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO "memberships" ("room_id", "user_id", "involvement", "unread_at", "connected_at", "connections", "created_at", "updated_at") VALUES (?, 1, 'mentions', ?, NULL, 0, ?, ?)"#,
            params![room, (room % 2 == 0).then(|| at(room)), at(room), at(room + 1)],
        )
        .unwrap();
    }
    for n in 1..=200 {
        conn.execute(
            r#"INSERT INTO "messages" ("id", "room_id", "creator_id", "client_message_id", "created_at", "updated_at") VALUES (?, 1, 1, ?, ?, ?)"#,
            params![n, format!("5f0c2b4e-1d7a-4c39-9a57-{n:012}"), at(n), at(n + 1)],
        )
        .unwrap();
    }
    conn
}

/// The median over 7 runs of the mean time per call.
fn time_per_call(iterations: u32, mut call: impl FnMut()) -> Duration {
    let mut runs: Vec<Duration> = (0..7)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..iterations {
                call();
            }
            start.elapsed() / iterations
        })
        .collect();
    runs.sort();
    runs[runs.len() / 2]
}

/// `cargo test --release -p campfire_db --lib -- --ignored --nocapture row_reading_timing`
#[test]
#[ignore = "a timing, run by hand in a release build"]
fn row_reading_timing() {
    let conn = timing_database();
    let before = Message::find(&conn, 161).unwrap();
    let timings = [
        ("Message::last_page (40 rows)", time_per_call(20_000, || drop(black_box(Message::last_page(&conn, 1).unwrap())))),
        ("Message::page_before (40 rows)", time_per_call(20_000, || drop(black_box(Message::page_before(&conn, 1, &before).unwrap())))),
        (
            "Membership::visible_with_ordered_room (12 rows)",
            time_per_call(20_000, || drop(black_box(Membership::visible_with_ordered_room(&conn, 1).unwrap()))),
        ),
        ("Message::find (1 row)", time_per_call(200_000, || drop(black_box(Message::find(&conn, 100).unwrap())))),
        ("Room::find (1 row)", time_per_call(200_000, || drop(black_box(Room::find(&conn, 5).unwrap())))),
    ];
    for (name, per_call) in timings {
        println!("{name}: {per_call:?}");
    }
}
