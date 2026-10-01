//! Scratch diagnostic from p12-t4a, kept as a no-op because the sandbox denies `rm`.
//!
//! The question it answered — "how many LOBPCG iterations does the wider 3D block need,
//! and how many 2D seeds fail?" — is now a committed test:
//! `layout::spectral::tests::dims_3d::the_wider_lobpcg_block_converges_on_the_first_fixture_above_the_dense_limit`,
//! and the start-column regression is pinned in `linalg::lobpcg::tests`.

fn main() {}
