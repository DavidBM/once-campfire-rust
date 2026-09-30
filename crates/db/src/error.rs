use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    /// `ActiveRecord::RecordNotFound`
    #[error("Couldn't find {0}")]
    RecordNotFound(&'static str),

    /// `ActiveRecord::RecordInvalid`
    #[error("Validation failed: {0}")]
    RecordInvalid(Errors),

    #[error("database writer is gone")]
    WriterGone,

    /// Any other failure, kept as its own type so that its source chain survives.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Like `std::io::Error::other`: wraps any error, or a message.
    pub fn other(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }

    /// `ActiveRecord::RecordNotUnique`: a unique index refused the write.
    pub fn is_record_not_unique(&self) -> bool {
        matches!(
            self,
            Self::Sqlite(rusqlite::Error::SqliteFailure(failure, _))
                if matches!(failure.extended_code, rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY)
        )
    }
}

/// `ActiveModel::Errors`: attribute/message pairs in the order they were added.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Errors(pub Vec<(&'static str, String)>);

impl Errors {
    pub fn add(&mut self, attribute: &'static str, message: impl Into<String>) {
        self.0.push((attribute, message.into()));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn on(&self, attribute: &str) -> Vec<&str> {
        self.0.iter().filter(|(a, _)| *a == attribute).map(|(_, m)| m.as_str()).collect()
    }

    /// `errors.full_messages`: "Endpoint must use HTTPS"
    pub fn full_messages(&self) -> Vec<String> {
        self.0.iter().map(|(attribute, message)| format!("{} {message}", humanize(attribute))).collect()
    }

    pub fn into_result(self) -> Result<()> {
        if self.is_empty() { Ok(()) } else { Err(Error::RecordInvalid(self)) }
    }
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full_messages().join(", "))
    }
}

fn humanize(attribute: &str) -> String {
    let words = attribute.trim_end_matches("_id").replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(crate) trait OptionalExt<T> {
    fn or_not_found(self, model: &'static str) -> Result<T>;
}

impl<T> OptionalExt<T> for Option<T> {
    fn or_not_found(self, model: &'static str) -> Result<T> {
        self.ok_or(Error::RecordNotFound(model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_errors_keep_their_type_and_message() {
        let error = Error::other(std::io::Error::other("disk full"));
        assert_eq!(error.to_string(), "disk full");
        assert!(matches!(&error, Error::Other(inner) if inner.is::<std::io::Error>()));
        assert_eq!(Error::other(format!("no table {}", "rooms")).to_string(), "no table rooms");
    }

    #[test]
    fn unique_violations_are_record_not_unique() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE)").unwrap();
        conn.execute("INSERT INTO users VALUES (1, 'a@example.com')", []).unwrap();
        let insert = |sql: &str| Error::from(conn.execute(sql, []).unwrap_err());

        assert!(insert("INSERT INTO users VALUES (2, 'a@example.com')").is_record_not_unique());
        assert!(insert("INSERT INTO users VALUES (1, 'b@example.com')").is_record_not_unique());
        assert!(!insert("INSERT INTO users VALUES (3, NULL)").is_record_not_unique());
        assert!(!Error::RecordNotFound("User").is_record_not_unique());
    }
}
