//! The model as a **document**: the pieces, in document order, and the length of it.
//!
//! Split out of `model.rs` by the house's 300-line limit, and because it is a different
//! question: `model.rs` owns the state and the transitions, this owns what that state is
//! *written* as. A writer needs two facts and nothing else — which collections survive
//! pruning, and which references resolve — and both are asked of the model through
//! [`Model::collections`] and the `exists` closure below, so this half cannot reach into
//! the maps itself.

use super::{Model, Stored, qualify};
use crate::hub::{HubError, Manifest};
use crate::ingest::{Collection, Ingest, collection_piece, doc_tail, frame_bytes, record_piece};

impl Model {
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
        let collections: &[Collection] = &collections;
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

    /// The document's byte length **before** any pruning, and so an exact upper bound of
    /// `to_json().len()` (spec §6).
    ///
    /// Counted from the *stored* record texts rather than from re-pruned ones, and that is
    /// the whole point of the function: a store can answer "how big is this document" from
    /// what it holds, without building the pieces and without knowing which references
    /// dangle. So it is exact when nothing is pruned and an over-estimate when something
    /// is — never an under-estimate, which is the direction that would size a buffer too
    /// small.
    pub fn doc_bytes(&self) -> u64 {
        let collections = self.collections();
        // The *kept* collection count, not the plugin count: `frame_bytes` counts the
        // separators between collections, and one plugin may contribute three of them.
        let frame = frame_bytes(
            &self.workspace,
            collections.len() as u64,
            self.records.len() as u64,
        );
        let collections: u64 = collections
            .iter()
            .map(|c| collection_piece(c).len() as u64)
            .sum();
        let records: u64 = self.records.values().map(|s| s.text.len() as u64).sum();
        frame + collections + records
    }

    /// Every piece of the document as it is *written*, in order: the collections', then
    /// each record's. `to_json` concatenates these; `doc_bytes` deliberately does not sum
    /// them, because it counts the stored texts so it stays an upper bound.
    pub fn pieces(&self) -> Vec<String> {
        let collections: &[Collection] = &self.collections();
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

    /// The kept declaration of every collection, **by qualified id**.
    ///
    /// Qualified, and this is load-bearing twice over. A record's `collection` is
    /// `plugin.coll` — the batch reader refuses a qualified one precisely so the store
    /// qualifies it exactly once — so a declaration left bare would name a collection no
    /// record is in: every lookup would miss, every dangling check would pass, and every
    /// pruning decision would be made against nothing. It is also what lets two plugins
    /// both declare a collection called `task` in one document at all.
    ///
    /// Sorted by that qualified id, so `"a-b.c"` comes before `"a.x"`: `-` is `0x2D` and
    /// `.` is `0x2E`, and the *qualified string* is what is compared — not the plugin and
    /// the collection separately, which would order by plugin first and silently disagree.
    pub fn collections(&self) -> Vec<Collection> {
        let mut out: Vec<Collection> = self
            .plugins
            .iter()
            .flat_map(|(plugin, manifest)| {
                manifest
                    .collections
                    .iter()
                    .map(move |c| (plugin.as_str(), c))
            })
            .map(|(plugin, c)| {
                let mut kept = crate::hub::prune::kept_collection(c, &|target| {
                    self.registered(plugin, target)
                });
                kept.id = qualify(plugin, &c.id);
                kept
            })
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
        match crate::hub::prune::prune_record(&stored.record, collection, &|c, id| {
            self.records.contains_key(&(c.to_owned(), id.to_owned()))
        }) {
            Some(pruned) => record_piece(&pruned),
            None => stored.text.clone(),
        }
    }
}
