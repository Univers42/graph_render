//! The JSON face's shape as Rust types, for `docs/contract/snapshot-schema.json`. They
//! describe; they do not read or write — [`super::to_json`] and [`super::from_json`] do.
//! The tests hold the two together: serde reads every canonical document into these
//! types, and [`super::from_json`] reads back what serde writes from them.

use crate::version::{FormatVersion, UNVERSIONED};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A graph-motor snapshot, semantic face: canonical JSON (compact, keys sorted by bytes,
/// one trailing newline). Byte layout and rules: `docs/contract/binary-layout.md`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// How many dimensions every node carries: `0` 2D, `1` 3D. Written from format 0.4
    /// and absent below it, where it reads as 0 — the same optional-member rule as
    /// `notes`. A 3D snapshot's `geometry.nodes` carries a `z` column; a 2D one's does
    /// not, and one that does is refused. JSON Schema cannot tie a member's presence to
    /// another member's value, so the rule is stated here and the reader enforces it.
    #[serde(default)]
    #[schemars(schema_with = "crate::snapshot::dim::dim_schema")]
    pub dim: crate::snapshot::Dim,
    /// Every edge's identity and endpoints, in edge order.
    pub edges: Edges,
    /// Where and how every node and edge is drawn.
    pub geometry: Geometry,
    /// Every node's identity, in node order.
    pub nodes: Nodes,
    /// What the stages repaired or approximated. Required from format 0.3; below 0.3 a
    /// snapshot carries none and an absent member reads as none. JSON Schema cannot tie
    /// a member's presence to the version's value, so this schema lists it as optional
    /// and the reader enforces the rule.
    #[serde(default)]
    pub notes: Notes,
    /// The format version. A reader refuses a newer major; absent reads as 0.0.
    #[serde(default = "unversioned")]
    pub version: FormatVersion,
}

fn unversioned() -> FormatVersion {
    UNVERSIONED
}

/// The notes section (format 0.3), one entry per note in both columns, strictly
/// ascending by `(code, index)`: a closed, canonical set, so a reserved or unallocated
/// code, a repeat or an out-of-order note is refused.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notes {
    /// Each note's code: 1 hierarchy.cycle_edge_dropped, 2 hierarchy.extra_parent_dropped,
    /// 3 packing.approximate, 4 dag.dummy_budget_exceeded, 5 dag.edge_reversed. 6 is
    /// reserved for a later phase and refused, as is any other.
    #[schemars(schema_with = "note_codes")]
    pub code: Vec<u32>,
    /// Each note's index: for codes 1, 2, 4 and 5 an edge position (an index into
    /// edges.id, below its length); for code 3 the literal 4294967295 (u32::MAX), the whole
    /// snapshot.
    pub index: Vec<u32>,
}

fn note_codes(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let codes: Vec<u32> = crate::notes::NoteCode::ALL
        .iter()
        .map(|c| c.code())
        .collect();
    schemars::json_schema!({
        "type": "array",
        "items": { "type": "integer", "format": "uint32", "enum": codes }
    })
}

/// A z column: an array of `f32` and nothing else, never `null`. `Option` would make
/// schemars admit `null` as well, which the reader refuses (`read/geometry.rs` takes a
/// z column or refuses, and a `null` is neither); absence is the whole of what `Option`
/// means here, because whether `z` must be there follows the top-level `dim`, which JSON
/// Schema cannot tie a member's presence to — so `z` stays out of `required` and the
/// reader decides. The doc comment still lands as the `description`.
fn z_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "array",
        "items": { "type": "number", "format": "float" }
    })
}

/// Node identity.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Nodes {
    /// Stable node ids, unique. A node's position here is its position in every node column.
    pub id: Vec<String>,
}

/// Edge identity and endpoints.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Edges {
    /// Stable edge ids, unique.
    pub id: Vec<String>,
    /// Each edge's source, as a node id.
    pub source: Vec<String>,
    /// Each edge's target, as a node id.
    pub target: Vec<String>,
}

/// One geometry for the whole snapshot: one node kind, one edge kind.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    /// Edge geometry.
    pub edges: EdgeGeometry,
    /// Node geometry.
    pub nodes: NodeGeometry,
}

/// Node geometry as columns, one value per node, in node order. Every value is a
/// finite `f32`, written as the shortest decimal that reads back as it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum NodeGeometry {
    /// Centres.
    Point {
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
        /// Depth centre. Present iff the snapshot's `dim` is 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(schema_with = "z_schema")]
        z: Option<Vec<f32>>,
    },
    /// Centres and radii.
    Circle {
        /// Radius, never negative.
        r: Vec<f32>,
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
        /// Depth centre. Present iff the snapshot's `dim` is 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(schema_with = "z_schema")]
        z: Option<Vec<f32>>,
    },
    /// Boxes.
    Box {
        /// Height, never negative.
        h: Vec<f32>,
        /// Width, never negative.
        w: Vec<f32>,
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
        /// Depth centre. Present iff the snapshot's `dim` is 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(schema_with = "z_schema")]
        z: Option<Vec<f32>>,
    },
}

/// Edge geometry. Edge `e` owns points `offsets[e]..offsets[e+1]`; point `p` is
/// `(pts[2p], pts[2p+1])`. A row holds the interior points, source to target; an empty
/// row is drawn straight.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EdgeGeometry {
    /// Straight from source to target; nothing stored.
    Line,
    /// Straight segments through each edge's interior points.
    Polyline {
        /// `m + 1` point offsets, from 0, never decreasing.
        offsets: Vec<u32>,
        /// `2 × offsets[m]` coordinates, x then y for each point.
        pts: Vec<f32>,
    },
    /// A curve of one degree for every edge, through each edge's control points.
    Curve {
        /// The curve degree, at least 1 (2 quadratic, 3 cubic).
        degree: u32,
        /// `m + 1` point offsets, from 0, never decreasing.
        offsets: Vec<u32>,
        /// `2 × offsets[m]` coordinates, x then y for each point.
        pts: Vec<f32>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::NoteCode;

    #[test]
    fn the_schema_admits_exactly_the_implemented_note_codes() {
        let schema = note_codes(&mut schemars::SchemaGenerator::default());
        let admitted = schema.as_value()["items"]["enum"].clone();
        let implemented: Vec<u32> = NoteCode::ALL.iter().map(|c| c.code()).collect();
        assert_eq!(admitted, serde_json::json!(implemented));
        assert_eq!(implemented, [1, 2, 3, 4, 5]);
    }
}
