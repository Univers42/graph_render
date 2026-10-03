//! The walk: one pass over a document's records, building nodes and edges.
//!
//! Split out of `build.rs` for the house's 300-line limit and for no other reason —
//! this is the same code, in the same order, and the Ponytail and the determinism
//! argument are in the parent module doc, which is the one to read.

use super::BuildError;
use crate::ingest::roles::{self, DEFAULT_WEIGHT};
use crate::{EdgeKind, EdgeRecord, NodeKind, NodeRecord, make_record_node_id, make_tag_node_id};
use graph_contract::ingest::{Ingest, Record};
use indexmap::IndexSet;

mod spec;
use spec::{Ends, Spec};

pub(super) struct Builder<'a> {
    doc: &'a Ingest,
    nodes: Vec<NodeRecord>,
    edges: Vec<EdgeRecord>,
    tags: IndexSet<String>,
    /// The relation edge ids already stated, over the whole walk. An `IndexSet`, so the
    /// order is first-appearance and the order the records were derived in — nothing is
    /// re-sorted and nothing is read back (D3).
    edges_stated: IndexSet<String>,
}

impl<'a> Builder<'a> {
    /// An empty walk over `doc`. Every accumulator starts empty: the derivation never
    /// reads back what it wrote, so there is nothing to seed.
    pub(super) fn new(doc: &'a Ingest) -> Self {
        Self {
            doc,
            nodes: Vec::new(),
            edges: Vec::new(),
            tags: IndexSet::new(),
            edges_stated: IndexSet::new(),
        }
    }

    pub(super) fn take_nodes(&mut self) -> Vec<NodeRecord> {
        std::mem::take(&mut self.nodes)
    }

    pub(super) fn take_edges(&mut self) -> Vec<EdgeRecord> {
        std::mem::take(&mut self.edges)
    }
}

