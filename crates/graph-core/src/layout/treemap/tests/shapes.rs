//! Golden boxes for the ordinary shapes (a deep chain, a wide fan, an unbalanced tree and
//! two fixtures), produced by running d3-hierarchy@3.1.2 (`treemapSquarify`, size 1x1,
//! the call sequence `golden.rs` states) and compared with `f64::to_bits`.

use super::*;

/// CHAIN8: weights [1,2,0.5,3,1,4,1,2], edges (dense parent, child) [[0,1],[1,2],[2,3],[3,4],[4,5],[5,6],[6,7]].
const CHAIN8: [[u64; 4]; 8] = [
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fed_cb08_d3dc_b08d,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fed_cb08_d3dc_b08d,
        0x3feb_425e_d097_b426,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fec_7f6c_9e21_01eb,
        0x3feb_425e_d097_b425,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe4_b9c3_5bba_ea1f,
        0x3feb_425e_d097_b425,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe4_b9c3_5bba_ea1f,
        0x3fe7_da12_f684_bda0,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe4_b9c3_5bba_ea1f,
        0x3fd4_71c7_1c71_c71b,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fdb_a259_cfa3_e2d4,
        0x3fd4_71c7_1c71_c71b,
    ],
];

/// FAN10: weights [1,3,1,4,1,5,9,2,6,5,3], edges (dense parent, child) [[0,1],[0,2],[0,3],[0,4],[0,5],[0,6],[0,7],[0,8],[0,9],[0,10]].
const FAN10: [[u64; 4]; 11] = [
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x3fe0_0000_0000_0000,
        0x3fdc_cccc_cccc_cccd,
        0x3fe8_0000_0000_0000,
        0x3fe8_0000_0000_0000,
    ],
    [
        0x3fe6_6666_6666_6666,
        0x3fe8_0000_0000_0000,
        0x3fec_cccc_cccc_cccd,
        0x3fec_0000_0000_0000,
    ],
    [
        0x3fe8_e38e_38e3_8e39,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x3fdc_cccc_cccc_cccd,
    ],
    [
        0x3fe6_6666_6666_6666,
        0x3fec_0000_0000_0000,
        0x3fec_cccc_cccc_cccd,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fe8_0000_0000_0000,
        0x3fe0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe0_0000_0000_0000,
        0x3fdc_cccc_cccc_cccd,
    ],
    [
        0x3fe0_0000_0000_0000,
        0x3fe8_0000_0000_0000,
        0x3fe6_6666_6666_6666,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fdc_cccc_cccc_cccd,
        0x3fe0_0000_0000_0000,
        0x3fe8_0000_0000_0000,
    ],
    [
        0x3fe0_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe8_e38e_38e3_8e39,
        0x3fdc_cccc_cccc_cccd,
    ],
    [
        0x3fe8_0000_0000_0000,
        0x3fdc_cccc_cccc_cccd,
        0x3ff0_0000_0000_0000,
        0x3fe8_0000_0000_0000,
    ],
];

/// UNBALANCED: weights [1,2,1,5,1,3,1,2,4,1], edges (dense parent, child) [[0,1],[1,2],[2,3],[3,4],[4,5],[0,6],[6,7],[6,8],[6,9]].
const UNBALANCED: [[u64; 4]; 10] = [
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fee_79e7_9e79_e79e,
        0x3fe3_3333_3333_3334,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe9_6596_5965_9659,
        0x3fe3_3333_3333_3334,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe6_db6d_b6db_6db6,
        0x3fe3_3333_3333_3334,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fd4_5145_1451_4514,
        0x3fe3_3333_3333_3334,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fd4_5145_1451_4514,
        0x3fdc_cccc_cccc_ccce,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fe3_3333_3333_3334,
        0x3fee_79e7_9e79_e79e,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x3fde_79e7_9e79_e79e,
        0x3fe3_3333_3333_3334,
        0x3fea_aaaa_aaaa_aaaa,
        0x3feb_bbbb_bbbb_bbbc,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fe3_3333_3333_3334,
        0x3fde_79e7_9e79_e79e,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x3fde_79e7_9e79_e79e,
        0x3feb_bbbb_bbbb_bbbc,
        0x3fea_aaaa_aaaa_aaaa,
        0x3ff0_0000_0000_0000,
    ],
];

