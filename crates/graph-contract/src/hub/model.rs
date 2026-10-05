//! The model: what a workspace's hub actually holds, and the document it is served as.
//!
//! Two `BTreeMap`s and nothing else — plugins by id, records by `(qualified collection,
//! record id)`. That is the whole design, and it is the design: byte order falls out of
//! the key order, so `to_json` is a walk with no sorting in it and no possibility of two
//! walks disagreeing. A `HashMap` here would be faster and would make every output
//! depend on the hasher (D5, D7).
//!
//! ## What `apply` does, in order
//!
//! 1. Every upsert and delete is turned into a stored record (or a removal) **and staged**;
//! 2. the `rev` each staged record gets is `previous + 1`, or 1 for a record that is not
//!    there — including one that was deleted, because a re-created record has never
//!    existed in the store's terms and a client that saw rev 9 of a deleted record must
//!    not be told its new record is rev 10;
//! 3. an identical upsert stages **nothing**: the same text at the same collection is not a
//!    change, and reporting one would put a change in the stream that a reader replaying
//!    it would apply to no effect;
//! 4. only then is anything committed. A batch is atomic, so one bad record leaves the
//!    model exactly as it was.
//!
//! ## Document order, and the one thing that is not a byte
//!
//! head, collections by qualified id, records by `(qualified collection, id)`, tail. The
//! record keys are compared **as a tuple**, not as concatenated text: `("a-b", "1")`
//! before `("a", "x")` and `("a.x", "1")` before `("a", "x")` are three different orders,
//! and only the tuple order is the one the spec states. A record with nothing pruned is
//! written from its stored `text`, so a read costs nothing to reproduce byte for byte.

mod document;

use super::breaks;
use super::manifest::{Growth, Manifest, growth};
use super::{HubError, Limits, MAX_PLUGINS, check_workspace_id, qualify};
use crate::ingest::{Record, record_piece};
use std::collections::BTreeMap;

mod feed;

/// One stored record: the `rev` the store assigned, the exact bytes it was written as, and
/// the parsed record itself.
///
/// `text` is not a cache: it is what makes "a record with nothing to prune is copied
/// verbatim" true, so a document read twice is byte-identical rather than merely equal
/// after a round trip through the parser.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    /// The store's revision for this record, 1 when it was created.
    pub rev: u64,
    /// The record's canonical text, exactly as [`crate::ingest::record_piece`] wrote it.
    pub text: String,
    /// The record, parsed.
    pub record: Record,
}

/// What one applied batch changed: the records it stored with their new `rev`s, and the
/// `(collection, id, rev)` of each removal.
///
/// An identical upsert appears in neither list — see the module doc.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// Every record this batch stored, with the `rev` it was given.
    pub upserted: Vec<(Record, u64)>,
    /// Every record this batch removed.
    pub deleted: Vec<(String, String, u64)>,
}

/// A workspace's whole hub: its plugins' declarations and every record they have stored.
#[derive(Debug, Clone)]
pub struct Model {
    workspace: String,
    plugins: BTreeMap<String, Manifest>,
    records: BTreeMap<(String, String), Stored>,
}

impl Model {
    /// An empty model for `ws`, or the refusal: the workspace id is the ingest `source`,
    /// so it goes through the same slug grammar or a node id would not round-trip.
    pub fn new(ws: &str) -> Result<Model, HubError> {
        check_workspace_id(ws)?;
        Ok(Model {
            workspace: ws.to_owned(),
            plugins: BTreeMap::new(),
            records: BTreeMap::new(),
        })
    }

    /// The workspace id: the `source` every record's node id begins with.
    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    /// Registers `plugin`'s manifest, or the refusal. Growth is the same rule the
    /// manifest reader's own test states: declarations may only be added, and a change at
    /// the same version is a conflict.
    ///
    /// The cap is on *registered plugins*, checked before the manifest is stored so a
    /// refused registration leaves nothing behind — and, more importantly, so the refusal
    /// cannot depend on whether the caller's manifest happened to be valid.
    pub fn register(&mut self, plugin: &str, manifest: Manifest) -> Result<Growth, HubError> {
        if !self.plugins.contains_key(plugin) && self.plugins.len() as u64 >= MAX_PLUGINS {
            return Err(HubError::TooLarge {
                what: "plugins",
                limit: MAX_PLUGINS,
            });
        }
        let outcome = match self.plugins.get(plugin) {
            Some(old) => growth(old, &manifest)?,
            None => Growth::Grown,
        };
        self.plugins.insert(plugin.to_owned(), manifest);
        Ok(outcome)
    }

    /// Applies `batch` for `plugin`, or the refusal — with **nothing changed** if it is
    /// refused. The staging is what makes that true: every check runs against the staged
    /// records and the commit happens once, at the end.
    pub fn apply(
        &mut self,
        plugin: &str,
        batch: &super::batch::Batch,
        limits: &Limits,
    ) -> Result<Applied, HubError> {
        batch.check(plugin, self.manifest_of(plugin)?, limits)?;
        let staged = feed::stage(self, plugin, batch)?;
        let applied = staged.applied.clone();
        self.records = staged.into_records();
        Ok(applied)
    }

    /// Every stored record by key. The one place the private map escapes, and it goes to
    /// [`feed::stage`] alone — which clones it, so the staging copy is a snapshot and not a
    /// second view that a commit could leave out of step.
    pub(super) fn records_map(&self) -> &BTreeMap<(String, String), Stored> {
        &self.records
    }

    /// Whether a dangling reference is kept rather than pruned — the `keep-dangling`
    /// negative control. See [`crate::hub::prune`].
    pub fn keeps_dangling(&self) -> bool {
        breaks::on("keep-dangling")
    }

    /// The `rev` a record created now gets: 1. A record that was deleted and is being
    /// created again also gets 1, and that is the point (see the module doc).
    pub fn first_rev() -> u64 {
        1
    }
}

impl Model {
    /// The stored value for `record` at `rev`: the three fields together, so a caller
    /// building a `Model` by hand gets the same `text` this module would have written.
    pub fn stored_of(record: &Record, rev: u64) -> Stored {
        Stored {
            rev,
            text: record_piece(record),
            record: record.clone(),
        }
    }

    /// The `(qualified collection, id)` a record is keyed by — the tuple order, stated once
    /// so a caller building a `Model` by hand cannot spell it differently.
    pub fn key_of(record: &Record) -> (String, String) {
        (record.collection.clone(), record.id.clone())
    }
}
