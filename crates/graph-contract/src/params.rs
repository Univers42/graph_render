//! Layout parameters on the wire (`docs/decisions/layout-params.md`): the schema a layout
//! publishes, the buffer a caller sends back, and the one refusal rule both are read
//! through.
//!
//! The buffer is **positional**: one little-endian `f64` per published parameter, in
//! schema order, and nothing else — no header, no name table, no version byte. The
//! schema is the version: `gm_layout_params` publishes the list a buffer is read
//! against, so a caller and the motor it talks to cannot disagree about what index 3
//! means. A *count* change is caught by the length check; a rename or a reorder is not,
//! which is why `gm_abi_version` is the thing that catches a motor/SDK skew.
//!
//! `f64` throughout, including for the `Int` and `Bool` kinds, so the buffer is one type
//! rather than three. Every published integer is a `u32`, which `f64` holds exactly.

/// Width of one value in a run's parameter buffer.
pub const VALUE_BYTES: usize = 8;

/// What a parameter's value means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ParamKind {
    /// A whole number: the value must be integral.
    Int = 0,
    /// Any finite number in range.
    Float = 1,
    /// A flag: the value must be `0.0` or `1.0`.
    Bool = 2,
}

impl ParamKind {
    /// The wire tag, and the `u8` the schema buffer carries.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// The tag read back off the wire, or `None` for a tag no version publishes.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Int),
            1 => Some(Self::Float),
            2 => Some(Self::Bool),
            _ => None,
        }
    }
}

/// One parameter a layout publishes. Every field is required and none is optional, so a
/// layout cannot publish a knob a caller could not drive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    /// The key a caller sends: the parameter struct's own field name.
    pub name: &'static str,
    /// What its value means.
    pub kind: ParamKind,
    /// Inclusive lower bound. A value below it is **refused**, never clamped.
    pub min: f64,
    /// Inclusive upper bound, refused the same way.
    pub max: f64,
    /// The value a run with no buffer takes: bit for bit the field's `Default`.
    pub default: f64,
    /// The increment a control should offer. Never applied to a value.
    pub step: f64,
    /// One line, for a label and a tooltip.
    pub doc: &'static str,
}

/// Why a parameter buffer was refused. Three shapes, because a caller needs three
/// different answers: *I sent the wrong thing*, *this layout takes nothing*, and *that
/// value is not one of its parameters*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamsError {
    /// The layout publishes no parameters and the buffer was not empty.
    NotAccepted,
    /// The buffer is not exactly `specs.len() * 8` bytes, or the `(ptr, len)` pair is
    /// not a live allocation.
    Malformed,
    /// A value is not finite, not integral (an `Int`), not `0`/`1` (a `Bool`), or
    /// outside `[min, max]`. The index is into [`ParamsView::specs`].
    OutOfRange {
        /// Which published parameter.
        index: usize,
    },
}

impl ParamsError {
    /// The parameter this refusal is about, or `None` when it is about the buffer.
    pub fn name(self, specs: &[ParamSpec]) -> Option<&'static str> {
        match self {
            Self::OutOfRange { index } => specs.get(index).map(|spec| spec.name),
            Self::NotAccepted | Self::Malformed => None,
        }
    }
}

/// A layout's published parameters, and the rules for reading a buffer against them.
///
/// Every caller — the wasm export, a native run, the SDK's own encoder — goes through
/// [`validate`](Self::validate), so there is exactly one answer to "is this buffer
/// acceptable" in the tree.
#[derive(Debug, Clone, Copy)]
pub struct ParamsView<'a> {
    specs: &'a [ParamSpec],
}

impl<'a> ParamsView<'a> {
    /// The view over `specs`, in schema order.
    pub const fn new(specs: &'a [ParamSpec]) -> Self {
        Self { specs }
    }

    /// The published parameters, in the order a buffer carries them.
    pub const fn specs(&self) -> &'a [ParamSpec] {
        self.specs
    }

    /// How many parameters are published.
    pub const fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether this layout publishes nothing, which is an answer and not a refusal.
    pub const fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// The exact length of an acceptable buffer.
    pub const fn buffer_len(&self) -> usize {
        self.specs.len() * VALUE_BYTES
    }

    /// The little-endian `f64` at `index`, read one byte at a time: an 8-byte load at a
    /// 4-aligned address traps on wasm32 (`crates/graph-wasm/src/alloc.rs`), and a
    /// caller's `gm_alloc` pointer is 4-aligned.
    pub fn value(&self, bytes: &[u8], index: usize) -> f64 {
        let at = index * VALUE_BYTES;
        let mut word = [0u8; VALUE_BYTES];
        word.copy_from_slice(&bytes[at..at + VALUE_BYTES]);
        f64::from_le_bytes(word)
    }

    /// The published defaults, as the values a buffer with no opinion carries.
    pub fn defaults(&self) -> Vec<f64> {
        self.specs.iter().map(|spec| spec.default).collect()
    }

    /// The values a buffer carries, or why it was refused.
    pub fn values(&self, bytes: &[u8]) -> Result<Vec<f64>, ParamsError> {
        self.validate(bytes)?;
        Ok((0..self.len()).map(|i| self.value(bytes, i)).collect())
    }

    /// The one refusal rule. A non-finite value is refused before the range is read,
    /// because every comparison against `NaN` is false and a `NaN` would otherwise pass.
    pub fn validate(&self, bytes: &[u8]) -> Result<(), ParamsError> {
        if self.is_empty() && !bytes.is_empty() {
            return Err(ParamsError::NotAccepted);
        }
        if bytes.len() != self.buffer_len() {
            return Err(ParamsError::Malformed);
        }
        for (index, spec) in self.specs.iter().enumerate() {
            let value = self.value(bytes, index);
            if !value.is_finite() || value < spec.min || value > spec.max {
                return Err(ParamsError::OutOfRange { index });
            }
            if !kind_holds(spec.kind, value) {
                return Err(ParamsError::OutOfRange { index });
            }
        }
        Ok(())
    }

    /// This layout's schema as the wire body `gm_layout_params` publishes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.len() as u32).to_le_bytes());
        for spec in self.specs {
            put_str(&mut out, spec.name);
            out.push(spec.kind.tag());
            for value in [spec.min, spec.max, spec.default, spec.step] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            put_str(&mut out, spec.doc);
        }
        out
    }
}

/// Whether `value` is a legal reading of `kind`. `Int` must be whole and `Bool` must be
/// `0.0` or `1.0`; both are exact in `f64`, so neither can be rounded into agreement.
fn kind_holds(kind: ParamKind, value: f64) -> bool {
    match kind {
        ParamKind::Int => value.fract() == 0.0,
        ParamKind::Bool => value == 0.0 || value == 1.0,
        ParamKind::Float => true,
    }
}

fn put_str(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

#[cfg(test)]
mod tests;
