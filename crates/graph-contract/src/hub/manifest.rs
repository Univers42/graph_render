//! The manifest: what a plugin declares about its collections before it sends a record.
//!
//! A manifest is the ingest document's *declaration* half, stored per plugin and
//! versioned. It is read through the ingest readers (`ingest/read.rs`'s `collection`,
//! `validate.rs`'s `check_title`) rather than a second set: the wire shape of a
//! collection is the ingest contract's, and a manifest that had its own copy of the
//! rules could disagree with the documents the records end up in.
//!
//! Four things happen here that the ingest reader does not do for a document, because a
//! manifest is a *live* declaration rather than a file:
//!
//! - **Two version members.** `version` is this wire format's version and is refused
//!   unless it is [`VERSION`]. `manifestVersion` is the client's own publication counter
//!   and lands in [`Manifest::version`], because that is what growth compares — a client
//!   that published a second manifest is not using a second *format*.
//! - **Qualified link targets.** A manifest may spell a target `coll` (same plugin) or
//!   `plugin.coll`; it is stored qualified, so two plugins can both declare a `task`
//!   collection and a document holding both records still names each one.
//! - **Caps.** A manifest is the one body whose *content* is capped by count, because a
//!   client can grow it forever and each version is stored.
//! - **Sorted.** Collections by id, fields by id, on the way in: the byte text of a
//!   manifest then depends on what it declares and not on the order a client wrote it,
//!   which is what makes `growth` a comparison of facts rather than of spellings.

use super::strict::parse_strict;
use super::{
    HubError, MAX_COLLECTIONS, MAX_FIELDS, MAX_MANIFEST_BYTES, VERSION, check_collection_id,
    qualify,
};
use crate::canonical_json::Value;
use crate::ingest::read::{array, member, object, require_only, text_of};
use crate::ingest::validate::check_title;
use crate::ingest::write::quoted;
use crate::ingest::{Collection, collection_piece};

mod growth;

/// The members a manifest names, exactly. An unknown member is refused: a client that
/// adds one to a version this contract does not know is asking for something this
/// reader cannot promise.
const MANIFEST_MEMBERS: [&str; 4] = ["collections", "manifestVersion", "name", "version"];

/// One plugin's declaration, at one version.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// The client's own version of this manifest. Growth compares these, so a client
    /// that bumps it is saying "something changed", not "something was added".
    pub version: u32,
    /// Human name, for diagnostics only: nothing in the motor derives from it, which is
    /// why renaming it between versions is growth and not a conflict.
    pub name: String,
    /// Its collections, sorted by id, each with its fields sorted by id, and each
    /// collection's link targets already qualified.
    pub collections: Vec<Collection>,
}

/// Reads one manifest body for `plugin`, or the refusal. `plugin` is needed only to
/// qualify the link targets a manifest spells bare; nothing else about it is read.
pub fn read_manifest(text: &str, plugin: &str) -> Result<Manifest, HubError> {
    let root = parse_strict(text, MAX_MANIFEST_BYTES, "manifest")?;
    let members = object_of(&root)?;
    require_only(members, &MANIFEST_MEMBERS, "").map_err(HubError::Shape)?;
    let wire = crate::ingest::read::integer(member_of(members, "version", "")?, "version")
        .map_err(HubError::Shape)?;
    if wire != VERSION {
        return Err(HubError::Invalid {
            path: "version".to_owned(),
            what: format!("unsupported hub manifest version {wire}"),
        });
    }
    let version = crate::ingest::read::integer(
        member_of(members, "manifestVersion", "")?,
        "manifestVersion",
    )
    .map_err(HubError::Shape)?;
    check_seq(version, "manifestVersion")?;
    let collections = collections(member_of(members, "collections", "")?, plugin)?;
    Ok(Manifest {
        version,
        name: text_of(member_of(members, "name", "")?, "name")
            .map_err(HubError::Shape)?
            .to_string(),
        collections,
    })
}

/// The manifest's canonical text: keys in byte order, no trailing newline (it is a
/// request body, not a document — `ingest::to_json` owns the newline). Round trips
/// through [`read_manifest`], which is the only proof a writer and a reader agree.
pub fn manifest_json(m: &Manifest) -> String {
    let collections = m
        .collections
        .iter()
        .map(collection_piece)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"collections\":[{collections}],\"manifestVersion\":{},\"name\":{},\"version\":{VERSION}}}",
        m.version,
        quoted(&m.name),
    )
}

