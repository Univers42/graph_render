//! The cells a document under construction holds: two node columns' worth of `u32`s,
//! two edge columns', and the byte offsets a negative test patches.

/// One node row's cells, before they are written. Every optional starts absent so a test
/// only has to name the fields it cares about.
#[derive(Clone, Copy)]
pub struct NodeCells {
    /// `weight`.
    pub weight: f64,
    /// `version`.
    pub version: f64,
    /// id string index.
    pub id: u32,
    /// kind string index.
    pub kind: u32,
    /// database string index.
    pub database: u32,
    /// source string index.
    pub source: u32,
    /// label string index.
    pub label: u32,
    /// group string index.
    pub group: u32,
    /// icon string index.
    pub icon: u32,
    /// `0` or `1`.
    pub has_note: u32,
}

/// One edge row's cells.
#[derive(Clone, Copy)]
pub struct EdgeCells {
    /// id string index.
    pub id: u32,
    /// source node row.
    pub source: u32,
    /// target node row.
    pub target: u32,
    /// kind string index.
    pub kind: u32,
    /// label string index.
    pub label: u32,
    /// record id string index.
    pub record_id: u32,
    /// `0` or `1`.
    pub directed: u32,
    /// `0` or `1`.
    pub child_first: u32,
    /// `strength`.
    pub strength: f64,
}

/// Where each section starts, so a negative test patches one cell instead of rebuilding the
/// whole document. Byte offsets into [`Encoded::bytes`].
#[derive(Debug, Clone, Copy)]
pub struct Marks {
    /// The offsets table.
    pub offsets: usize,
    /// The blob.
    pub blob: usize,
    /// The first pad byte; its length is `(-marks.pad) % 8` from the buffer start.
    pub pad: usize,
    /// `node weight`.
    pub weight: usize,
    /// `node version`.
    pub version: usize,
    /// `edge strength`.
    pub strength: usize,
    /// `node id`.
    pub node_id: usize,
    /// `node kind`.
    pub node_kind: usize,
    /// `node database`.
    pub node_database: usize,
    /// `node source`.
    pub node_source: usize,
    /// `node label`.
    pub node_label: usize,
    /// `node group`.
    pub node_group: usize,
    /// `node icon`.
    pub node_icon: usize,
    /// `node has_note`.
    pub node_has_note: usize,
    /// `edge id`.
    pub edge_id: usize,
    /// `edge source`.
    pub edge_source: usize,
    /// `edge target`.
    pub edge_target: usize,
    /// `edge kind`.
    pub edge_kind: usize,
    /// `edge label`.
    pub edge_label: usize,
    /// `edge record_id`.
    pub record_id: usize,
    /// `edge directed`.
    pub directed: usize,
    /// `edge child_first`.
    pub child_first: usize,
}
