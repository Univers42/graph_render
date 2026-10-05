use super::*;
use crate::exec::Serial;
use crate::layout::force::ForceParams;
use crate::layout::force::barnes_hut::step;
use crate::layout::force::particle_mesh::tests::placed;

/// A column with both signed zeros in it, so a sum that flips a zero's sign shows.
fn column(n: usize, f: f64) -> Vec<f64> {
    (0..n)
        .map(|i| match i % 97 {
            3 => -0.0,
            5 => 0.0,
            _ => libm::sin(i as f64 * f) * 40.0,
        })
        .collect()
}

/// Moving nodes, three of them pinned on one axis or both.
fn moving(n: usize) -> Sim {
    let mut sim = placed(column(n, 0.3), column(n, 0.7), ForceParams::default());
    (sim.vx, sim.vy) = (column(n, 1.1), column(n, 0.13));
    (sim.fx[4], sim.fy[9], sim.fx[9]) = (Some(12.5), Some(-3.0), Some(-0.0));
    sim
}

fn bits(sim: &Sim) -> [Vec<u64>; 4] {
    [&sim.x, &sim.y, &sim.vx, &sim.vy].map(|c| c.iter().map(|v| v.to_bits()).collect())
}

#[test]
fn the_passes_are_merge_and_integrate_bit_for_bit() {
    let n = 1000;
    let deltas: Vec<(f64, f64)> = column(n, 0.21).into_iter().zip(column(n, 0.017)).collect();
    let order: Vec<u32> = (0..n as u32).map(|k| (k * 389) % n as u32).collect();
    let mut slot = vec![0; n];
    for (k, &i) in order.iter().enumerate() {
        slot[i as usize] = k as u32;
    }
    for split in [false, true] {
        let mut want = moving(n);
        step::merge((&mut want.vx, &mut want.vy), Some(&order), &deltas, split);
        want.integrate();
        let gathered = Gathered {
            deltas: &deltas,
            slot: Some(&slot),
            split,
        };
        for workers in [1, 2, 3, 7] {
            let mut fused = moving(n);
            integrate(&mut fused, Some(gathered), (&Serial, workers));
            assert!(
                bits(&fused) == bits(&want),
                "fused, split {split}, workers {workers}"
            );
            let mut apart = moving(n);
            merge(&mut apart, gathered, (&Serial, workers));
            integrate(&mut apart, None, (&Serial, workers));
            assert!(
                bits(&apart) == bits(&want),
                "apart, split {split}, workers {workers}"
            );
        }
    }
}

/// The fused merge is the two merges run one after the other, to the bit: the same two
/// groupings in the same order, each with its own `split`.
#[test]
fn the_fused_merge_is_two_merges_bit_for_bit() {
    let n = 1000;
    let link: Vec<(f64, f64)> = column(n, 0.21).into_iter().zip(column(n, 0.017)).collect();
    let charge: Vec<(f64, f64)> = column(n, 0.53).into_iter().zip(column(n, 0.09)).collect();
    let order: Vec<u32> = (0..n as u32).map(|k| (k * 389) % n as u32).collect();
    let mut slot = vec![0; n];
    for (k, &i) in order.iter().enumerate() {
        slot[i as usize] = k as u32;
    }
    for link_split in [false, true] {
        for charge_split in [false, true] {
            let linked = Gathered {
                deltas: &link,
                slot: None,
                split: link_split,
            };
            let charged = Gathered {
                deltas: &charge,
                slot: Some(&slot),
                split: charge_split,
            };
            let mut want = moving(n);
            merge(&mut want, linked, (&Serial, 1));
            merge(&mut want, charged, (&Serial, 1));
            for workers in [1, 2, 3, 7] {
                let mut got = moving(n);
                merge_pair(&mut got, linked, charged, (&Serial, workers));
                assert!(
                    bits(&got) == bits(&want),
                    "link {link_split}, charge {charge_split}, workers {workers}"
                );
            }
        }
    }
}

#[test]
fn the_projection_is_position_plus_velocity() {
    let mut sim = moving(500);
    project(&mut sim, (&Serial, 3));
    for i in 0..500 {
        assert_eq!(sim.px[i].to_bits(), (sim.x[i] + sim.vx[i]).to_bits());
        assert_eq!(sim.py[i].to_bits(), (sim.y[i] + sim.vy[i]).to_bits());
    }
}

#[test]
fn a_node_ordered_merge_is_the_unordered_step_merge() {
    let n = 1000;
    let deltas: Vec<(f64, f64)> = column(n, 0.29).into_iter().zip(column(n, 0.031)).collect();
    for split in [false, true] {
        let mut want = moving(n);
        step::merge((&mut want.vx, &mut want.vy), None, &deltas, split);
        let linked = Gathered {
            deltas: &deltas,
            slot: None,
            split,
        };
        for workers in [1, 2, 3, 7] {
            let mut got = moving(n);
            merge(&mut got, linked, (&Serial, workers));
            assert!(
                bits(&got) == bits(&want),
                "split {split}, workers {workers}"
            );
        }
    }
}

#[test]
fn the_centering_pass_is_sim_center_bit_for_bit() {
    let mut want = moving(1000);
    want.params.center_strength = 0.7;
    want.center();
    for workers in [1, 2, 3, 7] {
        let mut got = moving(1000);
        got.params.center_strength = 0.7;
        center(&mut got, (&Serial, workers));
        assert!(bits(&got) == bits(&want), "workers {workers}");
    }
    let mut empty = placed(Vec::new(), Vec::new(), ForceParams::default());
    center(&mut empty, (&Serial, 2));
    assert!(empty.x.is_empty());
}
