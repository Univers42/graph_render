//! What the document declares, and how one stored record is written under it.
//!
//! The same two rules as `graph_contract::hub::Model::collections` and its `piece`: a `link`
//! field into a collection no registered plugin declares is dropped (`kept_collection`), and a
//! reference cell naming a record that is not stored is dropped (`prune_record`). Both are
//! graph-contract's own functions, called rather than restated, so the breaks that turn them off
//! turn them off here as well.

use std::collections::{BTreeMap, BTreeSet};

use graph_contract::hub::prune::{kept_collection, prune_record};
use graph_contract::hub::{Manifest, qualify};
use graph_contract::ingest::{
    Collection, DOC_HEAD, DOC_MIDDLE, DOC_SEPARATOR, Field, Role, collection_piece, record_piece,
};

use crate::error::StoreError;

/// The `(qualified collection, id)` pairs one record references and the store does not hold.
pub(crate) type Dangling = BTreeSet<(String, String)>;

/// The kept declarations of a workspace, and which of them can need pruning at all.
pub(crate) struct Declared {
    /// Every kept collection under its qualified id, sorted by that id (the document's order).
    collections: Vec<Collection>,
    /// The qualified ids whose *declared* collection has a reference field, before pruning.
    referencing: BTreeSet<String>,
}

impl Declared {
    /// The declarations of `manifests`, keyed by plugin, as the document writes them.
    pub(crate) fn of(manifests: &BTreeMap<String, Manifest>) -> Declared {
        let mut collections = Vec::new();
        let mut referencing = BTreeSet::new();
        for (plugin, manifest) in manifests {
            for c in &manifest.collections {
                let qcoll = qualify(plugin, &c.id);
                if c.fields.iter().any(refers) {
                    referencing.insert(qcoll.clone());
                }
                let mut kept =
                    kept_collection(c, &|target| registered(manifests, plugin, target));
                kept.id = qcoll;
                collections.push(kept);
            }
        }
        collections.sort_by(|a, b| a.id.cmp(&b.id));
        Declared {
            collections,
            referencing,
        }
    }

    /// Everything before the first record: the head, the declarations, the middle.
    pub(crate) fn head(&self) -> String {
        let mut out = String::from(DOC_HEAD);
        out.push_str(
            &self
                .collections
                .iter()
                .map(collection_piece)
                .collect::<Vec<_>>()
                .join(DOC_SEPARATOR),
        );
        out.push_str(DOC_MIDDLE);
        out
    }

    /// The record stored as `text` in `qcoll`, as the document writes it.
    ///
    /// Caveat: a record of a collection with a reference field is parsed whenever it is written,
    /// even when nothing in it dangles, because a non-text cell on a link field (a number, a
    /// map) is dropped by pruning and has no `links` row to announce it. The cost is linear in
    /// the record's bytes; the upgrade path is a "needs pruning" flag the writer records.
    pub(crate) fn piece(
        &self,
        qcoll: &str,
        text: String,
        dangling: &Dangling,
    ) -> Result<String, StoreError> {
        let Ok(at) = self
            .collections
            .binary_search_by(|c| c.id.as_str().cmp(qcoll))
        else {
            return Ok(text);
        };
        if !self.referencing.contains(qcoll) {
            return Ok(text);
        }
        let record = super::record::parse(&text)?;
        let exists = |q: &str, id: &str| !dangling.contains(&(q.to_owned(), id.to_owned()));
        Ok(match prune_record(&record, &self.collections[at], &exists) {
            Some(pruned) => record_piece(&pruned),
            None => text,
        })
    }
}

/// Whether `field` can name another record, by its declaration rather than by its cells.
fn refers(field: &Field) -> bool {
    field.link.is_some() || matches!(field.role, Role::Link | Role::Parent)
}

/// Whether `target`, read from `plugin`'s manifest, names a collection some plugin declares.
fn registered(manifests: &BTreeMap<String, Manifest>, plugin: &str, target: &str) -> bool {
    let (owner, collection) = target.split_once('.').unwrap_or((plugin, target));
    manifests
        .get(owner)
        .is_some_and(|m| m.collections.iter().any(|c| c.id == collection))
}
