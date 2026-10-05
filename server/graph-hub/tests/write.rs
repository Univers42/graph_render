//! The write half of §5.2's table: the three routes that change something, one test per status the
//! spec gives them.
//!
//! One file per route family under `write/`: [`workspaces`] for `PUT /v1/workspaces/{ws}`,
//! [`manifests`] for the manifest PUT and its refusals, [`batches`] for `POST .../batches` and the
//! refusals it owns, [`idem`] for the idempotency-key cases, [`records`] for the read-after-write
//! routes, and [`permits`] for the WRITERS gate. [`common`] holds the one helper two of them share.
//!
//! Every case builds its bodies through `support::fixtures`, which goes through graph-contract's own
//! writers, so a refusal here is the hub's or the store's and never a hand-typed body's.
#![cfg(feature = "db-tests")]

#[path = "write/batches.rs"]
mod batches;
#[path = "write/common.rs"]
mod common;
#[path = "write/idem.rs"]
mod idem;
#[path = "write/manifests.rs"]
mod manifests;
#[path = "write/permits.rs"]
mod permits;
#[path = "write/records.rs"]
mod records;
#[path = "support/mod.rs"]
mod support;
#[path = "write/workspaces.rs"]
mod workspaces;
