//! `reference/app/models/session.rb`

use jiff::SignedDuration;
use rusqlite::{Connection, params};

use crate::database::Tx;
use crate::error::{OptionalExt, Result};
use crate::sql::{self, CachedStatements, columns, query_all, query_one};
use crate::time::Timestamp;

/// `Session::ACTIVITY_REFRESH_RATE`
pub const ACTIVITY_REFRESH_RATE: SignedDuration = SignedDuration::from_hours(1);

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_active_at: Timestamp,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

columns! {
    Session, "sessions", session_columns {
        id: "id",
        user_id: "user_id",
        token: "token",
        ip_address: "ip_address",
        user_agent: "user_agent",
        last_active_at: "last_active_at",
        created_at: "created_at",
        updated_at: "updated_at",
    }
}

impl Session {
    pub fn find(conn: &Connection, id: i64) -> Result<Self> {
        query_one(
            conn,
            concat!("SELECT ", session_columns!(), r#" FROM "sessions" WHERE "sessions"."id" = ? LIMIT 1"#),
            [id],
            Self::from_row,
        )?
        .or_not_found("Session")
    }

    /// `Session.find_by(token:)`
    pub fn find_by_token(conn: &Connection, token: &str) -> Result<Option<Self>> {
        query_one(
            conn,
            concat!("SELECT ", session_columns!(), r#" FROM "sessions" WHERE "sessions"."token" = ? LIMIT 1"#),
            [token],
            Self::from_row,
        )
    }

    pub fn for_user(conn: &Connection, user_id: i64) -> Result<Vec<Self>> {
        query_all(
            conn,
            concat!("SELECT ", session_columns!(), r#" FROM "sessions" WHERE "sessions"."user_id" = ?"#),
            [user_id],
            Self::from_row,
        )
    }

    pub fn count_for_user(conn: &Connection, user_id: i64) -> Result<i64> {
        sql::count(conn, r#"SELECT COUNT(*) FROM "sessions" WHERE "sessions"."user_id" = ?"#, [user_id])
    }

    /// `user.sessions.start!(user_agent:, ip_address:)`: a new 24-character base58
    /// `has_secure_token`, and `last_active_at ||= Time.now` in `before_create`.
    pub fn start(tx: &mut Tx<'_>, user_id: i64, user_agent: Option<&str>, ip_address: Option<&str>) -> Result<Self> {
        let now = tx.now();
        let last_active_at = tx.now();
        let token = sql::base58(24);
        let id: i64 = tx.conn().query_row_cached(
            r#"INSERT INTO "sessions" ("created_at", "ip_address", "last_active_at", "token", "updated_at", "user_agent", "user_id") VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING "id""#,
            params![now, ip_address, last_active_at, token, now, user_agent, user_id],
            |r| r.get(0),
        )?;
        Ok(Self {
            id,
            user_id,
            token,
            ip_address: ip_address.map(Into::into),
            user_agent: user_agent.map(Into::into),
            last_active_at,
            created_at: now,
            updated_at: now,
        })
    }

    /// Whether [`Self::resume`] would refresh the session at `now`: its activity is over an hour old.
    pub fn needs_resume(&self, now: Timestamp) -> bool {
        self.last_active_at < now.ago(ACTIVITY_REFRESH_RATE)
    }

    /// `resume`: refreshes activity, user agent and IP at most once an hour.
    pub fn resume(&mut self, tx: &mut Tx<'_>, user_agent: Option<&str>, ip_address: Option<&str>) -> Result<()> {
        let now = tx.now();
        if !self.needs_resume(now) {
            return Ok(());
        }
        self.user_agent = user_agent.map(Into::into);
        self.ip_address = ip_address.map(Into::into);
        self.last_active_at = now;
        self.updated_at = tx.now();
        tx.conn().execute_cached(
            r#"UPDATE "sessions" SET "ip_address" = ?, "last_active_at" = ?, "updated_at" = ?, "user_agent" = ? WHERE "sessions"."id" = ?"#,
            params![self.ip_address, self.last_active_at, self.updated_at, self.user_agent, self.id],
        )?;
        Ok(())
    }

    /// `destroy!`
    pub fn destroy(&self, tx: &mut Tx<'_>) -> Result<()> {
        tx.conn().execute_cached(r#"DELETE FROM "sessions" WHERE "sessions"."id" = ?"#, [self.id])?;
        Ok(())
    }
}
