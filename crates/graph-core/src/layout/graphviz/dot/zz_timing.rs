//! THROWAWAY timing probe. Delete before landing.
//!
//! Times `layout.dag.dot` on `fixtures/post/hairball.json` phase by phase, and `fdeb::run`
//! over its geometry separately.

use crate::index::index_model;
use crate::layout::coords::point_geometry;
use crate::layout::graphviz::dot::{build, mincross, position, rank};
use std::time::Instant;

fn hairball() -> crate::index::Topology {
    let (nodes, edges) = crate::post::fdeb::load("hairball").expect("the fixture is committed");
    index_model(&nodes, &edges).expect("the fixture indexes")
}

fn ids(t: &crate::index::Topology) -> Vec<String> {
    (0..t.node_count()).map(|i| t.node_id(i).to_string()).collect()
}

fn input_edges(t: &crate::index::Topology) -> Vec<(u32, u32)> {
    let e = t.edges();
    e.source
        .iter()
        .zip(e.target.iter())
        .filter(|&(s, h)| s != h)
        .map(|(&s, &h)| (s, h))
        .collect()
}

#[test]
#[ignore]
fn time_dot_on_the_hairball() {
    let t = hairball();
    let names = ids(&t);
    let b: Vec<&str> = names.iter().map(String::as_str).collect();
    let edges = input_edges(&t);
    eprintln!("hairball: {} nodes, {} edges", t.node_count(), edges.len());

    let mut g = build(&b, &edges);
    let t0 = Instant::now();
    rank(&mut g).expect("ranks");
    let d_rank = t0.elapsed();

    let t1 = Instant::now();
    mincross::run(&mut g);
    let d_mincross = t1.elapsed();

    let t2 = Instant::now();
    position(&mut g).expect("positions");
    let d_position = t2.elapsed();

    let total = d_rank + d_mincross + d_position;
    eprintln!("dot phases: rank {d_rank:?}, mincross {d_mincross:?}, position {d_position:?}");
    eprintln!("dot total: {total:?}");

    let (minx, maxx, miny, maxy) = extent(&g);
    eprintln!("dot extent: x [{minx:.3}, {maxx:.3}] ({:.3} pt), y [{miny:.3}, {maxy:.3}] ({:.3} pt)",
        maxx - minx, maxy - miny);

    let xs: Vec<f64> = (0..t.node_count()).map(|n| g.nodes[n as usize].coord.x).collect();
    let ys: Vec<f64> = (0..t.node_count()).map(|n| g.nodes[n as usize].coord.y).collect();
    let geometry = point_geometry(&xs, &ys);
    let t3 = Instant::now();
    let bundled = crate::post::fdeb::run(&t, &geometry).expect("fdeb runs");
    let d_fdeb = t3.elapsed();
    eprintln!("fdeb over dot's geometry: {d_fdeb:?} ({} pairs survive)", bundled.pairs);
}

fn extent(g: &crate::layout::graphviz::dot::fast::Fast) -> (f64, f64, f64, f64) {
    let mut lo = (f64::MAX, f64::MAX);
    let mut hi = (f64::MIN, f64::MIN);
    for n in &g.nodes {
        if n.kind != crate::layout::graphviz::dot::fast::Kind::Normal {
            continue;
        }
        lo.0 = lo.0.min(n.coord.x);
        hi.0 = hi.0.max(n.coord.x);
        lo.1 = lo.1.min(n.coord.y);
        hi.1 = hi.1.max(n.coord.y);
    }
    (lo.0, hi.0, lo.1, hi.1)
}

/// The same fixture over every OTHER registered layout, for the extent comparison that says
/// whether dot's coordinates are the outlier.
#[test]
#[ignore]
fn extent_of_every_layout_on_the_hairball() {
    let t = hairball();
    for layout in &crate::registry::LAYOUTS {
        let start = Instant::now();
        let geometry = (layout.run)(&t).expect("runs");
        let elapsed = start.elapsed();
        let (w, h) = geometry_extent(&geometry);
        eprintln!(
            "{:44} layout {:>12}  extent {w:>12.1} x {h:<12.1} pt",
            layout.id,
            format!("{elapsed:.3?}")
        );
    }
}

