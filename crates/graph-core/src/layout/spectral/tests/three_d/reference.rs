//! SciGraphs' own numbers for `nx.path_graph(6)`, generated once in the pinned oracle image
//! as the parent module's header says, and split out of it for the house 300-line limit.

/// SciGraphs' `_spectral_component_coordinates(path6, 3)` — the peak-normalised
/// eigenvectors, before packing and before `_rescale_positions`. `_fix_eigenvector_signs`
/// is already applied.
pub(super) const PATH6_SPECTRAL_BLOCK: [[f64; 3]; 6] = [
    [
        bits(0xbfeffffffffffffb),
        bits(0xbfecb0bf0b6b7107),
        bits(0x3fe76cf5d0b09956),
    ],
    [
        bits(0xbfe76cf5d0b0994e),
        bits(0x3c6a146e07b6b62f),
        bits(0xbfe76cf5d0b0994d),
    ],
    [
        bits(0xbfd126145e9ecd46),
        bits(0x3fecb0bf0b6b7107),
        bits(0xbfe76cf5d0b09955),
    ],
    [
        bits(0x3fd126145e9ecd61),
        bits(0x3fecb0bf0b6b710a),
        bits(0x3fe76cf5d0b09952),
    ],
    [
        bits(0x3fe76cf5d0b09959),
        bits(0x3c615601adeed793),
        bits(0x3fe76cf5d0b0995a),
    ],
    [
        bits(0x3ff0000000000000),
        bits(0xbfecb0bf0b6b7106),
        bits(0xbfe76cf5d0b09951),
    ],
];

/// SciGraphs' `_spectral_layout_3d(path6, 5.0)`: the same block after `_pack_component_blocks`
/// (a no-op on one component) and `_rescale_positions(positions, 5.0)`.
pub(super) const PATH6_SPECTRAL: [[f64; 3]; 6] = [
    [
        bits(0xc014000000000000),
        bits(0xc011ee77672326a5),
        bits(0x400d483344dcbfa9),
    ],
    [
        bits(0xc00d483344dcbfa8),
        bits(0xbcb6977979761e33),
        bits(0xc00d483344dcbfa4),
    ],
    [
        bits(0xbff56f99764680a4),
        bits(0x4011ee77672326a4),
        bits(0xc00d483344dcbfae),
    ],
    [
        bits(0x3ff56f99764680ad),
        bits(0x4011ee77672326a6),
        bits(0x400d483344dcbfa4),
    ],
    [
        bits(0x400d483344dcbfa9),
        bits(0xbcb7f53a677d58fc),
        bits(0x400d483344dcbfae),
    ],
    [
        bits(0x4013fffffffffffd),
        bits(0xc011ee77672326a4),
        bits(0xc00d483344dcbfa9),
    ],
];

/// SciGraphs' `_mds_layout_3d(path6, 5.0)`.
pub(super) const PATH6_MDS: [[f64; 3]; 6] = [
    [
        bits(0x4014000000000000),
        bits(0x3c9f381712a51f48),
        bits(0x3cbbc3185f377f90),
    ],
    [
        bits(0x4008000000000000),
        bits(0x3c808a9add0a3056),
        bits(0xbc9e7779f73cd843),
    ],
    [
        bits(0x3ff0000000000000),
        bits(0x3c8fd7214a74dfbe),
        bits(0xbc7e12c14c73449b),
    ],
    [
        bits(0xbfeffffffffffffe),
        bits(0x3c9791d3dbefc794),
        bits(0xbc92fe15252cd4b6),
    ],
    [
        bits(0xc008000000000000),
        bits(0x3c9f381712a51f48),
        bits(0xbcac9eb29b5bc59c),
    ],
    [
        bits(0xc014000000000000),
        bits(0xbcbb8cb8053e638c),
        bits(0x3c695a194b0058ac),
    ],
];

const fn bits(raw: u64) -> f64 {
    f64::from_bits(raw)
}
