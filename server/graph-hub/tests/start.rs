//! §6's limits as `config::Settings` reads them, and §6's refusals on a real database.
//!
//! Two halves, and they are separate files because they need different things: [`defaults`] needs
//! no database at all and runs in the no-database floor, while [`refusals`] reads the image's own
//! PostgreSQL and sits behind `db-tests` (condition (c)).
#![cfg(feature = "db-tests")]

mod refusals;
mod settings;
mod support;