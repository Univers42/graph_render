//! Rows 7 to 18: the five `IGRAPH_` force layouts SciGraphs dispatches through its own
//! networkx and igraph helpers, then the seven closed-form rows that follow them
//! (`SPHERE` .. `BIPARTITE_3D`).
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.

use super::super::{Baseline, row};

pub(super) const NETWORKX: [Baseline; 12] = [
    // The five igraph rows carry `sg-igraph-dims`'s `_igraph_fit_positions` on the motor arm, and
    // `IGRAPH_FR` / `IGRAPH_KK` carry the `_3d` motor ids, because SciGraphs calls both at
    // `dim=3` (`igraph_layouts.py:74`, `:99`). The reference shas are unchanged in every row and
    // in every run — that is the reproducibility result: two `--reference` runs gave 64/64 files
    // byte-identical, and these digests are the ones pinned before the remap. A fit is a uniform
    // scale and a translation, so it moved the motor bytes and left the Procrustes medians at the
    // digit; the remap moved the medians (FR 0.267 -> 0.166, KK 0.812 -> 0.757).
    // `docs/measurements/sg-igraph-dims.md`.
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
        "122420ffd67d0e1c0b4d96176e349c7c32ac011f9296c559ae474d2ae21dbcd4",
        "a89c503e5fb39b8756fcbef3a6985ae6335770f874fd2e6cd06773bea5d0264a",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "IGRAPH_DRL",
        "0d43b8f201bfb7c14d21d4a52b1ac9efd6aaf83613ec01d5d8a9a683b5fcfc77",
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
        "70ac87fc4a9b0c6c717b6380bf4e51d1740b585a72464a7579051aaba81fe50c",
        "42af0e266a9b4a80596762308f6bdd38e5efaf07ad34d493ff8addf7a8f280a6",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "SPIRAL_3D",
        "1882ccdfd1d480b4ea3d30f3aa4fb95b9b42e20cde4113541f892cfff8f1090f",
        "94502f4418b5f7c71b0137e36b5cf1e29170c4c0c74e494219cfabba7818533f",
        "",
        1e0,
        "shape",
        "algorithm",
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
        "551445ae12399d0464ba8853c13c3c4404db0fe3afcf875a8f9c7ff713cf8826",
        "2876776f43705602f42bf11b64fa165868e6f5a5078c682d5fee3a98454236ad",
        "",
        1e0,
        "bitwise",
        "rng",
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
        "40e7daced9ccc174a6597e3c693c2207b76930c3250ce9d45cf4917f857c16a7",
        "a1e5daacdaa148264e26ad2e20e2578e4f3159d49bd5afeeaa7c36db76da6dc5",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
];
