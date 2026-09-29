//! The two closed enums, as the schema's `oneOf` of consts.
//!
//! Hand-written because `Role::as_str` and `Cardinality::as_str` are the authority:
//! a second hand-written list of names could drift from them, and a schema that
//! admits a role the reader refuses is a schema that validates a document the motor
//! then rejects.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::ingest::{Cardinality, Role};

// `Role` and `Cardinality` are the contract's own types, not a mirror's: they are
// already a closed set of names, and `as_str` is their authority, so a *serde* impl
// that goes through it cannot drift from the schema written by hand below. The impls
// live here, behind `codegen`, because the contract itself stays serializer-free.
//
// A role the wire text does not name is a **refusal**, not `null` and not a default:
// a wrong role silently changes the graph, so reading one is the one case where
// guessing is never the right answer (D9's discipline applied to an enum).
macro_rules! named_enum {
    ($ty:ty, $all:expr, $wire:literal) => {
        impl Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let name = String::deserialize(d)?;
                Self::from_name(&name).ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "unknown {} {name:?}, expected one of {:?}",
                        $wire,
                        $all.iter().map(|v| v.as_str()).collect::<Vec<_>>()
                    ))
                })
            }
        }
    };
}

named_enum!(Role, Role::ALL, "role");
named_enum!(Cardinality, Cardinality::ALL, "cardinality");

/// The eight declared roles, as the schema's `oneOf` of consts. Hand-written because
/// `Role::as_str` is the authority and a second hand-written list could drift from it.
impl JsonSchema for Role {
    fn schema_name() -> Cow<'static, str> {
        "Role".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        closed_enum(
            Role::ALL.iter().map(|role| role.as_str()),
            "What a declared field means. Declared, never inferred: a wrong role \
             silently changes the graph, so the explicit declaration is the escape \
             hatch and this contract offers no convenience inference on top of it.",
        )
    }
}

/// The two cardinalities, likewise.
impl JsonSchema for Cardinality {
    fn schema_name() -> Cow<'static, str> {
        "Cardinality".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        closed_enum(
            Cardinality::ALL.iter().map(|card| card.as_str()),
            "How many records a `link` field may name. Declared, not inferred: a \
             one-valued list and a single value are different schema shapes, and \
             reading one as the other would hide a mistake.",
        )
    }
}

fn closed_enum<'a>(names: impl Iterator<Item = &'a str>, description: &str) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "description": description,
        "oneOf": names
            .map(|name| serde_json::json!({ "const": name }))
            .collect::<Vec<_>>(),
    })
}
