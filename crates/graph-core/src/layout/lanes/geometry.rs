//! Points and polylines. A vertex sits at `(lane · lane_spacing, row · row_spacing)`. An edge
//! carried in lane `l` bends into `l` half a row after its earlier end when `l` is not that
//! end's lane, and out of `l` half a row before its later end when `l` is not that end's lane.
//! Its interior points run from the edge's own source to its own target. A self-loop has none,
//! the same convention as `layout.dag.sugiyama`.

use super::LanesParams;
use super::assign::Drawing;
use super::rows::Rows;
use crate::index::Topology;
use graph_contract::geometry::Paths;

/// Every vertex's `(x, y)`, in node order.
pub(super) fn positions(
    drawing: &Drawing,
    rows: &Rows,
    params: &LanesParams,
) -> (Vec<f32>, Vec<f32>) {
    let x = drawing
        .lane
        .iter()
        .map(|&l| l as f32 * params.lane_spacing)
        .collect();
    let y = rows
        .row
        .iter()
        .map(|&r| r as f32 * params.row_spacing)
        .collect();
    (x, y)
}

/// Every edge's interior points, CSR-shaped like `layout.dag.sugiyama`'s.
pub(super) fn paths(
    drawing: &Drawing,
    topology: &Topology,
    rows: &Rows,
    params: &LanesParams,
) -> Paths {
    let cols = topology.edges();
    let m = cols.source.len();
    let mut offsets = Vec::with_capacity(m + 1);
    let mut pts = Vec::with_capacity(2 * m);
    offsets.push(0);
    for e in 0..m {
        let (s, t) = (cols.source[e], cols.target[e]);
        if s != t {
            let route = Route {
                source: s,
                target: t,
                lane: drawing.carried[e],
            };
            push_route(&mut pts, route, (drawing, rows), params);
        }
        offsets.push((pts.len() / 2) as u32);
    }
    Paths { offsets, pts }
}

/// One edge: its endpoints and the lane carrying it.
struct Route {
    source: u32,
    target: u32,
    lane: u32,
}

fn push_route(
    pts: &mut Vec<f32>,
    route: Route,
    (drawing, rows): (&Drawing, &Rows),
    params: &LanesParams,
) {
    let (rs, rt) = (
        rows.row[route.source as usize],
        rows.row[route.target as usize],
    );
    let (early, late) = if rs < rt {
        (route.source, route.target)
    } else {
        (route.target, route.source)
    };
    let x = route.lane as f32 * params.lane_spacing;
    let mut points = [(0.0f32, 0.0f32); 2];
    let mut count = 0;
    if route.lane != drawing.lane[early as usize] {
        points[count] = (x, (rs.min(rt) as f32 + 0.5) * params.row_spacing);
        count += 1;
    }
    if route.lane != drawing.lane[late as usize] {
        points[count] = (x, (rs.max(rt) as f32 - 0.5) * params.row_spacing);
        count += 1;
    }
    let points = &mut points[..count];
    if early != route.source {
        points.reverse();
    }
    for &(px, py) in points.iter() {
        pts.push(px);
        pts.push(py);
    }
}
