//! Every way a record can fail to back a row: another tree, too few seeds, a failed
//! run, a green control, an unexplained mismatch, an empty or duplicate field.

use super::*;

fn refused(edit: impl FnOnce(&mut Evidence)) -> Vec<String> {
    let mut evidence = honest();
    edit(&mut evidence);
    problems(&[row(Status::Gated)], &evidence)
}

#[test]
fn a_record_from_another_tree_or_too_few_seeds_is_refused() {
    let stale = refused(|e| e.fingerprint = "edited".into());
    assert_eq!(stale.len(), 2, "{stale:?}");
    assert!(stale[0].contains("from another tree"), "{stale:?}");
    let short = refused(|e| e.hashgate.as_mut().expect("set")["seeds"] = json!(MIN_SEEDS - 1));
    assert!(short[0].contains("ran 999 seeds, need 1000"), "{short:?}");
    let short = refused(|e| e.by_name.get_mut("oracle-diff").expect("set")["seeds"] = json!(999));
    assert!(short[0].contains("oracle-diff ran 999 seeds"), "{short:?}");
}

#[test]
fn a_failed_run_a_short_stage_or_a_green_control_is_refused() {
    let red = refused(|e| e.hashgate.as_mut().expect("set")["pass"] = json!(false));
    assert!(red[0].contains("hashgate did not pass"), "{red:?}");
    let stage = refused(|e| e.hashgate.as_mut().expect("set")["equal"]["topology"] = json!(999));
    assert!(
        stage[0].contains("stage topology not 4-way equal"),
        "{stage:?}"
    );
    let control = refused(|e| degree_control(e)["pass"] = json!(true));
    assert!(
        control[0].contains("hashgate-control-reference-degree did not go red;"),
        "{control:?}"
    );
    for (topology, why) in [(json!(8), "8 of 8 equal"), (json!(null), "no count")] {
        let blind = refused(|e| degree_control(e)["equal"]["topology"] = topology.clone());
        assert!(
            blind[0].contains("no negative control backs the topology stage"),
            "{why}: {blind:?}"
        );
        assert!(
            blind[0].contains("reference-degree did not go red on the topology stage"),
            "{why}: {blind:?}"
        );
    }
    let control = refused(|e| e.controls[0].1 = None);
    assert!(
        control[0].contains("no hashgate-control-reference-degree record"),
        "{control:?}"
    );
    let stale = refused(|e| degree_control(e)["fingerprint"] = json!("old"));
    assert!(stale[0].contains("from another tree"), "{stale:?}");
    let none = refused(|e| e.controls.clear());
    assert!(none[0].ends_with("no control is registered"), "{none:?}");
    let oracle = refused(|e| e.by_name.get_mut("oracle-diff").expect("set")["pass"] = json!(false));
    assert!(oracle[0].contains("oracle-diff did not pass"), "{oracle:?}");
}

fn degree_control(e: &mut Evidence) -> &mut Value {
    e.controls[0].1.as_mut().expect("set")
}

fn functions(e: &mut Evidence) -> &mut Value {
    &mut e.by_name.get_mut("oracle-diff").expect("set")["functions"]
}

/// `u64::MAX` is an accepted ceiling and every downstream `n <= ceiling` test is then
/// vacuously true — a ceiling that refuses nothing says nothing. The bound is the largest
/// ceiling this tree declares.
#[test]
fn a_scale_ceiling_above_the_largest_this_tree_declares_is_refused() {
    let mut greedy = row(Status::Implemented);
    greedy.scale_ceiling = u64::MAX;
    let found = problems(&[greedy], &honest());
    assert!(
        found
            .iter()
            .any(|p| p.contains(&format!("scale_ceiling {} is above", u64::MAX))),
        "{found:?}"
    );
    assert!(
        problems(&registry(), &honest())
            .iter()
            .all(|p| !p.contains("is above the largest one")),
        "and no declared ceiling is above it"
    );
}

