//! Rows 7 to 18: the five `IGRAPH_` force layouts SciGraphs dispatches through its own
//! networkx and igraph helpers, then the seven closed-form rows that follow them
//! (`SPHERE` .. `BIPARTITE_3D`).
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.
//!
//! **Re-pinned 2026-10-04, `sg-igraph-3d`.** The five `IGRAPH_` rows move their **motor** sha
//! and nothing else, and the cause is two edits, both in this crate. (i) The reference helper
//! ends in `_igraph_fit_positions` (`igraph_layouts.py:24-42`), so the motor arm now applies the
//! same fit (`conformance/motor/fit.rs`) — a uniform translation and scale. (ii) `IGRAPH_FR`,
//! `IGRAPH_KK` and `IGRAPH_DRL` now name the `.3d` motor layouts, because SciGraphs calls all
//! three at `dim = 3` (`igraph_layouts.py:74`, `:99`, `:342`) and the 2-D siblings are not what
//! this reference runs. **The reference shas are unchanged in every row**, which is the
//! reproducibility result the seed correction rests on: two `--reference` runs over the same
//! fixtures gave byte-identical files on all 32 rows. `IGRAPH_DRL_2D` and `IGRAPH_LGL` keep
//! their planar motor layouts and moved only under (i); their motor shas are identical to the
//! ones the `sg-igraph-clean` branch pinned, which is the cross-check that this tree's 2-D
//! kernels are that branch's byte for byte. Medians in
//! `docs/measurements/scigraphs-conformance.md`.

use super::super::{Baseline, row};

pub(super) const NETWORKX: [Baseline; 12] = [
    row(
        "IGRAPH_FR",
        "86bce46cf87a8d476244d254929c9e2ca75964ff835061e4e494c92c29cd6678",
        "0cf3c05e67d79c08c152d0902dfe70dd5e3ef1cf4f9785448b394d24c8cfb170",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "IGRAPH_KK",
        "bbfac51b54ec26cb256e5ecfe3736803738ec0404fe994de7c99bada1d68128d",
        "a89c503e5fb39b8756fcbef3a6985ae6335770f874fd2e6cd06773bea5d0264a",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "IGRAPH_DRL",
        "9341c5093c1e7b9abaff88be524809aba7ed9da20bffe92cbfcf617909d54aa5",
        "19706b910225f374945f8e72c8594361dbb20d93e0716acaacec835c6ecf2b88",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "IGRAPH_DRL_2D",
        "0d43b8f201bfb7c14d21d4a52b1ac9efd6aaf83613ec01d5d8a9a683b5fcfc77",
        "79434cc8e4a271f57891b8170d454ca02a685d22f83a5ab1ed9606215a7b3fa0",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "IGRAPH_LGL",
        "41100b0d1dd4d83fa79eccd8db6f80f5768cdfb3c7a553f05ace0f661dacc49b",
        "a619ed3bc32e31f78056fbed6186352c5bf382457b40ffea5d0742645f71c3fd",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "SPHERE",
        "cef885fe29151fe8026438a0ebfb48515907c4635bb493b190398c84c2bbcd40",
        "14705b43ae52566f57a53dfa6c9e58dad9b4a265af49475dd0dc34f31fb5b929",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "SPECTRAL_3D",
        "8c8342bd944917806c777cab05678bdc2895a9c24a3b1a258db3d89181a32a02",
        "42af0e266a9b4a80596762308f6bdd38e5efaf07ad34d493ff8addf7a8f280a6",
        "",
        1e-15,
        "bitwise",
        "convention",
    ),
    row(
        "SPIRAL_3D",
        "57bf83dea6a23c75322523aae4fa02852c8e43b31726673d6685fd1ac7cfe41b",
        "94502f4418b5f7c71b0137e36b5cf1e29170c4c0c74e494219cfabba7818533f",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "HELIX",
        "d21691d1f552e74a2725c897618859249ccd66530ffa6980dfeaf6b2f2d594f4",
        "ea7fed73b64357f5e505488a99c1af7e5e8d2ecf1e1ce0c7dc206ada12649fb8",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "CUBE",
        "3b62357250ab90175272c1a4d43c270ab210f485b86f72301b59040e757bc02b",
        "2876776f43705602f42bf11b64fa165868e6f5a5078c682d5fee3a98454236ad",
        "",
        1e-16,
        "tolerance",
        "arithmetic",
    ),
    row(
        "HIERARCHICAL_3D",
        "235ca0cd6bd6c8e36c89797008c5033b2510f25e47422bc868298af3bc1b5955",
        "4df238a01dbf9a806d8871f406eb551dc551e04bc669066021bf086f2c3f97fd",
        "",
        1e-16,
        "tolerance",
        "arithmetic",
    ),
    row(
        "BIPARTITE_3D",
        "8451f75cc5300177dfe7d495e8dbf8ef67f80f96aad0488b8dd0514a64b7088b",
        "a1e5daacdaa148264e26ad2e20e2578e4f3159d49bd5afeeaa7c36db76da6dc5",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
];
