//! The two files `gpu-stress` reads: the `.gmfx` fixture's graph and start, and the GPU
//! tick's final `f32` positions. Every length is checked against the header before a read.

use std::path::Path;

/// The `.gmfx` magic, the same four bytes `gpu_fixtures::emit` writes.
pub(super) const MAGIC: [u8; 4] = *b"GMFX";

/// The `format major` this reader knows, the one `gpu_fixtures::emit` writes.
pub(super) const MAJOR: u32 = 1;

/// Header length in bytes: ten `u32` words, then three `f64`s.
const HEADER_LEN: usize = 64;

/// The graph and the start positions one `.gmfx` carries: the sections this command reads,
/// and nothing the mesh's own solve would fill in.
pub struct Fixture {
    /// Node count `n`.
    pub n: u32,
    /// Each simple edge's lower-index endpoint, in edge order.
    pub lo: Vec<u32>,
    /// Each simple edge's higher-index endpoint, in the same order.
    pub hi: Vec<u32>,
    /// The surviving raw edge's strength per simple edge: the link's distance and stiffness.
    pub strength: Vec<f64>,
    /// The session's own start `x` column.
    pub start_x: Vec<f64>,
    /// The session's own start `y` column.
    pub start_y: Vec<f64>,
}

impl Fixture {
    /// Reads and parses one `.gmfx`, refusing a wrong magic, an unknown major, or a
    /// truncated payload.
    pub fn read(path: &Path) -> Result<Fixture, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let n = header(&bytes, path)?;
        let m = u32_at(&bytes, 16);
        let need = HEADER_LEN + 8 + 8 * m as usize + 16 * n as usize;
        if bytes.len() < need {
            return Err(format!(
                "{}: the file holds {need} bytes of header, edges and positions, and the file is {} long",
                path.display(),
                bytes.len()
            ));
        }
        let mut at = HEADER_LEN + 8;
        let lo = u32s(&bytes, at, m, path)?;
        at += 4 * m as usize;
        let hi = u32s(&bytes, at, m, path)?;
        at += 4 * m as usize;
        let strength = f64s(&bytes, at, m, path)?;
        at += 8 * m as usize;
        let start_x = f64s(&bytes, at, n, path)?;
        at += 8 * n as usize;
        let start_y = f64s(&bytes, at, n, path)?;
        Ok(Fixture {
            n,
            lo,
            hi,
            strength,
            start_x,
            start_y,
        })
    }
}

/// The GPU tick's final positions: raw f32, x,y interleaved, 2n components.
pub struct Positions {
    /// Per-node `x`, narrowed to f32.
    pub x: Vec<f32>,
    /// Per-node `y`, narrowed to f32.
    pub y: Vec<f32>,
}

impl Positions {
    /// Reads `2n` f32 words as `n` interleaved `x,y` pairs, refusing a length that does
    /// not follow from the fixture's own `n`.
    pub fn read(path: &Path, n: u32) -> Result<Positions, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let want = 8 * n as usize;
        if bytes.len() != want {
            return Err(format!(
                "{}: the file holds {} bytes, and {n} nodes need {want}",
                path.display(),
                bytes.len()
            ));
        }
        let mut x = Vec::with_capacity(n as usize);
        let mut y = Vec::with_capacity(n as usize);
        for i in 0..n as usize {
            x.push(f32_at(&bytes, 8 * i));
            y.push(f32_at(&bytes, 8 * i + 4));
        }
        Ok(Positions { x, y })
    }
}

/// The header's own checks — magic, major — and the node count they carry.
fn header(bytes: &[u8], path: &Path) -> Result<u32, String> {
    if bytes.len() < HEADER_LEN {
        return Err(format!(
            "{}: the file is {} bytes, shorter than the {HEADER_LEN}-byte header",
            path.display(),
            bytes.len()
        ));
    }
    if bytes[0..4] != MAGIC {
        return Err(format!(
            "{}: the magic is not GMFX, so this is not a fixture",
            path.display()
        ));
    }
    let major = u32_at(bytes, 4);
    if major != MAJOR {
        return Err(format!(
            "{}: format major {major}, and this reader knows {MAJOR}",
            path.display()
        ));
    }
    Ok(u32_at(bytes, 12))
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

fn f64_at(bytes: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}

/// `count` `u32` words at `at`, read in wire order.
fn u32s(bytes: &[u8], at: usize, count: u32, path: &Path) -> Result<Vec<u32>, String> {
    let end = at + 4 * count as usize;
    if end > bytes.len() {
        return Err(format!(
            "{}: a u32 section ends at {end} and the file holds {} bytes",
            path.display(),
            bytes.len()
        ));
    }
    Ok((0..count)
        .map(|i| u32_at(bytes, at + 4 * i as usize))
        .collect())
}

/// `count` `f64` words at `at`, read in wire order.
fn f64s(bytes: &[u8], at: usize, count: u32, path: &Path) -> Result<Vec<f64>, String> {
    let end = at + 8 * count as usize;
    if end > bytes.len() {
        return Err(format!(
            "{}: an f64 section ends at {end} and the file holds {} bytes",
            path.display(),
            bytes.len()
        ));
    }
    Ok((0..count)
        .map(|i| f64_at(bytes, at + 8 * i as usize))
        .collect())
}