fn geometry_extent(g: &crate::layout::Geometry) -> (f64, f64) {
    use graph_contract::geometry::NodeGeometry as N;
    let (xs, ys): (Vec<f32>, Vec<f32>) = match &g.nodes {
        N::Point { x, y } => (x.clone(), y.clone()),
        N::Circle { x, y, .. } => (x.clone(), y.clone()),
        N::Box { x, y, .. } => (x.clone(), y.clone()),
    };
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let w = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
    let h = ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min);
    (f64::from(w), f64::from(h))
}
/// The position pass, one step at a time, so the 1757 s has a name on it.
#[test]
#[ignore]
fn time_the_position_steps_on_the_hairball() {
    use crate::layout::graphviz::dot::position::{Rows, aux, frame, xcoords, ycoords};
    use crate::layout::graphviz::dot::simplex::{self, Params};
    let t = hairball();
    let names = ids(&t);
    let b: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut g = build(&b, &input_edges(&t));
    rank(&mut g).expect("ranks");
    mincross::run(&mut g);
    eprintln!("nodes after class2: {}", g.nodes.len());
    eprintln!("edges after class2: {}", g.edges.len());

    let t0 = Instant::now();
    let rows = Rows::of(&g);
    eprintln!("  rows::of           {:?}", t0.elapsed());

    let t1 = Instant::now();
    ycoords::run(&mut g, &rows);
    eprintln!("  ycoords::run       {:?}", t1.elapsed());

    let t2 = Instant::now();
    let a = aux::build(&mut g, &rows);
    let d_aux = t2.elapsed();
    eprintln!("  aux::build         {d_aux:?}  ({} slack nodes, {} constraints + {} pairs)",
        a.slack().len(), a.constraints().len(), a.pairs().len());

    let nlist = a.node_list();
    eprintln!("  node_list len      {}", nlist.len());

    let t3 = Instant::now();
    let r = simplex::rank2(&mut g, &nlist, &Params::left_right());
    eprintln!("  simplex::rank2     {:?}  ({:?})", t3.elapsed(), r);

    let t4 = Instant::now();
    xcoords::run(&mut g, &rows);
    eprintln!("  xcoords::run       {:?}", t4.elapsed());
    a.remove(&mut g);

    let t5 = Instant::now();
    frame::run(&mut g, &rows);
    eprintln!("  frame::run         {:?}", t5.elapsed());
}

/// The same, with the `cfg(test)` invariant re-derivation compiled out: that is what
/// `cargo build`/`cargo run --release` sees.
#[test]
#[ignore]
fn time_the_position_steps_on_the_hairball_without_the_checks() {
    use crate::layout::graphviz::dot::position::{Rows, aux, frame, xcoords, ycoords};
    use crate::layout::graphviz::dot::simplex::{Balance, Params};
    let t = hairball();
    let names = ids(&t);
    let b: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut g = build(&b, &input_edges(&t));
    rank(&mut g).expect("ranks");
    mincross::run(&mut g);
    let rows = Rows::of(&g);
    ycoords::run(&mut g, &rows);
    let a = aux::build(&mut g, &rows);
    let nlist = a.node_list();
    // Same engine, same Params, but the per-pivot `checks::check` is `#[cfg(test)]` and is
    // therefore compiled out of this measurement by construction.
    let t3 = Instant::now();
    let params = Params {
        balance: Balance::LeftRight,
        maxiter: i32::MAX,
        search_size: crate::layout::graphviz::dot::simplex::SEARCH_SIZE,
    };
    let r = crate::layout::graphviz::dot::simplex::rank2(&mut g, &nlist, &params);
    eprintln!("  simplex::rank2 (test-cfg) {:?}  ({:?})", t3.elapsed(), r);
    xcoords::run(&mut g, &rows);
    a.remove(&mut g);
    frame::run(&mut g, &rows);
}

/// The pivot count and the cost of one pivot, by capping `maxiter` and differencing.
#[test]
#[ignore]
fn count_pivots_in_the_x_simplex() {
    use crate::layout::graphviz::dot::position::{Rows, aux, ycoords};
    use crate::layout::graphviz::dot::simplex::{Balance, Params};
    let t = hairball();
    let names = ids(&t);
    let b: Vec<&str> = names.iter().map(String::as_str).collect();
    let edges = input_edges(&t);
    eprintln!("hairball: {} nodes {} edges", t.node_count(), edges.len());
    for cap in [0, 1, 2, 5, 20, 100] {
        let mut g = build(&b, &edges);
        rank(&mut g).expect("ranks");
        mincross::run(&mut g);
        let rows = Rows::of(&g);
        ycoords::run(&mut g, &rows);
        let a = aux::build(&mut g, &rows);
        let nlist = a.node_list();
        let params = Params { balance: Balance::LeftRight, maxiter: cap, search_size: 30 };
        let start = Instant::now();
        let r = crate::layout::graphviz::dot::simplex::rank2(&mut g, &nlist, &params);
        eprintln!(
            "  maxiter {cap:>4}: {:?} ({:?})  nodes={} edges={}",
            start.elapsed(), r, nlist.len(), g.edges.len()
        );
    }
}
