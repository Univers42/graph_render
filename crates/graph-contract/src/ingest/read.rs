//! The strict reader: the ingest document's wire text to [`Ingest`], or a refusal that
//! names the path. Every refusal is a [`IngestError`], never a defaulted value.

use super::{Cardinality, Collection, Field, Ingest, IngestError, JsonValue, Link, Record, Role};
use crate::canonical_json::{self, JsonError, Value};

mod cell;

pub(in crate::ingest) use cell::cell;

impl IngestError {
    /// A refusal, or the JSON fault underneath.
    pub(super) fn from_json(err: JsonError) -> Self {
        Self::Json(err)
    }
}

/// Reads one ingest document, or the refusal. Strict throughout: every member named and
/// required, an unknown member refused, no member given a default that changes what a graph
/// looks like. Parsing is one pass (`document`); everything needing the whole document —
/// duplicate ids, dangling references, the id grammar — is `super::validate`'s, run after.
pub fn read(text: &str) -> Result<Ingest, IngestError> {
    super::validate::check(document(text)?)
}

/// The parse, with no cross-document check: the reader and the canonical writer are two
/// directions of one format, held to that by `write_then_read_gives_back_the_same_document`.
fn document(text: &str) -> Result<Ingest, IngestError> {
    canonical_json::parse(text)
        .map_err(IngestError::from_json)
        .and_then(|value| {
            let members = object(&value, "")?;
            require_only(members, &DOCUMENT_MEMBERS, "")?;
            let version = integer(member(members, "version", "")?, "version")?;
            if version != super::VERSION {
                return Err(shape("version", format!("unsupported version {version}")));
            }
            let collections = array(member(members, "collections", "")?, "collections")?
                .iter()
                .enumerate()
                .map(|(i, v)| collection(v, &format!("collections[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let records = array(member(members, "records", "")?, "records")?
                .iter()
                .enumerate()
                .map(|(i, v)| record(v, &format!("records[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Ingest {
                version,
                source: text_of(member(members, "source", "")?, "")?.to_string(),
                collections,
                records,
            })
        })
}

/// The members each object names, exactly. Module-level rather than a `let` inside the
/// function that reads them (house limit: four parameters).
const DOCUMENT_MEMBERS: [&str; 4] = ["version", "source", "collections", "records"];
const COLLECTION_MEMBERS: [&str; 4] = ["id", "name", "titleField", "fields"];
const FIELD_MEMBERS: [&str; 4] = ["id", "name", "role", "link"];
const LINK_MEMBERS: [&str; 3] = ["collection", "cardinality", "symmetric"];
const RECORD_MEMBERS: [&str; 5] = ["id", "collection", "deleted", "updatedAt", "values"];

fn collection(value: &Value, path: &str) -> Result<Collection, IngestError> {
    let members = object(value, path)?;
    require_only(members, &COLLECTION_MEMBERS, path)?;
    let fields_path = format!("{path}.fields");
    let fields = array(member(members, "fields", path)?, &fields_path)?
        .iter()
        .enumerate()
        .map(|(i, v)| field(v, &format!("{fields_path}[{i}]")))
        .collect::<Result<Vec<_>, _>>()?;
    // Not sorted here: the per-collection checks in `validate` report the *document's*
    // index, which is what a reader with the file open is looking at; the sort follows.
    Ok(Collection {
        id: text_of(member(members, "id", path)?, &format!("{path}.id"))?.to_string(),
        name: text_of(member(members, "name", path)?, &format!("{path}.name"))?.to_string(),
        title_field: text_of(
            member(members, "titleField", path)?,
            &format!("{path}.titleField"),
        )?
        .to_string(),
        fields,
    })
}

fn field(value: &Value, path: &str) -> Result<Field, IngestError> {
    let members = object(value, path)?;
    require_only(members, &FIELD_MEMBERS, path)?;
    let role_path = format!("{path}.role");
    let role_name = text_of(member(members, "role", path)?, &role_path)?;
    let role = Role::from_name(role_name)
        .ok_or_else(|| shape(&role_path, format!("unknown role {role_name:?}")))?;
    // `link` is required on every field, as every member is (the module doc, the JSON
    // Schema's `required`, and `required_link` in `ingest/schema.rs` all say so). A present
    // `null` is how a non-`link` field says it has no target; omitting it is the mistake.
    let declared = member(members, "link", path)?;
    let link = match declared {
        Value::Null => None,
        value => Some(link(value, &format!("{path}.link"))?),
    };
    // Both directions are refused, and neither is defaulted: a `link` field with no target
    // would derive no edges, a `scalar` field with one a declaration nothing reads.
    match (role, &link) {
        (Role::Link, None) => {
            return Err(shape(path, "a `link` field must declare its `link` member"));
        }
        (Role::Link, Some(_)) => {}
        (_, Some(_)) => {
            return Err(shape(
                path,
                "only a `link` field may declare a `link` member",
            ));
        }
        (_, None) => {}
    }
    Ok(Field {
        id: text_of(member(members, "id", path)?, &format!("{path}.id"))?.to_string(),
        name: text_of(member(members, "name", path)?, &format!("{path}.name"))?.to_string(),
        role,
        link,
    })
}

fn link(value: &Value, path: &str) -> Result<Link, IngestError> {
    let members = object(value, path)?;
    require_only(members, &LINK_MEMBERS, path)?;
    let card_path = format!("{path}.cardinality");
    let card_name = text_of(member(members, "cardinality", path)?, &card_path)?;
    let cardinality = Cardinality::from_name(card_name)
        .ok_or_else(|| shape(&card_path, format!("unknown cardinality {card_name:?}")))?;
    Ok(Link {
        collection: text_of(
            member(members, "collection", path)?,
            &format!("{path}.collection"),
        )?
        .to_string(),
        cardinality,
        symmetric: boolean(
            member(members, "symmetric", path)?,
            &format!("{path}.symmetric"),
        )?,
    })
}

fn record(value: &Value, path: &str) -> Result<Record, IngestError> {
    let members = object(value, path)?;
    require_only(members, &RECORD_MEMBERS, path)?;
    // Sorted by key on the way in, so an `Ingest` is in canonical member order the moment
    // it exists: the H6 fix at the type level, a record's cells being a set keyed by field.
    let values_path = format!("{path}.values");
    let mut values: Vec<(String, JsonValue)> =
        object(member(members, "values", path)?, &values_path)?
            .iter()
            .map(|(key, value)| {
                cell(value, &format!("{values_path}.{key}")).map(|v| (key.clone(), v))
            })
            .collect::<Result<_, _>>()?;
    values.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Record {
        id: text_of(member(members, "id", path)?, &format!("{path}.id"))?.to_string(),
        collection: text_of(
            member(members, "collection", path)?,
            &format!("{path}.collection"),
        )?
        .to_string(),
        deleted: boolean(
            member(members, "deleted", path)?,
            &format!("{path}.deleted"),
        )?,
        updated_at: integer(
            member(members, "updatedAt", path)?,
            &format!("{path}.updatedAt"),
        )?,
        values,
    })
}

// ------------------------------------------------------------------ scalars

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a [(String, Value)], IngestError> {
    match value {
        Value::Object(members) => Ok(members),
        _ => Err(shape(path, "expected an object")),
    }
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], IngestError> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(shape(path, "expected an array")),
    }
}

fn member<'a>(
    members: &'a [(String, Value)],
    key: &str,
    path: &str,
) -> Result<&'a Value, IngestError> {
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| shape(path, format!("missing member `{key}`")))
}

