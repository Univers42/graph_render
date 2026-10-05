//! Every error the store can answer, and the one mapping from the driver's.
//!
//! `StoreError` never carries an HTTP status: the hub maps `code()` to a status in slice 3. The
//! store's job is to say *what kind* of thing went wrong, not what the wire should say.

use std::fmt;

/// A database error, kept as its SQLSTATE and its message.
///
/// The driver already gives `code()`, so this adds nothing the store could not answer — except
/// the one case the store raises itself, [`DbError::migration_hash`], which has no SQLSTATE at
/// all because it never reached the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbError {
    code: String,
    message: String,
}

impl DbError {
    /// The five-character SQLSTATE, or the synthetic `XX000` for an error the store raised.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The driver's message, verbatim.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// A store-side refusal with no server behind it, named by `code` and explained by `message`.
    ///
    /// §6's start checks and the migration-hash guard are both this: neither reaches PostgreSQL,
    /// so neither has a real SQLSTATE.
    pub fn store(code: &'static str, message: String) -> DbError {
        DbError {
            code: code.to_string(),
            message,
        }
    }

    /// A migration file's bytes changed after it was applied, so the schema is no longer the one
    /// the code was written against.
    ///
    /// This never reaches the server, so it carries the store's own synthetic SQLSTATE.
    pub fn migration_hash(name: &str) -> DbError {
        DbError::store("XX000", format!("migration hash mismatch: {name}"))
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for DbError {}

/// Everything `Store` can return.
///
/// `Gone` and `Eof` are values, not failures: a cursor below what is kept and an exhausted
/// document are both answers the caller asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A request the store accepted but the contract rejects: shape, grammar, a cap, a conflict.
    Hub(graph_contract::hub::HubError),
    /// Anything the driver reported, including a migration hash mismatch.
    Db(DbError),
    /// `GM_HUB_PG_URL` is unset or the start checks refused the database.
    NoDatabase,
    /// A serialization failure or deadlock. `retried` says whether the one retry already ran:
    /// `true` is the hub's 503 with `Retry-After: 1`, `false` is a `40001` that was retried once
    /// and came back.
    Serialization {
        /// Whether the single permitted retry has already been spent.
        retried: bool,
    },
    /// The writer is saturated; `retry_after` is the seconds the hub should advertise.
    Busy {
        /// Seconds to put in `Retry-After`.
        retry_after: u64,
    },
    /// The cursor is below what is kept, or from another epoch: 410 on `/changes`, `event: resync`
    /// on the stream. A value, because the caller resyncs from `/graph` rather than failing.
    Gone,
    /// The document stream is exhausted.
    Eof,
}

impl StoreError {
    /// The store's own stable name for this error, which the hub maps to a status.
    pub fn code(&self) -> &'static str {
        match self {
            StoreError::Hub(_) => "hub",
            StoreError::Db(_) => "db",
            StoreError::NoDatabase => "no-database",
            StoreError::Serialization { .. } => "serialization",
            StoreError::Busy { .. } => "busy",
            StoreError::Gone => "gone",
            StoreError::Eof => "eof",
        }
    }

    /// The seconds the hub should put in `Retry-After`, when this error names one.
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            StoreError::Busy { retry_after } => Some(*retry_after),
            _ => None,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Hub(e) => write!(f, "{e}"),
            StoreError::Db(e) => write!(f, "{e}"),
            StoreError::NoDatabase => write!(f, "no database"),
            StoreError::Serialization { retried } => {
                write!(f, "serialization failure, retried: {retried}")
            }
            StoreError::Busy { retry_after } => write!(f, "busy, retry after {retry_after}s"),
            StoreError::Gone => write!(f, "cursor is gone"),
            StoreError::Eof => write!(f, "end of document"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<graph_contract::hub::HubError> for StoreError {
    fn from(e: graph_contract::hub::HubError) -> Self {
        StoreError::Hub(e)
    }
}

/// Map a driver error by its SQLSTATE.
///
/// `40001` and `40P01` are the two the one permitted retry exists for; a unique violation on
/// the idempotency key (`23505`) is a conflict the hub answers, not a database fault. Everything
/// else is [`StoreError::Db`] unchanged, because a message the store does not recognise is
/// better surfaced than swallowed.
pub fn from_db(err: tokio_postgres::Error) -> StoreError {
    let code = err.code().map(|c| c.code()).unwrap_or("XX000").to_string();
    match code.as_str() {
        "40001" | "40P01" => StoreError::Serialization { retried: false },
        "23505" => StoreError::Hub(graph_contract::hub::HubError::Conflict {
            what: "unique violation".to_string(),
        }),
        _ => {
            // `Display` for a driver error is the bare string "db error"; the detail is what
            // names the statement that failed, so it is the message worth keeping.
            let message = match err.as_db_error() {
                Some(db) => match db.detail() {
                    Some(detail) => format!("{db}: {detail}"),
                    None => db.to_string(),
                },
                None => err.to_string(),
            };
            StoreError::Db(DbError { code, message })
        }
    }
}

impl From<tokio_postgres::Error> for StoreError {
    fn from(err: tokio_postgres::Error) -> Self {
        from_db(err)
    }
}
