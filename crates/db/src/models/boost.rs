//! `reference/app/models/boost.rb`

use rusqlite::{Connection, params};

use crate::database::Tx;
use crate::error::{OptionalExt, Result};
use crate::models::Message;
use crate::sql::{CachedStatements, columns, query_all, query_one};
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq)]
pub struct Boost {
    pub id: i64,
    pub message_id: i64,
    pub booster_id: i64,
    pub content: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

columns! {
    Boost, "boosts", boost_columns {
        id: "id",
        message_id: "message_id",
        booster_id: "booster_id",
        content: "content",
        created_at: "created_at",
        updated_at: "updated_at",
    }
}

impl Boost {
    pub fn find(conn: &Connection, id: i64) -> Result<Self> {
        query_one(conn, concat!("SELECT ", boost_columns!(), r#" FROM "boosts" WHERE "boosts"."id" = ? LIMIT 1"#), [id], Self::from_row)?
            .or_not_found("Boost")
    }

    pub fn for_message(conn: &Connection, message_id: i64) -> Result<Vec<Self>> {
        query_all(
            conn,
            concat!("SELECT ", boost_columns!(), r#" FROM "boosts" WHERE "boosts"."message_id" = ?"#),
            [message_id],
            Self::from_row,
        )
    }

    /// `message.boosts.ordered`
    pub fn for_message_ordered(conn: &Connection, message_id: i64) -> Result<Vec<Self>> {
        query_all(
            conn,
            concat!("SELECT ", boost_columns!(), r#" FROM "boosts" WHERE "boosts"."message_id" = ? ORDER BY "boosts"."created_at" ASC"#),
            [message_id],
            Self::from_row,
        )
    }

    /// `message.boosts.find_by!(id:, booster:)`
    pub fn find_by_message_and_booster(conn: &Connection, message_id: i64, id: i64, booster_id: i64) -> Result<Self> {
        query_one(
            conn,
            concat!(
                "SELECT ",
                boost_columns!(),
                r#" FROM "boosts" WHERE "boosts"."message_id" = ? AND "boosts"."id" = ? AND "boosts"."booster_id" = ? LIMIT 1"#
            ),
            [message_id, id, booster_id],
            Self::from_row,
        )?
        .or_not_found("Boost")
    }

    /// `message.boosts.create!(content:, booster:)`: touches the message (and so the room).
    pub fn create(tx: &mut Tx<'_>, message_id: i64, booster_id: i64, content: &str) -> Result<Self> {
        let now = tx.now();
        let id: i64 = tx.conn().query_row_cached(
            r#"INSERT INTO "boosts" ("booster_id", "content", "created_at", "message_id", "updated_at") VALUES (?, ?, ?, ?, ?) RETURNING "id""#,
            params![booster_id, content, now, message_id, now],
            |r| r.get(0),
        )?;
        Message::find(tx.conn(), message_id)?.touch(tx)?;
        Ok(Self { id, message_id, booster_id, content: content.into(), created_at: now, updated_at: now })
    }

    /// `destroy!`: touches the message (and so the room).
    pub fn destroy(&self, tx: &mut Tx<'_>) -> Result<()> {
        self.delete_row(tx)?;
        Message::find(tx.conn(), self.message_id)?.touch(tx)
    }

    /// The delete alone, for a message being destroyed (its touch is moot).
    pub(crate) fn delete_row(&self, tx: &Tx<'_>) -> Result<()> {
        tx.conn().execute_cached(r#"DELETE FROM "boosts" WHERE "boosts"."id" = ?"#, [self.id])?;
        Ok(())
    }
}