/// Refuses a member the shape does not name: a stray camelCase becomes a loud refusal.
fn require_only(
    members: &[(String, Value)],
    allowed: &[&str],
    path: &str,
) -> Result<(), IngestError> {
    for (key, _) in members {
        if !allowed.contains(&key.as_str()) {
            return Err(shape(path, format!("unknown member `{key}`")));
        }
    }
    Ok(())
}

fn text_of<'a>(value: &'a Value, path: &str) -> Result<&'a str, IngestError> {
    match value {
        Value::String(text) => Ok(text),
        _ => Err(shape(path, "expected a string")),
    }
}

fn boolean(value: &Value, path: &str) -> Result<bool, IngestError> {
    match value {
        Value::Bool(b) => Ok(*b),
        _ => Err(shape(path, "expected a boolean")),
    }
}

/// A `u32` written as a plain non-negative integer literal, never `1.0` or `1e0` read as
/// `1` (D6: a version is compared, not rounded).
fn integer(value: &Value, path: &str) -> Result<u32, IngestError> {
    let Value::Number(text) = value else {
        return Err(shape(path, "expected a number"));
    };
    text.parse()
        .map_err(|_| shape(path, "expected a plain non-negative integer"))
}

fn shape(path: &str, what: impl Into<String>) -> IngestError {
    IngestError::Shape {
        path: path.to_string(),
        what: what.into(),
    }
}