/// The `collections` array read, capped, sorted and qualified. Every refusal below names
/// the index the bad collection has in the body the client sent, so it can find it.
///
/// Two passes, and the order is load-bearing: every collection is read and checked first,
/// and only then are the link targets resolved — a target may name a collection declared
/// *later* in the array, and refusing it for that reason would make the manifest's member
/// order a fact the contract enforced.
fn collections(value: &Value, plugin: &str) -> Result<Vec<Collection>, HubError> {
    let items = array(value, "collections").map_err(HubError::Shape)?;
    if items.len() as u64 > MAX_COLLECTIONS {
        return Err(HubError::TooLarge {
            what: "collections",
            limit: MAX_COLLECTIONS,
        });
    }
    let mut read: Vec<Collection> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let path = format!("collections[{i}]");
        let c = one_collection(item, &path)?;
        // The duplicate check needs the *earlier* ones in the body's own order, so it
        // cannot live in `one_collection`: the sort has not happened yet and the index in
        // the refusal must be the index the client can see.
        if read.iter().any(|other| other.id == c.id) {
            return Err(HubError::Invalid {
                path: format!("{path}.id"),
                what: format!("duplicate collection id `{}`", c.id),
            });
        }
        read.push(c);
    }
    let qualified: Vec<String> = read.iter().map(|c| qualify(plugin, &c.id)).collect();
    for (i, c) in read.iter_mut().enumerate() {
        qualify_links(c, &format!("collections[{i}]"), plugin, &qualified)?;
    }
    read.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(read)
}

/// One collection: the ingest reader's own rules (member names, field ids, the role and
/// target of a `link`, a `titleField` naming a title field), plus the manifest's two caps
/// and the collection-id grammar the ingest contract does not police.
fn one_collection(value: &Value, path: &str) -> Result<Collection, HubError> {
    let c = crate::ingest::read::collection(value, path).map_err(HubError::Shape)?;
    check_collection_id(&c.id).map_err(shift_id(path))?;
    if c.fields.len() as u64 > MAX_FIELDS {
        return Err(HubError::TooLarge {
            what: "fields",
            limit: MAX_FIELDS,
        });
    }
    check_field_ids(&c, path)?;
    check_title(&c, path).map_err(HubError::Shape)?;
    Ok(c)
}

/// A collection's field ids unique, before anything reads them by id. Checked here and
/// not in the field sort below, because the sort's tiebreak would be a decision about
/// which duplicate wins — and a duplicate is a refusal, never first-wins.
fn check_field_ids(c: &Collection, path: &str) -> Result<(), HubError> {
    for (i, field) in c.fields.iter().enumerate() {
        if c.fields[..i].iter().any(|other| other.id == field.id) {
            return Err(HubError::Invalid {
                path: format!("{path}.fields[{i}].id"),
                what: format!("duplicate field id `{}`", field.id),
            });
        }
    }
    Ok(())
}

/// Every link target qualified, one dot, and — the rule a document does not have —
/// naming a collection *this manifest* declares. A hub stores records by qualified id,
/// so a target that names nothing would leave every cell under that field dangling and
/// nothing in the output would say so. A target in another plugin is allowed and is not
/// checked here: whether that plugin has registered that collection is a question about
/// the *workspace*, and the model answers it (dropping the field and its cells).
fn qualify_links(
    c: &mut Collection,
    path: &str,
    plugin: &str,
    declared: &[String],
) -> Result<(), HubError> {
    for (i, field) in c.fields.iter_mut().enumerate() {
        let Some(link) = &mut field.link else {
            continue;
        };
        if link.collection.matches('.').count() > 1 {
            return Err(HubError::Invalid {
                path: format!("{path}.fields[{i}].link.collection"),
                what: format!("`{}` is not one qualified id", link.collection),
            });
        }
        // A bare target is this plugin's own collection, so this manifest must declare it:
        // a client pointing a field at a collection it did not declare is a mistake, and
        // every record already stored under that field would carry the mistake.
        if !link.collection.contains('.') {
            let target = qualify(plugin, &link.collection);
            if !declared.contains(&target) {
                return Err(HubError::Invalid {
                    path: format!("{path}.fields[{i}].link.collection"),
                    what: format!("link target `{target}` is not declared by this manifest"),
                });
            }
            link.collection = target;
        }
    }
    Ok(())
}

/// An id refusal lands on the collection's own path, so the message reads
/// `collections[1].id: collection id "B.coll" is not a legal id` instead of a bare
/// coordinate. The coordinate the id checker reports is kept inside `what`.
fn shift_id(path: &str) -> impl Fn(HubError) -> HubError + '_ {
    move |error| match error {
        HubError::Grammar { coordinate, value } => HubError::Invalid {
            path: format!("{path}.id"),
            what: format!("{coordinate} {value:?} is not a legal id"),
        },
        other => other,
    }
}

/// A `manifestVersion` past [`MAX_SEQ`](crate::hub::MAX_SEQ) would be rounded by a JSON consumer that reads
/// it as a double, so the counter a hub compares versions with has to survive the round
/// trip — the same bound a `seq` and a `rev` obey.
fn check_seq(version: u32, what: &'static str) -> Result<(), HubError> {
    if u64::from(version) > super::ids::MAX_SEQ {
        return Err(HubError::TooLarge {
            what,
            limit: super::ids::MAX_SEQ,
        });
    }
    Ok(())
}

/// [`crate::ingest::read::object`], as a `HubError`. One `map_err` per call site is
/// noise; there are four.
fn object_of(value: &Value) -> Result<&[(String, Value)], HubError> {
    object(value, "").map_err(HubError::Shape)
}

/// [`crate::ingest::read::member`], as a `HubError`.
fn member_of<'a>(
    members: &'a [(String, Value)],
    key: &str,
    path: &str,
) -> Result<&'a Value, HubError> {
    member(members, key, path).map_err(HubError::Shape)
}

pub use growth::{Growth, growth};
