//! Task 3's routes do not exist yet, so this file names the plan's matrix in the shape Task 3 can
//! prove: the authorization decision is a function of the header and the path, and nothing else.
//!
//! Four children, one per decision, none of which needs a route: [`credential`] parses the header,
//! [`refusals`] is the 403-before-404 matrix, [`paths`] the refusals about the path itself, and
//! [`grants`] the grants file as a model and as a file. The router-level matrix (`hub-authz` over
//! the real routes) is Task 5's, where the routes it exercises are written.
#![cfg(feature = "db-tests")]

#[path = "authz/credential.rs"]
mod credential;
#[path = "authz/grants.rs"]
mod grants;
#[path = "authz/paths.rs"]
mod paths;
#[path = "authz/refusals.rs"]
mod refusals;
mod support;
