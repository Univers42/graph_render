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
    /// Every edge's identity and endpoints, in edge order.
    pub edges: Edges,
    /// Where and how every node and edge is drawn.
    pub geometry: Geometry,
    /// Every node's identity, in node order.
    pub nodes: Nodes,
    /// The format version. A reader refuses a newer major; absent reads as 0.0.
    #[serde(default = "unversioned")]
    pub version: FormatVersion,
}

fn unversioned() -> FormatVersion {
    UNVERSIONED
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
    },
    /// Centres and radii.
    Circle {
        /// Radius, never negative.
        r: Vec<f32>,
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
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
