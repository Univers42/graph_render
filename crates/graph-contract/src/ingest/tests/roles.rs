//! The eight roles and the two cardinalities, pinned by name and in order.

use super::support::*;

// ---------------------------------------------------------------- the roles

#[test]
fn there_are_exactly_eight_roles_named_as_the_phase_prompt_spells_them() {
    let names: Vec<&str> = Role::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        names,
        [
            "title", "label", "group", "tags", "link", "scalar", "weight", "parent"
        ]
    );
    assert_eq!(Role::ALL.len(), 8);
}

#[test]
fn role_names_round_trip_and_an_unknown_name_is_refused() {
    for role in Role::ALL {
        assert_eq!(Role::from_name(role.as_str()), Some(role), "{role:?}");
    }
    assert_eq!(Role::from_name("Title"), None);
    assert_eq!(Role::from_name("multi_select"), None);
    assert_eq!(Role::from_name(""), None);
}

#[test]
fn every_role_name_is_a_distinct_lowercase_token() {
    let mut seen: Vec<&str> = Vec::new();
    for role in Role::ALL {
        let name = role.as_str();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase()),
            "{name} is not a lowercase token"
        );
        assert!(!seen.contains(&name), "{name} is declared twice");
        seen.push(name);
    }
}

#[test]
fn a_link_field_spellings_round_trip_and_unknown_is_none() {
    assert_eq!(Cardinality::from_name("one"), Some(Cardinality::One));
    assert_eq!(Cardinality::from_name("many"), Some(Cardinality::Many));
    assert_eq!(Cardinality::from_name("ONE"), None);
    assert_eq!(Cardinality::from_name(""), None);
    assert_eq!(Cardinality::Many.as_str(), "many");
    assert_eq!(Cardinality::One.as_str(), "one");
}
