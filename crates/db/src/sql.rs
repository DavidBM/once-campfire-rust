use rusqlite::{Connection, Params, Row};

use crate::error::Result;

/// `Connection::execute` and `Connection::query_row` through the connection's statement cache,
/// so a query is compiled once per connection rather than on every call (Active Record keeps a
/// prepared-statement cache per connection too).
pub trait CachedStatements {
    fn execute_cached(&self, sql: &str, params: impl Params) -> rusqlite::Result<usize>;

    fn query_row_cached<T>(&self, sql: &str, params: impl Params, map: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>)
    -> rusqlite::Result<T>;
}

impl CachedStatements for Connection {
    fn execute_cached(&self, sql: &str, params: impl Params) -> rusqlite::Result<usize> {
        self.prepare_cached(sql)?.execute(params)
    }

    fn query_row_cached<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        self.prepare_cached(sql)?.query_row(params, map)
    }
}

/// `?, ?, ?`
pub fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

pub fn query_all<T>(conn: &Connection, sql: &str, params: impl Params, map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>) -> Result<Vec<T>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let rows = stmt.query_map(params, map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
}

pub fn query_one<T>(
    conn: &Connection,
    sql: &str,
    params: impl Params,
    map: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Option<T>> {
    use rusqlite::OptionalExtension;
    let mut stmt = conn.prepare_cached(sql)?;
    Ok(stmt.query_row(params, map).optional()?)
}

pub fn count(conn: &Connection, sql: &str, params: impl Params) -> Result<i64> {
    Ok(conn.prepare_cached(sql)?.query_row(params, |r| r.get(0))?)
}

pub fn exists(conn: &Connection, sql: &str, params: impl Params) -> Result<bool> {
    Ok(conn.prepare_cached(sql)?.exists(params)?)
}

/// A model read by position rather than by name, from one list of `field: "column"` pairs:
///
/// - `$list!()` is the list as SQL, `"table"."column", ...`, a literal for `concat!`;
/// - `Model::COLUMNS` are the column names;
/// - `Model::from_row_at(row, offset)` reads field i from column `offset + i`, and `from_row` from 0.
///
/// Reading by name costs a scan of the statement's column names for every field of every row,
/// which was 3-5% of a room page (bench/results/columns-20260930). A query must select the list,
/// never `*`: databases Rails migrated have their columns in another order than ones it loaded
/// from `schema.rb` (see `schema.rs`).
macro_rules! columns {
    ($model:ident, $table:literal, $list:ident { $($field:ident: $column:literal),+ $(,)? }) => {
        macro_rules! $list {
            () => {
                $crate::sql::columns!(@list $table, $($column),+)
            };
        }
        // For other models' joins. The model's own module finds the macro without it.
        #[allow(unused_imports)]
        pub(crate) use $list;

        impl $model {
            pub(crate) const COLUMNS: &[&str] = &[$($column),+];

            pub(crate) fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
                Self::from_row_at(row, 0)
            }

            pub(crate) fn from_row_at(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Self> {
                // Each field's position in the list, as a discriminant.
                #[allow(non_camel_case_types)]
                enum Position {
                    $($field),+
                }
                $crate::sql::debug_assert_columns(row, offset, Self::COLUMNS);
                Ok(Self { $($field: row.get(offset + Position::$field as usize)?),+ })
            }
        }
    };
    (@list $table:literal, $first:literal $(, $column:literal)*) => {
        concat!("\"", $table, "\".\"", $first, "\"" $(, ", \"", $table, "\".\"", $column, "\"")*)
    };
}
pub(crate) use columns;

/// In debug builds, and so in every test, checks that the row has `columns` from `offset` on:
/// that the query selected the model's list, and a join reads each model at its own offset.
pub(crate) fn debug_assert_columns(row: &Row<'_>, offset: usize, columns: &[&str]) {
    let statement: &rusqlite::Statement<'_> = row.as_ref();
    for (i, column) in columns.iter().enumerate() {
        debug_assert_eq!(statement.column_name(offset + i).ok(), Some(*column), "column {} of {:?}", offset + i, statement.expanded_sql());
    }
}

/// `SecureRandom.alphanumeric(n)`
pub fn alphanumeric(n: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..n).map(|_| CHARS[rng.random_range(0..CHARS.len())] as char).collect()
}

/// `SecureRandom.base58(n)`, as `has_secure_token` uses it.
pub fn base58(n: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut rng = rand::rng();
    (0..n).map(|_| CHARS[rng.random_range(0..CHARS.len())] as char).collect()
}

/// `SecureRandom.uuid` / `Random.uuid`
pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
