//! `reference/app/models/membership.rb` and `membership/connectable.rb`.

use jiff::SignedDuration;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, Row, params};

use crate::database::Tx;
use crate::error::{OptionalExt, Result};
use crate::models::room::room_columns;
use crate::models::{Room, User};
use crate::sql::{self, CachedStatements, columns, query_all, query_one};
use crate::time::Timestamp;

/// `enum :involvement, %w[ invisible nothing mentions everything ].index_by(&:itself)`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Involvement {
    Invisible,
    Nothing,
    Mentions,
    Everything,
}

impl Involvement {
    pub fn name(self) -> &'static str {
        match self {
            Involvement::Invisible => "invisible",
            Involvement::Nothing => "nothing",
            Involvement::Mentions => "mentions",
            Involvement::Everything => "everything",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "invisible" => Some(Involvement::Invisible),
            "nothing" => Some(Involvement::Nothing),
            "mentions" => Some(Involvement::Mentions),
            "everything" => Some(Involvement::Everything),
            _ => None,
        }
    }
}

impl ToSql for Involvement {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.name()))
    }
}

impl FromSql for Involvement {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let name = value.as_str()?;
        Involvement::from_name(name).ok_or_else(|| FromSqlError::Other(format!("unknown involvement {name:?}").into()))
    }
}

/// `Membership::Connectable::CONNECTION_TTL`
pub const CONNECTION_TTL: SignedDuration = SignedDuration::from_secs(60);