/// fixtures/hierarchy/TREE_BALANCED: boxes in the fixture's node order.
const TREE_BALANCED: [[u64; 4]; 15] = [
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fe0_d045_6c79_7dd5,
        0x3fee_f368_eb04_325c,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fee_f368_eb04_325c,
        0x3fe0_d045_6c79_7dd5,
    ],
    [
        0x3fe1_afa9_aadd_d3a2,
        0x3fe0_d045_6c79_7dd5,
        0x3fee_f368_eb04_325c,
        0x3fea_f017_2428_7f47,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fe0_d045_6c79_7dd5,
        0x3fe1_afa9_aadd_d3a2,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fdf_f300_f298_fa2d,
        0x3fe0_d045_6c79_7dd5,
    ],
    [
        0x3fdf_f300_f298_fa2d,
        0x0000_0000_0000_0000,
        0x3fee_f368_eb04_325c,
        0x3fd4_2d20_1bc4_fd66,
    ],
    [
        0x3fe1_afa9_aadd_d3a2,
        0x3fe7_9026_9198_d421,
        0x3feb_a279_1afa_9aae,
        0x3fea_f017_2428_7f47,
    ],
    [
        0x3fe1_afa9_aadd_d3a2,
        0x3fe0_d045_6c79_7dd5,
        0x3feb_a279_1afa_9aae,
        0x3fe7_9026_9198_d421,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fe0_d045_6c79_7dd5,
        0x3fd1_afa9_aadd_d3a2,
        0x3fec_3411_5b1e_5f75,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fec_3411_5b1e_5f75,
        0x3fd1_afa9_aadd_d3a2,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x3fd9_3868_22b6_3cc0,
        0x3fbf_f300_f298_fa2d,
        0x3fe0_d045_6c79_7dd5,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fdf_f300_f298_fa2d,
        0x3fd9_3868_22b6_3cc0,
    ],
    [
        0x3fdf_f300_f298_fa2d,
        0x0000_0000_0000_0000,
        0x3fe6_a176_012c_5be0,
        0x3fc4_2d20_1bc4_fd66,
    ],
    [
        0x3fdf_f300_f298_fa2d,
        0x3fc4_2d20_1bc4_fd66,
        0x3fe6_a176_012c_5be0,
        0x3fd4_2d20_1bc4_fd66,
    ],
];

/// fixtures/hierarchy/TREE_DEGENERATE: boxes in the fixture's node order.
const TREE_DEGENERATE: [[u64; 4]; 8] = [
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe3_54a3_1237_b53b,
        0x3fe5_9f22_49c8_9e6f,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe7_325d_490f_a646,
        0x3fe5_9f22_49c8_9e6f,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe8_29cb_d6c5_a289,
        0x3fe5_9f22_49c8_9e6f,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe8_29cb_d6c5_a289,
        0x3fec_8a60_75df_db5f,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe8_29cb_d6c5_a289,
        0x3fef_ffff_8beb_79d8,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe8_29cb_d6c5_a288,
        0x3fef_ffff_c5f5_bcec,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3fe8_29cb_d6c5_a288,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x0000_0000_0000_0000,
        0x0000_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
    ],
];

fn assert_boxes(topology: &Topology, want: &[[u64; 4]], label: &str) {
    let hierarchy = Hierarchy::of(topology).expect("fits");
    let boxes = compute(topology, &hierarchy);
    for (v, edges) in want.iter().enumerate() {
        let r = boxes.rect(v as u32);
        assert_eq!(
            [r.x0, r.y0, r.x1, r.y1].map(f64::to_bits),
            *edges,
            "{label}: node {v} differs from d3-hierarchy 3.1.2"
        );
    }
}

fn synthetic(weights: &[f64], pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| weighted(&format!("v{i}"), *w))
        .collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (p, c))| {
            tree(
                &format!("e{i}"),
                &format!("v{p}"),
                &format!("v{c}"),
                "parent_of",
            )
        })
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn a_deep_chain_matches_d3_bit_for_bit() {
    let pairs: Vec<_> = (0..7).map(|i| (i, i + 1)).collect();
    let t = synthetic(&[1.0, 2.0, 0.5, 3.0, 1.0, 4.0, 1.0, 2.0], &pairs);
    assert_boxes(&t, &CHAIN8, "chain8");
}

#[test]
fn a_ten_leaf_fan_matches_d3_bit_for_bit() {
    let pairs: Vec<_> = (1..=10).map(|i| (0, i)).collect();
    let w = [1.0, 3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0];
    assert_boxes(&synthetic(&w, &pairs), &FAN10, "fan10");
}

#[test]
fn an_unbalanced_tree_matches_d3_bit_for_bit() {
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (0, 6),
        (6, 7),
        (6, 8),
        (6, 9),
    ];
    let w = [1.0, 2.0, 1.0, 5.0, 1.0, 3.0, 1.0, 2.0, 4.0, 1.0];
    assert_boxes(&synthetic(&w, &pairs), &UNBALANCED, "unbalanced");
}

#[test]
fn the_balanced_fixture_matches_d3_bit_for_bit() {
    assert_boxes(
        &from_fixture("tree-balanced"),
        &TREE_BALANCED,
        "tree-balanced",
    );
}

#[test]
fn the_degenerate_fixture_matches_d3_bit_for_bit() {
    assert_boxes(
        &from_fixture("tree-degenerate"),
        &TREE_DEGENERATE,
        "tree-degenerate",
    );
}
