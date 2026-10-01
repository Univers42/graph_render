//! DrL's fixed schedule: which stage the run is in and the values that stage decays.

use super::{DrlParams, Phase};

const STAGES: usize = 5;
const CUT_DISABLED_ABOVE: f64 = 39_500.0;
const START_MIN_EDGES: f64 = 20.0;
const NO_CUTTING: f64 = 99.0;

pub(super) struct Schedule {
    phases: [Phase; 6],
    stage: usize,
    count: u32,
    temperature: f64,
    attraction: f64,
    damping_mult: f64,
    min_edges: f64,
    cut_length: f64,
    cut_end: f64,
    cut_rate: f64,
}

impl Schedule {
    pub(super) fn new(p: &DrlParams) -> Self {
        let cut_end = (40_000.0 * (1.0 - p.edge_cut)).max(1.0);
        let cut_start = 4.0 * cut_end;
        let init = p.phases[0];
        Self {
            phases: p.phases,
            stage: 0,
            count: init.iterations,
            temperature: init.temperature,
            attraction: init.attraction,
            damping_mult: init.damping_mult,
            min_edges: START_MIN_EDGES,
            cut_length: cut_start,
            cut_end,
            cut_rate: (cut_start - cut_end) / 400.0,
        }
    }

    pub(super) fn finished(&self) -> bool {
        self.stage >= STAGES
    }

    pub(super) fn fine(&self) -> bool {
        self.stage == 4
    }

    pub(super) fn cutting(&self) -> bool {
        self.stage < 4 && self.cut_end < CUT_DISABLED_ABOVE
    }

    pub(super) fn temperature(&self) -> f64 {
        self.temperature
    }

    pub(super) fn attraction(&self) -> f64 {
        self.attraction
    }

    pub(super) fn damping_mult(&self) -> f64 {
        self.damping_mult
    }

    pub(super) fn min_edges(&self) -> f64 {
        self.min_edges
    }

    pub(super) fn cut_length(&self) -> f64 {
        self.cut_length
    }

    /// `q(s)` of the current stage: `s^4` liquid, `s^2` expansion, `s` after.
    pub(super) fn pull(&self, s: f64) -> f64 {
        match self.stage {
            0 => (s * s) * (s * s),
            1 => s * s,
            _ => s,
        }
    }

    /// Ends a sweep: decay the stage's values, then enter the next stage when due.
    pub(super) fn advance(&mut self) {
        self.decay();
        self.count += 1;
        if self.stage == 0 && self.count == self.phases[0].iterations + 1 {
            self.load(1);
        }
        if self.count >= self.phases[self.stage + 1].iterations {
            self.stage += 1;
            self.count = 0;
            self.enter();
        }
    }

    fn load(&mut self, phase: usize) {
        let p = self.phases[phase];
        self.temperature = p.temperature;
        self.attraction = p.attraction;
        self.damping_mult = p.damping_mult;
    }

    fn enter(&mut self) {
        if self.finished() {
            return;
        }
        self.load(self.stage + 1);
        match self.stage {
            2 => self.min_edges = 12.0,
            3 => self.min_edges = 1.0,
            4 => self.min_edges = NO_CUTTING,
            _ => {}
        }
    }

    fn decay(&mut self) {
        match self.stage {
            1 => {
                lower(&mut self.attraction, 0.05, 1.0);
                lower(&mut self.min_edges, 0.05, 12.0);
                self.cut_length = (self.cut_length - self.cut_rate).max(self.cut_end);
                lower(&mut self.damping_mult, 0.005, 0.1);
            }
            2 => {
                lower(&mut self.temperature, 10.0, 50.0);
                lower(&mut self.min_edges, 0.2, 1.0);
                self.cut_length = (self.cut_length - 2.0 * self.cut_rate).max(self.cut_end);
            }
            3 => {
                self.cut_length = self.cut_end;
                self.min_edges = 1.0;
            }
            4 => lower(&mut self.temperature, 2.0, 50.0),
            _ => {}
        }
    }
}

/// `value -= step` while `value > floor`.
fn lower(value: &mut f64, step: f64, floor: f64) {
    if *value > floor {
        *value -= step;
    }
}
