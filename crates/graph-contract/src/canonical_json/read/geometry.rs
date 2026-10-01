//! The `geometry` member: node and edge columns, in the shape the schema names.
//!
//! Split out of `read.rs` by the house's 300-line limit, and because the node columns carry
//! the one rule the JSON face has that the rest of the document does not: the z column's
//! presence follows the top-level `dim`, so `dim` has to be read before this runs.

use super::{Object, f32_of, kind, list, shape, u32_of};
use crate::canonical_json::{EDGE_KINDS, JsonError, NODE_KINDS};
use crate::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, NodeGeometryKind, Paths};
use crate::snapshot::Dim;

/// The node columns, with the z column iff `dim` is 3D. Both directions are refused rather
/// than assumed: a 3D snapshot with no `z` (`z` is required) and a 2D one with a `z` (`z` is
/// not part of its shape) each name what was wrong.
pub(super) fn node_geometry(
    mut object: Object,
    dim: Dim,
) -> Result<(NodeGeometry, Option<Vec<f32>>), JsonError> {
    let mut column = |key| list(object.take(key)?, f32_of);
    let (x, y) = (column("x")?, column("y")?);
    let z = match object.maybe("z") {
        Some(_) if !dim.is_3d() => {
            return Err(shape(
                "geometry.nodes.z",
                "is not part of a 2D snapshot's shape",
            ));
        }
        Some(value) => list((value, "geometry.nodes.z".into()), f32_of)?,
        None if dim.is_3d() => return Err(shape("geometry.nodes.z", "is missing")),
        None => Vec::new(),
    };
    let geometry = match kind(&mut object, &NODE_KINDS)? {
        NodeGeometryKind::Point => NodeGeometry::Point { x, y },
        NodeGeometryKind::Circle => NodeGeometry::Circle {
            x,
            y,
            r: list(object.take("r")?, f32_of)?,
        },
        NodeGeometryKind::Box => NodeGeometry::Box {
            x,
            y,
            w: list(object.take("w")?, f32_of)?,
            h: list(object.take("h")?, f32_of)?,
        },
    };
    object.finish()?;
    Ok((geometry, dim.is_3d().then_some(z)))
}

pub(super) fn edge_geometry(mut object: Object) -> Result<EdgeGeometry, JsonError> {
    let geometry = match kind(&mut object, &EDGE_KINDS)? {
        EdgeGeometryKind::Line => EdgeGeometry::Line,
        EdgeGeometryKind::Polyline => EdgeGeometry::Polyline(paths(&mut object)?),
        EdgeGeometryKind::Curve => {
            let (degree, path) = object.take("degree")?;
            let degree = u32_of(degree, &path)?;
            EdgeGeometry::Curve {
                degree,
                paths: paths(&mut object)?,
            }
        }
    };
    object.finish()?;
    Ok(geometry)
}

fn paths(object: &mut Object) -> Result<Paths, JsonError> {
    Ok(Paths {
        offsets: list(object.take("offsets")?, u32_of)?,
        pts: list(object.take("pts")?, f32_of)?,
    })
}
