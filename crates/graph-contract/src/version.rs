//! The snapshot format version, and the one rule every reader applies to it.
//!
//! Major/minor, after `SciGraphs/core/data_io/graph_animation.py:264-280`: a reader
//! **refuses** a major above the one it knows, naming both versions, rather than let an
//! older reader guess at a newer layout. A newer minor is read. A JSON document with no
//! version at all is read as [`UNVERSIONED`] (`0.0`) rather than refused, as that file
//! reads a file with no `format` row by the rules it has.
//!
//! `0.x` is pre-release: a minor may still change the payload. `0.1` was the Phase-0
//! stub (header plus bare columns), hashed but never persisted and never decoded. `0.2`
//! is the Phase-2 layout; `0.3` appends the notes section (`crate::notes`), which a 0.3
//! reader reads only from a snapshot labelled 0.3 or later — a 0.2 snapshot still reads,
//! as one with no notes. `0.4` adds the z column (`crate::snapshot::dim`), which a reader
//! reads only from a snapshot labelled 0.4 or later.
//!
//! Which minor a snapshot is *labelled* with is a separate question from which minor
//! this crate writes, and the label is the lowest that can express the snapshot — see
//! [`crate::snapshot::label_for`], the one function that decides it. A 2D snapshot is
//! labelled 0.3 and its bytes are unchanged by 0.4 existing; only 3D is labelled 0.4.
//! The byte layout this crate writes is `docs/contract/binary-layout.md`.

use core::fmt;

/// A snapshot format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema),
    serde(deny_unknown_fields)
)]
pub struct FormatVersion {
    /// Incremented on any change an older reader would misread.
    pub major: u32,
    /// Incremented on additive changes an older reader can safely ignore.
    pub minor: u32,
}

/// The version this crate writes, and the highest major it reads. Not the label every
/// snapshot carries: a 2D snapshot is labelled 0.3
/// ([`crate::snapshot::label_for`]), so no 2D byte moves when this moves.
pub const CURRENT_VERSION: FormatVersion = FormatVersion { major: 0, minor: 4 };

/// What a JSON document that carries no version is read as.
pub const UNVERSIONED: FormatVersion = FormatVersion { major: 0, minor: 0 };

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A snapshot written in a major this reader does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewerMajor {
    /// The version the snapshot declares.
    pub found: FormatVersion,
    /// The version this reader writes; its major is the highest it reads.
    pub known: FormatVersion,
}

impl fmt::Display for NewerMajor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "snapshot format {} is newer than this reader's {}: refusing rather than guessing at a newer layout",
            self.found, self.known
        )
    }
}

/// `Ok` when this reader can read a snapshot declaring `found`. The major alone decides:
/// within major 0 a minor is **not** a compatibility promise — `0.x` is pre-release, so a
/// newer minor may have moved the payload, and the only safe reading of an unknown `0.x`
/// minor is through this crate's own reader, never by assuming the bytes it writes.
pub const fn check_readable(found: FormatVersion) -> Result<(), NewerMajor> {
    if found.major > CURRENT_VERSION.major {
        return Err(NewerMajor {
            found,
            known: CURRENT_VERSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_refusal_names_both_versions_for_the_next_major() {
        let found = FormatVersion {
            major: CURRENT_VERSION.major + 1,
            minor: 0,
        };
        let err = check_readable(found).expect_err("a newer major");
        assert_eq!(
            err,
            NewerMajor {
                found,
                known: CURRENT_VERSION
            }
        );
        assert_eq!(
            err.to_string(),
            format!(
                "snapshot format 1.0 is newer than this reader's {CURRENT_VERSION}: \
                 refusing rather than guessing at a newer layout"
            )
        );
    }

    #[test]
    fn version_refusal_spares_this_major_at_any_minor() {
        for minor in [
            0,
            CURRENT_VERSION.minor,
            CURRENT_VERSION.minor + 1,
            u32::MAX,
        ] {
            let found = FormatVersion {
                major: CURRENT_VERSION.major,
                minor,
            };
            assert_eq!(check_readable(found), Ok(()), "{found}");
        }
        assert_eq!(check_readable(UNVERSIONED), Ok(()));
        assert_eq!(UNVERSIONED.to_string(), "0.0");
    }
}
