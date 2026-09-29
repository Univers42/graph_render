//! The run: rounds, passes, and which edge rides which bundle. The mutable state of one
//! [`bundle`](super::bundle) call, and nothing else.

use super::P;
use super::Params;
use super::level::{self, Bundle};
use super::pass;

/// One run's mutable state: the current level, which bundle each edge rides, which way round
/// that bundle is to the edge, and the meeting points each edge has collected.
pub struct Run {
    level: Vec<Bundle>,
    chain: Vec<Vec<[P; 2]>>,
    owner: Vec<u32>,
    forward: Vec<bool>,
    merges: u32,
}

impl Run {
    /// Round 0: every edge alone, on its own source side.
    pub fn new(ends: &[(P, P)]) -> Self {
        let m = ends.len();
        Self {
            level: level::seed(ends),
            chain: vec![Vec::new(); m],
            owner: (0..m as u32).collect(),
            forward: vec![true; m],
            merges: 0,
        }
    }

    /// One round: passes until none merges, then the level's trunks. False when the round
    /// merged nothing, which ends the hierarchy — the next round would be handed exactly the
    /// level it just refused.
    pub fn round(&mut self, p: &Params, eps: f64) -> bool {
        let mut taken = 0;
        for _ in 0..super::PASSES_PER_ROUND {
            taken += self.one_pass(p, eps);
            if self.level.len() < 2 {
                break;
            }
        }
        if taken == 0 {
            return false;
        }
        self.record();
        self.level = self.level.iter().map(level::trunk).collect();
        true
    }

    /// One matching pass. 0 when it scored nothing that could be taken, which also ends the
    /// round: the next pass would score the same pairs the same way against the same level.
    fn one_pass(&mut self, p: &Params, eps: f64) -> u32 {
        if self.level.len() < 2 {
            return 0;
        }
        let cand = pass::candidates(&self.level, p.neighbors as usize);
        if cand.is_empty() {
            return 0;
        }
        let scores: Vec<pass::Score> = cand
            .iter()
            .map(|&(u, v)| pass::score(&self.level[u as usize], &self.level[v as usize], eps))
            .collect();
        let picks = pass::matching(&self.level, &cand, &scores, p.min_gain);
        if picks.is_empty() {
            return 0;
        }
        let (level, remap, rev) = pass::fused(&self.level, &cand, &picks, &scores);
        for (e, slot) in self.owner.iter_mut().enumerate() {
            self.forward[e] ^= rev[*slot as usize];
            *slot = remap[*slot as usize];
        }
        self.level = level;
        self.merges += picks.len() as u32;
        picks.len() as u32
    }

    /// Each edge records the meeting points of the bundle it now rides, oriented so its own
    /// source side comes first. A bundle that absorbed nobody records nothing: an edge that
    /// never merged draws straight, as it did before.
    fn record(&mut self) {
        for e in 0..self.owner.len() {
            let b = &self.level[self.owner[e] as usize];
            if b.head.pts.len() < 2 {
                continue;
            }
            let hop = if self.forward[e] {
                [b.p, b.q]
            } else {
                [b.q, b.p]
            };
            self.chain[e].push(hop);
        }
    }

    /// The meeting points each edge rode, one row per edge.
    pub fn chains(&self) -> Vec<Vec<[P; 2]>> {
        self.chain.clone()
    }

    /// Which bundle each edge rides.
    pub fn cluster(&self) -> Vec<u32> {
        self.owner.clone()
    }

    /// Levels each edge was bundled through: `0` for an edge that never merged.
    pub fn depths(&self) -> Vec<u32> {
        self.chain.iter().map(|hops| hops.len() as u32).collect()
    }

    /// Merges accepted, over every pass of every round.
    pub fn merges(&self) -> u32 {
        self.merges
    }
}