/// The required-field sweep covers `stage`, `degradation`, `ponytail` and `complexity` —
/// **not `oracle`**. A row that names no reference and checks no function (see
/// `verdict::oracle_diff`) passed `--check`.
#[test]
fn a_row_that_names_no_oracle_is_refused() {
    let mut mute = row(Status::Implemented);
    mute.oracle = "";
    assert!(
        problems(&[mute], &honest())
            .iter()
            .any(|p| p.contains("required field `oracle` is empty")),
        "a row with no reference claims a differential it does not have"
    );
}

/// A geometry kind this ledger has no name for reads `unknown`, and `--check` says so. The
/// catch-all used to answer `"Curve"` for anything unrecognised, so a fourth kind added to
/// `EdgeGeometryKind` would have landed in the ledger as a confidently wrong value.
#[test]
fn a_row_whose_geometry_kind_this_ledger_cannot_name_is_refused() {
    let mut unnamed = row(Status::Implemented);
    unnamed.geometry = Some(UNKNOWN_GEOMETRY);
    assert!(
        problems(&[unnamed], &honest())
            .iter()
            .any(|p| p.contains("geometry kind `unknown`")),
        "an unnamed kind is a finding, not a borrowed name"
    );
    assert!(
        problems(&registry(), &honest())
            .iter()
            .all(|p| !p.contains("geometry kind `unknown`")),
        "and no published row carries one"
    );
}

/// The three refusals are asserted **by content**, not by count: a count still holds when
/// one check is silently replaced by another while the total stays three.
#[test]
fn the_required_field_refusals_are_named_and_not_merely_counted() {
    let mut bare = row(Status::Implemented);
    bare.ponytail = " ";
    bare.degradation = "";
    bare.scale_ceiling = 0;
    let found = problems(&[bare], &honest());
    for why in [
        "required field `ponytail` is empty",
        "required field `degradation` is empty",
        "scale_ceiling is 0",
    ] {
        assert!(
            found.iter().any(|p| p.contains(why)),
            "{why} is refused by name: {found:?}"
        );
    }
    assert_eq!(found.len(), 3, "and nothing else is: {found:?}");
}

#[test]
fn a_function_without_cases_or_with_an_unexplained_mismatch_is_refused() {
    let none = refused(|e| functions(e)["emptyModel"]["cases"] = json!(0));
    assert!(none[0].contains("ran no emptyModel case"), "{none:?}");
    let wrong = refused(|e| functions(e)["indexModel"]["unexplained"] = json!(1));
    assert!(wrong[0].contains("indexModel has unexplained"), "{wrong:?}");
    let mut evidence = honest();
    functions(&mut evidence)["layoutGroups"]["declared"] = json!(3);
    assert_eq!(
        find_row(&evidence, "topology.index").oracle_diff,
        "byte-equal/1000 seeds (25 cases, 3 declared divergences)"
    );
}

#[test]
fn empty_required_fields_zero_ceiling_and_duplicate_ids_are_refused() {
    let mut bare = row(Status::Implemented);
    bare.ponytail = " ";
    bare.degradation = "";
    bare.scale_ceiling = 0;
    let twice = [row(Status::Stub), row(Status::Stub)];
    assert_eq!(
        problems(&twice, &honest()),
        ["topology.index: duplicate id"]
    );
    assert_eq!(
        problems(&[bare], &honest()).len(),
        3,
        "the count, as well: the named form above pins which three"
    );
}

/// An empty `functions` is an **absent** differential, never a gated one. Emptied of
/// every oracle function the row is indistinguishable from an honest one to the loop in
/// `oracle_diff`: it runs zero times, `cases` stays 0, and the verdict used to be built
/// from no comparison at all.
#[test]
fn a_gated_row_that_names_no_oracle_function_is_refused() {
    let mut hollow = row(Status::Gated);
    hollow.functions = &[];
    let found = problems(&[hollow], &honest());
    assert!(
        found.iter().any(|p| p.contains("names no oracle function")),
        "an empty differential backs nothing: {found:?}"
    );
    // The ledger cell says the same thing rather than quoting a verdict with 0 cases.
    let honest = honest();
    let mut hollow = row(Status::Implemented);
    hollow.functions = &[];
    assert_eq!(
        verdict::oracle_diff(&honest, "oracle-diff", hollow.functions),
        Err("names no oracle function: an empty differential backs nothing".into())
    );
}
