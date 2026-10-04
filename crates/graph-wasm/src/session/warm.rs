//! `gm_force_session_create_warm`'s body: a session seeded on the graph's drawn picture. A
//! child of [`super`] so the session table stays private to the one module that owns it; split
//! out by the house's 300-line limit.

use super::{Engine, insert};
use crate::errors::Code;
use graph_core::layout::force::{ForceSession, LiveParams, SessionError};
use graph_core::post::centres;
use graph_core::{Geometry, Topology};

/// [`create`](super::create) seeded on `geometry`'s node centres, the picture the graph's last layout run
/// drew, instead of on the engine's spiral: the session continues that picture rather than
/// replacing it with one of its own (`docs/decisions/force-session-warm-seed.md`).
///
/// Refused with [`Code::NoGeometryYet`] before any run, and with [`Code::TamperedGeometry`]
/// for a centre that is not finite or a column that is not one value per node. The `f32`
/// centres widen to `f64` exactly, so the session starts on the drawn coordinates bit for bit.
pub fn create_warm(
    graph: u32,
    (topology, geometry): (&Topology, Option<&Geometry>),
    params: LiveParams,
    engine: Engine,
) -> Result<u32, Code> {
    let (xs, ys) = centres(&geometry.ok_or(Code::NoGeometryYet)?.nodes);
    let widen = |column: &[f32]| column.iter().map(|&v| f64::from(v)).collect::<Vec<_>>();
    let session = ForceSession::from_positions(topology, params, &widen(xs), &widen(ys)).map_err(
        |error| match error {
            SessionError::NonFinite { field: "xs" | "ys" } | SessionError::ColumnLength { .. } => {
                Code::TamperedGeometry
            }
            _ => Code::SessionRefused,
        },
    )?;
    insert(graph, session, engine)
}
