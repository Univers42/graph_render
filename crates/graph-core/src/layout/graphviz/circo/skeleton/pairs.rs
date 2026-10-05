//! `find_pair_edges` and the degree top-up it asks for (`blockpath.c:102-177`).
//!
//! The pass runs two nested loops over `node`'s row: a neighbour that has an edge to another
//! of them is **paired** and counted once, and the unpaired ones are then joined back up so
//! `node`'s degree is unchanged. Split out of [`super`] only by the house's 40-line limit.

use super::Work;
#[cfg(test)]
use super::trace;

/// Drop the edges inside `node`'s neighbourhood, then pair up what is left.
pub(super) fn find_pair_edges(work: &mut Work, node: u32) {
    let node_degree = work.degree[node as usize];
    let mut edge_count = 0;
    let (with, without) = classify(work, node, &mut edge_count);
    #[cfg(test)]
    if trace::for_block(work.degree.len()) {
        trace::pairs(
            &trace::Pairing {
                node,
                degree: node_degree,
                count: edge_count,
            },
            (&with, &without),
        );
    }
    let diff = node_degree - 1 - edge_count;
    #[cfg(test)]
    if trace::for_block(work.degree.len()) {
        trace::diff(diff);
    }
    pair_up(work, &with, &without, diff);
}

/// `find_pair_edges`' two nested loops: every neighbour of `node` goes on `with` when it has
/// an edge to another of them and on `without` when it has not, and each such pair of
/// neighbours is counted once in `edge_count` — the reference's `(uintptr_t)n1 < n2`
/// (`blockpath.c:124`), which counts each pair under exactly one of its two orientations.
fn classify(work: &mut Work, node: u32, edge_count: &mut i32) -> (Vec<u32>, Vec<u32>) {
    let (mut with, mut without) = (Vec::new(), Vec::new());
    for (one, edge) in work.row(node) {
        let mut paired = false;
        for (two, other) in work.row(node) {
            if edge != other && work.find(one, two).is_some() {
                paired = true;
                if one < two {
                    *edge_count += 1;
                    drop_from_outg(work, one, two);
                }
            }
        }
        if paired {
            with.push(one)
        } else {
            without.push(one)
        }
    }
    (with, without)
}

/// `agdelete(outg, ORIGE(ex)); ORIGE(ex) = 0;` (`blockpath.c:126-129`) — dropped only
/// `if (ORIGE(ex))`, so a pair edge `pair_up` made is never taken out of `outg`. That guard
/// needs no flag here: a pair edge is born with `keep == false`, which is the same condition.
fn drop_from_outg(work: &mut Work, one: u32, two: u32) {
    let Some(found) = work.find(one, two) else {
        return;
    };
    #[cfg(test)]
    if work.keep[found as usize] && trace::for_block(work.degree.len()) {
        let (tail, head) = work.ends[found as usize];
        trace::deleted(tail, head);
    }
    work.keep[found as usize] = false;
}

/// The degree top-up: pair the unpaired neighbours off, two at a time.
fn pair_up(work: &mut Work, with: &[u32], without: &[u32], mut diff: i32) {
    if diff <= 0 {
        return;
    }
    if (diff as usize) < without.len() {
        let mut mark = 0;
        while mark + 1 < without.len() {
            work.link(without[mark], without[mark + 1]);
            diff -= 1;
            mark += 2;
        }
        for mark in 2..without.len() {
            if diff <= 0 {
                return;
            }
            work.link(without[0], without[mark]);
            diff -= 1;
        }
    } else if diff as usize == without.len() {
        fan_off_hub(work, with, without);
    }
}

/// `agedge(g, tp, hp, NULL, 1)` for every unpaired neighbour, with `tp` the **first** of
/// `with` — or `NULL` when `with` is empty, which makes an anonymous node the reference never
/// walks again (`blockpath.c:161-172`).
///
/// The reference raises `DEGREE` once on `hp` and once on `tp` when `tp` is not `NULL`, and
/// [`Work::link`] already does exactly that for a real hub, so the anonymous case is the only
/// one that may bump by hand. An extra `degree[hp] += 1` under a hub counts `hp` twice, and
/// `DEGREE` is what the next round's `LIST_SORT` orders on.
fn fan_off_hub(work: &mut Work, with: &[u32], without: &[u32]) {
    for &head in without {
        match with.first() {
            Some(&hub) => work.link(hub, head),
            None => work.degree[head as usize] += 1,
        }
    }
}
