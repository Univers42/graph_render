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

use super::breaks;
use super::manifest::{Growth, Manifest, growth};
use super::prune::prune_record;
use super::{HubError, Limits, MAX_PLUGINS, check_workspace_id, qualify};
use crate::ingest::{
    Collection, Ingest, JsonValue, Record, collection_piece, doc_tail, frame_bytes,
    record_piece, to_json,
};
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
        let mut staged = feed::stage(self, plugin, batch)?;
        let applied = Applied {
            upserted: staged.upserted.clone(),
            deleted: staged.deleted.clone(),
        };
        self.records = staged.into_records();
        Ok(applied)
    }

    /// The `document`'s whole model: what a read of every plugin would write. The test's
    /// assertion `to_json == ingest::to_json(&to_ingest())` is the reason both exist — the
    /// streaming writer and the built document are two directions of one format.
    pub fn to_ingest(&self) -> Ingest {
        Ingest {
            version: crate::ingest::VERSION,
            source: self.workspace.clone(),
            collections: self.collections().to_vec(),
            records: self
                .records
                .values()
                .map(|stored| stored.record.clone())
                .collect(),
        }
    }

    /// The document's canonical text: the pieces, in document order, with a record that
    /// has nothing to prune written from its stored bytes.
    pub fn to_json(&self) -> String {
        let collections = self.collections();
        let mut out = String::from(crate::ingest::DOC_HEAD);
        out.push_str(
            &collections
                .iter()
                .map(collection_piece)
                .collect::<Vec<_>>()
                .join(crate::ingest::DOC_SEPARATOR),
        );
        out.push_str(crate::ingest::DOC_MIDDLE);
        let mut first = true;
        for stored in self.records.values() {
            if !first {
                out.push_str(crate::ingest::DOC_SEPARATOR);
            }
            first = false;
            out.push_str(&self.piece(stored, collections));
        }
        out.push_str(&doc_tail(&self.workspace));
        out
    }

    /// The document's byte length, counted and not measured: `frame` plus every piece, or
    /// `to_json().len()` — the two must agree, which is why this is not just the latter.
    pub fn doc_bytes(&self) -> u64 {
        let frame = frame_bytes(
            &self.workspace,
            self.plugins.len() as u64,
            self.records.len() as u64,
        );
        frame + self.pieces().iter().map(String::len).sum::<usize>() as u64
    }

    /// Every piece of the document, in order: the collections', then each record's. `doc_bytes`
    /// sums these; `to_json` concatenates them, so the bound and the bytes cannot disagree.
    pub fn pieces(&self) -> Vec<String> {
        let collections = self.collections();
        let mut out: Vec<String> = collections.iter().map(collection_piece).collect();
        out.extend(self.records.values().map(|s| self.piece(s, collections)));
        out
    }

    /// `plugin`'s registered manifest, or the refusal. A batch for a plugin that has not
    /// registered one is a 422, not an empty manifest: the records would then have no
    /// fields to be checked against, and "no fields" accepts anything.
    pub fn manifest_of(&self, plugin: &str) -> Result<&Manifest, HubError> {
        self.plugins.get(plugin).ok_or_else(|| HubError::Invalid {
            path: "plugin".to_owned(),
            what: format!("plugin `{plugin}` has no registered manifest"),
        })
    }

    /// Every stored record, in document order.
    pub fn records(&self) -> impl Iterator<Item = &Stored> {
        self.records.values()
    }

    /// The stored record at `(collection, id)`, or `None`.
    pub fn stored(&self, collection: &str, id: &str) -> Option<&Stored> {
        self.records.get(&(collection.to_owned(), id.to_owned()))
    }

    /// The kept declaration of every collection, by qualified id.
    ///
    /// Sorted by qualified id, so `"a-b.c"` comes before `"a.x"`: `-` is `0x2D` and `.` is
    /// `0x2E`, and the *qualified string* is what is compared — not the plugin and
    /// collection separately, which would order by plugin first and silently disagree.
    pub fn collections(&self) -> Vec<Collection> {
        let mut out: Vec<Collection> = self
            .plugins
            .iter()
            .flat_map(|(plugin, manifest)| {
                manifest.collections.iter().map(move |c| (plugin.as_str(), c))
            })
            .map(|(plugin, c)| super::prune::kept_collection(c, &|target| {
                self.registered(plugin, target)
            }))
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Whether `target` names a collection some plugin in this model has registered. The
    /// link check `kept_collection` uses, and the model answers it — a link to another
    /// plugin's collection is legal in a manifest and only decidable here.
    fn registered(&self, plugin: &str, target: &str) -> bool {
        let (owner, collection) = target.split_once('.').unwrap_or((plugin, target));
        self.plugins
            .get(owner)
            .is_some_and(|m| m.collections.iter().any(|c| c.id == collection))
    }

    /// One record's text: its stored bytes when nothing is pruned, and the pruned record's
    /// own piece when something is.
    fn piece(&self, stored: &Stored, collections: &[Collection]) -> String {
        let Some(collection) = collections
            .iter()
            .find(|c| c.id == stored.record.collection)
        else {
            // Unreachable through `apply`, which refuses an undeclared collection. Kept as
            // the stored bytes rather than a panic: a model built by hand must not be able
            // to abort the process, and the stored text is the best answer available.
            return stored.text.clone();
        };
        match prune_record(&stored.record, collection, &|c, id| {
            self.records.contains_key(&(c.to_owned(), id.to_owned()))
        }) {
            Some(pruned) => record_piece(&pruned),
            None => stored.text.clone(),
        }
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