impl Builder<'_> {
    /// Every link target named anywhere in the document, checked before any record is
    /// derived — so a document with a bad declaration is refused the same way whichever
    /// record happened to come first.
    pub(super) fn check_targets(&self) -> Result<(), BuildError> {
        for collection in &self.doc.collections {
            for field in roles::link_fields(collection) {
                let Some(link) = &field.link else { continue };
                if self.doc.collection(&link.collection).is_none() {
                    return Err(BuildError::UnknownLinkTarget {
                        collection: collection.id.clone(),
                        field: field.id.clone(),
                        target: link.collection.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Every record in document order, refusing a repeated id first-wins style. The
    /// edges of one record come out hierarchy, then relations, then tags — a fixed
    /// order, so two derivations of one document agree without a sort.
    ///
    /// The key is the **(collection, id)** pair, not the bare id: `Record.id` is unique
    /// within its collection, and the derived node id carries the collection
    /// (`source:collection:record`), so the same id in two collections is two records
    /// with two distinct nodes. Keyed on the id alone, a document the contract permits
    /// was refused with an error naming a record that was not the one at fault.
    pub(super) fn run(&mut self) -> Result<(), BuildError> {
        let mut seen: IndexSet<(&str, &str)> = IndexSet::new();
        for record in &self.doc.records {
            if !seen.insert((record.collection.as_str(), record.id.as_str())) {
                return Err(BuildError::DuplicateRecord {
                    collection: record.collection.clone(),
                    record: record.id.clone(),
                });
            }
            if record.deleted {
                continue;
            }
            self.node(record)?;
            self.hierarchy(record)?;
            self.relations(record)?;
            self.tags_of(record)?;
        }
        self.tag_hubs();
        Ok(())
    }

    fn node(&mut self, record: &Record) -> Result<(), BuildError> {
        let collection = self.collection_of(record)?;
        let id = make_record_node_id(&self.doc.source, &collection.id, &record.id);
        self.nodes.push(NodeRecord {
            id,
            kind: NodeKind::Record,
            database_id: Some(collection.id.clone()),
            source: self.doc.source.clone(),
            // No title value is not an empty label: the record's own id is, and it is
            // the one string the document always carries.
            label: roles::label(self.doc, record)
                .unwrap_or(&record.id)
                .to_owned(),
            group: roles::group(self.doc, record).map(str::to_owned),
            weight: roles::weight(self.doc, record),
            version: f64::from(record.updated_at),
            has_note: false,
            icon: None,
        });
        Ok(())
    }

    fn hierarchy(&mut self, record: &Record) -> Result<(), BuildError> {
        let Some(parent) = roles::parent(self.doc, record) else {
            return Ok(());
        };
        let child = self.node_id(record);
        let parent = self.node_id_in(&record.collection, parent);
        // Parent first, so `Topology::hierarchy` files the edge under its parent and a
        // `child_first` flag is never needed for a derived edge.
        self.edges
            .push(Spec::new(Ends::new(parent, child), EdgeKind::Hierarchy, "", false).edge());
        Ok(())
    }

    fn relations(&mut self, record: &Record) -> Result<(), BuildError> {
        for spec in self.relation_specs(record)? {
            // A claim stated twice is one fact, so it is one edge: a reference carried
            // twice in one list, and a link both records of a symmetric pair carry, each
            // derive the same id twice. The ids would be identical either way and
            // `index_model` keeps the first, so the derivation states one edge rather
            // than leaving a consumer to discover the duplicate. The key is the **edge
            // id**, over the whole walk rather than one record, because the two ways of
            // saying it live in two records.
            if self.edges_stated.insert(spec.id()) {
                self.edges.push(spec.edge());
            }
        }
        Ok(())
    }

    /// Every relation edge `record` states, in canonical link-field order and in the
    /// order each field's references were written. No dedup here: a repeated claim is
    /// filtered against the whole walk's ids by the caller, which is the only place that
    /// can see claims from an earlier record.
    fn relation_specs(&self, record: &Record) -> Result<Vec<Spec>, BuildError> {
        let collection = self.collection_of(record)?;
        let source = self.node_id(record);
        let mut specs = Vec::new();
        for field in roles::link_fields(collection) {
            let Some(link) = &field.link else { continue };
            for target in roles::references(self.doc, record, &field.id) {
                let target = self.node_id_in(&link.collection, &target);
                // The **field id**, never the field's human name. The contract says `name`
                // is "for diagnostics only. Nothing derives from it", and an edge id is
                // derived identity: two adapters declaring the same link under different
                // display names (`blocks` / "Blocks") derive one edge, not two.
                specs.push(Spec::new(
                    Ends::new(source.clone(), target),
                    EdgeKind::Relation,
                    &field.id,
                    // A symmetric link is drawn and identified without direction, so an
                    // A→B and a B→A are one edge rather than two.
                    !link.symmetric,
                ));
            }
        }
        Ok(specs)
    }

    fn tags_of(&mut self, record: &Record) -> Result<(), BuildError> {
        let source = self.node_id(record);
        let values = roles::tags(self.doc, record);
        let mut specs = Vec::new();
        let mut emitted: IndexSet<&str> = IndexSet::new();
        for value in &values {
            check_tag(value)?;
            // A value carried twice in one record is one fact, so it is one edge: the
            // edge id would be identical either way and `index_model` would drop the
            // second, but the derivation states what it derived rather than leaving a
            // consumer to discover the duplicate.
            if !emitted.insert(value.as_str()) {
                continue;
            }
            self.tags.insert(value.clone());
            specs.push(Spec::new(
                Ends::new(source.clone(), make_tag_node_id(value)),
                EdgeKind::Tag,
                value,
                false,
            ));
        }
        self.edges.extend(specs.iter().map(Spec::edge));
        Ok(())
    }

    /// The tag hubs, after every record node, in first-appearance order. A separate
    /// pass so a node list is "records, then tags" whatever order the records and the
    /// tags arrived in — the order a consumer can rely on.
    fn tag_hubs(&mut self) {
        for value in &self.tags {
            self.nodes.push(NodeRecord {
                id: make_tag_node_id(value),
                kind: NodeKind::Tag,
                database_id: None,
                source: self.doc.source.clone(),
                label: value.clone(),
                group: None,
                weight: DEFAULT_WEIGHT,
                version: 0.0,
                has_note: false,
                icon: None,
            });
        }
    }

    /// A record's own collection, or the refusal naming both the record and the
    /// collection it asked for: "no such collection" alone is not actionable when a
    /// document declares several.
    fn collection_of(
        &self,
        record: &Record,
    ) -> Result<&graph_contract::ingest::Collection, BuildError> {
        self.doc
            .collection(&record.collection)
            .ok_or_else(|| BuildError::UnknownCollection {
                record: record.id.clone(),
                collection: record.collection.clone(),
            })
    }

    fn node_id(&self, record: &Record) -> String {
        self.node_id_in(&record.collection, &record.id)
    }

    /// A node id for a record named by *reference*, which may live in another
    /// collection: the id's middle coordinate is the referenced record's own
    /// collection, not the referencing one. Reading it the other way round would make
    /// every cross-collection link dangle.
    fn node_id_in(&self, collection_id: &str, record_id: &str) -> String {
        make_record_node_id(&self.doc.source, collection_id, record_id)
    }
}

/// H5 for a tag value: see the module doc.
fn check_tag(value: &str) -> Result<(), BuildError> {
    if value.contains(':') {
        return Err(BuildError::IdGrammar {
            coordinate: "tag value",
            value: value.to_owned(),
        });
    }
    Ok(())
}
