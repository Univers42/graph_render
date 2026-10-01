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

#[test]
fn a_function_without_cases_or_with_an_unexplained_mismatch_is_refused() {
    let none = refused(|e| functions(e)["emptyModel"]["cases"] = json!(0));
    assert!(none[0].contains("ran no emptyModel case"), "{none:?}");
    let wrong = refused(|e| functions(e)["indexModel"]["unexplained"] = json!(1));
    assert!(wrong[0].contains("indexModel has unexplained"), "{wrong:?}");
    let mut evidence = honest();
    functions(&mut evidence)["layoutGroups"]["declared"] = json!(3);
    assert_eq!(
        ledger(&evidence)[0].oracle_diff,
        "byte-equal/1000 seeds (25 cases, 3 declared divergences)"
    );
}

#[test]
fn empty_required_fields_zero_ceiling_and_duplicate_ids_are_refused() {
    let mut bare = row(Status::Implemented);
    bare.ponytail = " ";
    bare.degradation = "";
    bare.scale_ceiling = 0;
    assert_eq!(problems(&[bare], &honest()).len(), 3);
    let twice = [row(Status::Stub), row(Status::Stub)];
    assert_eq!(
        problems(&twice, &honest()),
        ["topology.index: duplicate id"]
    );
}