#[derive(Debug, Clone, PartialEq)]
pub struct Membership {
    pub id: i64,
    pub room_id: i64,
    pub user_id: i64,
    /// Nullable in the schema (default "mentions").
    pub involvement: Option<Involvement>,
    pub unread_at: Option<Timestamp>,
    pub connected_at: Option<Timestamp>,
    pub connections: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

columns! {
    Membership, "memberships", membership_columns {
        id: "id",
        room_id: "room_id",
        user_id: "user_id",
        involvement: "involvement",
        unread_at: "unread_at",
        connected_at: "connected_at",
        connections: "connections",
        created_at: "created_at",
        updated_at: "updated_at",
    }
}

impl Membership {
    pub fn find(conn: &Connection, id: i64) -> Result<Self> {
        query_one(
            conn,
            concat!("SELECT ", membership_columns!(), r#" FROM "memberships" WHERE "memberships"."id" = ? LIMIT 1"#),
            [id],
            Self::from_row,
        )?
        .or_not_found("Membership")
    }

    pub fn count(conn: &Connection) -> Result<i64> {
        sql::count(conn, r#"SELECT COUNT(*) FROM "memberships""#, [])
    }

    pub fn for_user(conn: &Connection, user_id: i64) -> Result<Vec<Self>> {
        query_all(
            conn,
            concat!("SELECT ", membership_columns!(), r#" FROM "memberships" WHERE "memberships"."user_id" = ?"#),
            [user_id],
            Self::from_row,
        )
    }

    pub fn for_room(conn: &Connection, room_id: i64) -> Result<Vec<Self>> {
        query_all(
            conn,
            concat!("SELECT ", membership_columns!(), r#" FROM "memberships" WHERE "memberships"."room_id" = ?"#),
            [room_id],
            Self::from_row,
        )
    }

    /// `room.memberships.find_by(user:)` / `user.memberships.find_by(room_id:)`
    pub fn find_by_room_and_user(conn: &Connection, room_id: i64, user_id: i64) -> Result<Option<Self>> {
        query_one(
            conn,
            concat!(
                "SELECT ",
                membership_columns!(),
                r#" FROM "memberships" WHERE "memberships"."room_id" = ? AND "memberships"."user_id" = ? LIMIT 1"#
            ),
            [room_id, user_id],
            Self::from_row,
        )
    }

    /// `user.memberships.visible.with_ordered_room`, each with its room.
    pub fn visible_with_ordered_room(conn: &Connection, user_id: i64) -> Result<Vec<(Self, Room)>> {
        query_all(
            conn,
            concat!(
                "SELECT ",
                membership_columns!(),
                ", ",
                room_columns!(),
                r#" FROM "memberships" INNER JOIN "rooms" ON "rooms"."id" = "memberships"."room_id" WHERE "memberships"."user_id" = ? AND "memberships"."involvement" != 'invisible' ORDER BY LOWER(rooms.name)"#
            ),
            [user_id],
            with_room,
        )
    }

    /// `user.memberships.with_ordered_room` (invisible included).
    pub fn with_ordered_room(conn: &Connection, user_id: i64) -> Result<Vec<(Self, Room)>> {
        query_all(
            conn,
            concat!(
                "SELECT ",
                membership_columns!(),
                ", ",
                room_columns!(),
                r#" FROM "memberships" INNER JOIN "rooms" ON "rooms"."id" = "memberships"."room_id" WHERE "memberships"."user_id" = ? ORDER BY LOWER(rooms.name)"#
            ),
            [user_id],
            with_room,
        )
    }

    /// `user.memberships.without_direct_rooms.count`
    pub fn count_without_direct_rooms(conn: &Connection, user_id: i64) -> Result<i64> {
        sql::count(
            conn,
            r#"SELECT COUNT(*) FROM "memberships" INNER JOIN "rooms" "room" ON "room"."id" = "memberships"."room_id" WHERE "memberships"."user_id" = ? AND "room"."type" != 'Rooms::Direct'"#,
            [user_id],
        )
    }

    /// `user.memberships.unread.count`: the push badge.
    pub fn unread_count(conn: &Connection, user_id: i64) -> Result<i64> {
        sql::count(
            conn,
            r#"SELECT COUNT(*) FROM "memberships" WHERE "memberships"."user_id" = ? AND "memberships"."unread_at" IS NOT NULL"#,
            [user_id],
        )
    }

    /// `Membership.connected.exists?(id)`
    pub fn connected_exists(conn: &Connection, id: i64, now: Timestamp) -> Result<bool> {
        sql::exists(
            conn,
            r#"SELECT 1 FROM "memberships" WHERE "memberships"."connected_at" >= ? AND "memberships"."id" = ? LIMIT 1"#,
            params![Self::connection_cutoff(now), id],
        )
    }

    /// `Membership.disconnected.exists?(id)`
    pub fn disconnected_exists(conn: &Connection, id: i64, now: Timestamp) -> Result<bool> {
        sql::exists(
            conn,
            r#"SELECT 1 FROM "memberships" WHERE ("memberships"."connected_at" IS NULL OR "memberships"."connected_at" < ?) AND "memberships"."id" = ? LIMIT 1"#,
            params![Self::connection_cutoff(now), id],
        )
    }

    /// `CONNECTION_TTL.ago`
    pub fn connection_cutoff(now: Timestamp) -> Timestamp {
        now.ago(CONNECTION_TTL)
    }

    pub fn room(&self, conn: &Connection) -> Result<Room> {
        Room::find(conn, self.room_id)
    }

    pub fn user(&self, conn: &Connection) -> Result<User> {
        User::find(conn, self.user_id)
    }

    // Involvement and read state

    pub fn involved_in(&self, involvement: Involvement) -> bool {
        self.involvement == Some(involvement)
    }

    /// `update!(involvement:)`
    pub fn update_involvement(&mut self, tx: &mut Tx<'_>, involvement: impl Into<Option<Involvement>>) -> Result<()> {
        let involvement = involvement.into();
        if self.involvement == involvement {
            return Ok(());
        }
        let now = tx.now();
        tx.conn().execute_cached(
            r#"UPDATE "memberships" SET "involvement" = ?, "updated_at" = ? WHERE "memberships"."id" = ?"#,
            params![involvement, now, self.id],
        )?;
        self.involvement = involvement;
        self.updated_at = now;
        Ok(())
    }

    /// `read`: `update!(unread_at: nil)`
    pub fn read(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        if self.unread_at.is_none() {
            return Ok(());
        }
        let now = tx.now();
        tx.conn().execute_cached(
            r#"UPDATE "memberships" SET "unread_at" = ?, "updated_at" = ? WHERE "memberships"."id" = ?"#,
            params![None::<Timestamp>, now, self.id],
        )?;
        self.unread_at = None;
        self.updated_at = now;
        Ok(())
    }

    pub fn unread(&self) -> bool {
        self.unread_at.is_some()
    }

    /// `destroy`: the user's sockets reconnect after commit, so their subscriptions to this
    /// room are dropped.
    pub fn destroy(&self, tx: &mut Tx<'_>) -> Result<()> {
        tx.conn().execute_cached(r#"DELETE FROM "memberships" WHERE "memberships"."id" = ?"#, [self.id])?;
        let user_id = self.user_id;
        tx.after_commit(move |tx| {
            User::find(tx.conn(), user_id)?.reset_remote_connections(tx);
            Ok(())
        });
        Ok(())
    }

    // Membership::Connectable

    /// `Membership.disconnect_all`
    pub fn disconnect_all(tx: &mut Tx<'_>) -> Result<usize> {
        let now = tx.now();
        Ok(tx.conn().execute_cached(
            r#"UPDATE "memberships" SET "connected_at" = ?, "connections" = ?, "updated_at" = ? WHERE "memberships"."connected_at" >= ?"#,
            params![None::<Timestamp>, 0, now, Self::connection_cutoff(now)],
        )?)
    }

    /// `Membership.connect(membership, connections)`: no `updated_at`.
    pub fn connect(tx: &mut Tx<'_>, id: i64, connections: i64) -> Result<()> {
        tx.conn().execute_cached(
            r#"UPDATE "memberships" SET "connections" = ?, "connected_at" = ?, "unread_at" = ? WHERE "memberships"."id" = ?"#,
            params![connections, tx.now(), None::<Timestamp>, id],
        )?;
        Ok(())
    }

    /// `connected?`
    pub fn is_connected(&self, now: Timestamp) -> bool {
        self.connected_at.is_some_and(|at| at >= Self::connection_cutoff(now))
    }

    /// `present`
    pub fn present(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        let connections = if self.is_connected(tx.now()) { self.connections + 1 } else { 1 };
        Self::connect(tx, self.id, connections)
    }

    /// `connected`
    pub fn connected(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        self.increment_connections(tx)?;
        self.touch_connected_at(tx)
    }

    /// `disconnected`
    pub fn disconnected(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        self.decrement_connections(tx)?;
        if self.connections < 1 && self.connected_at.is_some() {
            let now = tx.now();
            tx.conn().execute_cached(
                r#"UPDATE "memberships" SET "connected_at" = ?, "updated_at" = ? WHERE "memberships"."id" = ?"#,
                params![None::<Timestamp>, now, self.id],
            )?;
            self.connected_at = None;
            self.updated_at = now;
        }
        Ok(())
    }

    /// `refresh_connection`
    pub fn refresh_connection(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        if !self.is_connected(tx.now()) {
            self.increment_connections(tx)?;
        }
        self.touch_connected_at(tx)
    }

    fn increment_connections(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        if self.is_connected(tx.now()) { self.update_counter(tx, 1) } else { self.update_connections(tx, 1) }
    }

    fn decrement_connections(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        if self.is_connected(tx.now()) { self.update_counter(tx, -1) } else { self.update_connections(tx, 0) }
    }

    /// `increment!(:connections, touch: true)` / `decrement!`
    fn update_counter(&mut self, tx: &mut Tx<'_>, by: i64) -> Result<()> {
        let now = tx.now();
        let sql = if by >= 0 {
            r#"UPDATE "memberships" SET "connections" = COALESCE("memberships"."connections", 0) + ?, "updated_at" = ? WHERE "memberships"."id" = ?"#
        } else {
            r#"UPDATE "memberships" SET "connections" = COALESCE("memberships"."connections", 0) - ?, "updated_at" = ? WHERE "memberships"."id" = ?"#
        };
        tx.conn().execute_cached(sql, params![by.abs(), now, self.id])?;
        self.connections += by;
        self.updated_at = now;
        Ok(())
    }

    /// `update!(connections:)`, a no-op when unchanged.
    fn update_connections(&mut self, tx: &mut Tx<'_>, connections: i64) -> Result<()> {
        if self.connections == connections {
            return Ok(());
        }
        let now = tx.now();
        tx.conn().execute_cached(
            r#"UPDATE "memberships" SET "connections" = ?, "updated_at" = ? WHERE "memberships"."id" = ?"#,
            params![connections, now, self.id],
        )?;
        self.connections = connections;
        self.updated_at = now;
        Ok(())
    }

    /// `touch :connected_at`
    fn touch_connected_at(&mut self, tx: &mut Tx<'_>) -> Result<()> {
        let now = tx.now();
        tx.conn().execute_cached(
            r#"UPDATE "memberships" SET "updated_at" = ?, "connected_at" = ? WHERE "memberships"."id" = ?"#,
            params![now, now, self.id],
        )?;
        self.connected_at = Some(now);
        self.updated_at = now;
        Ok(())
    }

    pub fn reload(&mut self, conn: &Connection) -> Result<()> {
        *self = Self::find(conn, self.id)?;
        Ok(())
    }
}

/// A membership and its room, from `membership_columns!()` followed by `room_columns!()`.
fn with_room(row: &Row<'_>) -> rusqlite::Result<(Membership, Room)> {
    Ok((Membership::from_row(row)?, Room::from_row_at(row, Membership::COLUMNS.len())?))
}
