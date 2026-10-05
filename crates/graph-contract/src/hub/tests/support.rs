//! The documents the hub tests share, so "the same manifest at two versions" is written
//! once. `manifest.rs` and `growth.rs` both need them and must not each hold a copy: two
//! copies of a fixture is two fixtures, and a growth test that grew from a different one
//! than the reader tested would prove nothing.

/// A two-collection manifest, written out of order on purpose: the reader sorts, so the
/// order a manifest was *written* in is not a fact the store keeps.
pub(super) const TWO: &str = r#"{
  "manifestVersion": 1,
  "name": "Tasks",
  "collections": [
    { "id": "note", "name": "Notes", "titleField": "title",
      "fields": [ { "id": "title", "name": "Title", "role": "title", "link": null } ] },
    { "id": "task", "name": "Tasks", "titleField": "name",
      "fields": [
        { "id": "name", "name": "Name", "role": "title", "link": null },
        { "id": "peer", "name": "Peer", "role": "link",
          "link": { "collection": "note", "cardinality": "one", "symmetric": false } },
        { "id": "up", "name": "Up", "role": "parent", "link": null }
      ] }
  ]
}"#;

/// The version member as it is written in [`TWO`].
pub(super) const BUMP: &str = r#""manifestVersion": 1"#;

/// [`TWO`] read: the manifest every growth rule is stated against.
pub(super) fn v1() -> super::super::Manifest {
    read("tracker", TWO)
}

/// [`TWO`] at `version`, read: the manifest a growth rule is tested against.
pub(super) fn at(version: u32, text: &str) -> super::super::Manifest {
    read(
        "tracker",
        &text.replace(BUMP, &format!(r#""manifestVersion": {version}"#)),
    )
}

/// One manifest read, panicking with its own text if it does not: the growth tests are
/// about the *rule*, not about the reader refusing a fixture.
pub(super) fn read(plugin: &str, text: &str) -> super::super::Manifest {
    super::super::manifest::read_manifest(text, plugin)
        .unwrap_or_else(|e| panic!("the fixture reads: {e}\n{text}"))
}