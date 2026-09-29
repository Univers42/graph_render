//! A collection of records and the roles of its fields.
//!
//! Split out of `ingest.rs` for the house's 300-line limit and for no other reason.
//! The interesting part is the ordering rule, and it is on `Collection::fields`.

use super::{Field, Role};

/// A collection of records and the roles of its fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Collection {
    /// Stable id, unique in the document; the middle coordinate of a node id.
    pub id: String,
    /// Human name, for diagnostics only.
    pub name: String,
    /// Id of the field whose value is a record's label.
    pub title_field: String,
    /// Its declared fields, in **canonical order: sorted by field id**. Not the order
    /// the source listed them, which is not a fact the contract can rely on — two
    /// exports of one dataset, or two adapters mapping two source shapes, list the same
    /// fields in different orders. Sorting here means `fields[0]` and "the first field
    /// with a role" are the same thing in every document, and a document's byte text
    /// depends on what it *declares* rather than on how it was written down.
    pub fields: Vec<Field>,
}

impl Collection {
    /// The field with id `id`, or `None`. The only lookup the derivation uses — a
    /// declared id, never "the first field of some role".
    pub fn field(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.id == id)
    }

    /// The lowest-id field with `role`, or `None`. Ties are broken by the id, never by
    /// the order the fields arrived in (H6, D5): two fields with the same role are a
    /// schema mistake, and picking between them by document order would make the
    /// derivation depend on a reserialization.
    pub fn first_with_role(&self, role: Role) -> Option<&Field> {
        self.fields.iter().find(|field| field.role == role)
    }
}
