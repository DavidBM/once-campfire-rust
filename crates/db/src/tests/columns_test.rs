//! The hot models read their columns by position (`crate::sql::columns!`), which must give what
//! reading them by name gives, whatever order a table has its columns in.

use std::collections::HashMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

use rails_compat::clock::SystemClock;
use rusqlite::{Row, params};

use crate::fixtures;
use crate::{Connection, Membership, Message, Room, Timestamp, query_all, schema};

/// Where a database's tables got their column order.
#[derive(Debug, Clone, Copy)]
enum Layout {
    /// `schema.sql`'s alphabetical order, as `db:prepare` loads `schema.rb` into a new database.
    Schema,
    /// The order `20231215043540_create_initial_schema.rb` created them in, which databases Rails
    /// migrated keep.
    Migrated,
}

const LAYOUTS: [Layout; 2] = [Layout::Schema, Layout::Migrated];

#[rustfmt::skip]
const MIGRATED_COLUMNS: &[(&str, &[&str])] = &[
    ("messages", &["id", "room_id", "creator_id", "created_at", "updated_at", "client_message_id"]),
];

fn database(layout: Layout) -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    schema::prepare(&mut conn, "test", &SystemClock).unwrap();
    if let Layout::Migrated = layout {
        for (table, order) in MIGRATED_COLUMNS {
            reorder_columns(&conn, table, order);
        }
    }
    conn
}

fn database_with_fixtures(layout: Layout) -> Connection {
    let mut conn = database(layout);
    let tx = conn.transaction().unwrap();
    fixtures::load(&tx, &fixtures::reference_dir(), &fixtures::Options { now: at(0), bcrypt_cost: 4 }).unwrap();
    tx.commit().unwrap();
    conn
}

/// Recreates the empty `table` with its columns in `order`, each keeping its type, NOT NULL and
/// default.
fn reorder_columns(conn: &Connection, table: &str, order: &[&str]) {
    let mut definitions: HashMap<String, String> = query_all(conn, &format!(r#"PRAGMA table_info("{table}")"#), [], |row| {
        let name: String = row.get("name")?;
        let mut definition = format!(r#""{name}" {}"#, row.get::<_, String>("type")?);
        if row.get("pk")? {
            definition.push_str(" PRIMARY KEY AUTOINCREMENT");
        }
        if row.get("notnull")? {
            definition.push_str(" NOT NULL");
        }
        if let Some(default) = row.get::<_, Option<String>>("dflt_value")? {
            definition.push_str(&format!(" DEFAULT {default}"));
        }
        Ok((name, definition))
    })
    .unwrap()
    .into_iter()
    .collect();
    assert_eq!(definitions.len(), order.len(), "{table}: {order:?} names every column once");
    let columns: Vec<String> = order.iter().map(|column| definitions.remove(*column).unwrap()).collect();
    conn.execute_batch(&format!(r#"DROP TABLE "{table}"; CREATE TABLE "{table}" ({})"#, columns.join(", "))).unwrap();
    let names = query_all(conn, &format!(r#"SELECT "name" FROM pragma_table_info('{table}')"#), [], |row| row.get::<_, String>(0)).unwrap();
    assert_eq!(names, order);
}

/// A distinct time for each `n`, with microseconds.
fn at(n: i64) -> Timestamp {
    Timestamp::from_microsecond(1_790_000_000_000_000 + n * 1_000_123)
}

fn insert_user(conn: &Connection, id: i64) {
    conn.execute(r#"INSERT INTO "users" ("id", "name", "created_at", "updated_at") VALUES (?, 'User', ?, ?)"#, params![id, at(0), at(0)])
        .unwrap();
}

fn insert_room(conn: &Connection, room: &Room) {
    conn.execute(
        r#"INSERT INTO "rooms" ("id", "name", "type", "creator_id", "created_at", "updated_at") VALUES (?, ?, ?, ?, ?, ?)"#,
        params![room.id, room.name, room.room_type, room.creator_id, room.created_at, room.updated_at],
    )
    .unwrap();
}

fn insert_message(conn: &Connection, message: &Message) {
    conn.execute(
        r#"INSERT INTO "messages" ("id", "room_id", "creator_id", "client_message_id", "created_at", "updated_at") VALUES (?, ?, ?, ?, ?, ?)"#,
        params![message.id, message.room_id, message.creator_id, message.client_message_id, message.created_at, message.updated_at],
    )
    .unwrap();
}

/// A room with a distinct value in every column.
fn distinct_room() -> Room {
    Room {
        id: 2001,
        name: Some("Room 2002".into()),
        room_type: crate::RoomType::Closed,
        creator_id: 2003,
        created_at: at(2004),
        updated_at: at(2005),
    }
}

// Messages

/// How `Message::from_row` read a row before it read by position.
fn message_by_name(row: &Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get("id")?,
        room_id: row.get("room_id")?,
        creator_id: row.get("creator_id")?,
        client_message_id: row.get("client_message_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[test]
fn a_message_reads_each_column_into_its_own_field() {
    for layout in LAYOUTS {
        let conn = database(layout);
        let room = distinct_room();
        insert_user(&conn, room.creator_id);
        insert_room(&conn, &room);
        let message = Message {
            id: 1001,
            room_id: room.id,
            creator_id: room.creator_id,
            client_message_id: "client 1004".into(),
            created_at: at(1005),
            updated_at: at(1006),
        };
        insert_message(&conn, &message);

        assert_eq!(Message::find(&conn, message.id).unwrap(), message, "{layout:?}");
        assert_eq!(Message::last_page(&conn, room.id).unwrap(), std::slice::from_ref(&message), "{layout:?}");
    }
}

#[test]
fn messages_read_by_position_as_by_name() {
    for layout in LAYOUTS {
        let conn = database_with_fixtures(layout);
        let messages = query_all(&conn, r#"SELECT * FROM "messages" ORDER BY "id""#, [], message_by_name).unwrap();
        assert!(messages.len() > 5);
        for message in &messages {
            assert_eq!(&Message::find(&conn, message.id).unwrap(), message, "{layout:?}");
            let mut in_room = Message::for_room(&conn, message.room_id).unwrap();
            in_room.sort_by_key(|m| m.id);
            assert_eq!(in_room, messages.iter().filter(|m| m.room_id == message.room_id).cloned().collect::<Vec<_>>(), "{layout:?}");
        }
    }
}

// Timing

/// A room page's worth of rows and more: 200 messages in room 1, and user 1 in 12 rooms.
fn timing_database() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    schema::prepare(&mut conn, "test", &SystemClock).unwrap();
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
