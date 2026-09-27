use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gm-fixtures-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn lines(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).expect("written");
    text.lines().map(str::to_owned).collect()
}

#[test]
fn every_case_has_one_expected_line_and_graph_definitions_expect_null() {
    let out = scratch("lines");
    let counts = emit(3, &out).expect("emits");
    let (cases, expect) = (
        lines(&out.join("cases.jsonl")),
        lines(&out.join("expect.jsonl")),
    );
    assert_eq!(cases.len(), expect.len());
    let mut graphs = 0;
    for (case, result) in cases.iter().zip(&expect) {
        let case: Value = serde_json::from_str(case).expect("json");
        if case["fn"] == "graph" {
            graphs += 1;
            assert_eq!(result, "null");
        } else {
            assert_ne!(result, "null", "{case}");
        }
    }
    assert_eq!(graphs, 6, "two graphs per seed");
    let total: u64 = counts.values().sum();
    assert_eq!(total as usize + graphs, cases.len());
    std::fs::remove_dir_all(&out).expect("cleanup");
}

#[test]
fn seed_0_reaches_all_17_functions_and_the_two_extra_arms() {
    let out = scratch("cover");
    let counts = emit(1, &out).expect("emits");
    let names: Vec<&str> = counts.keys().map(String::as_str).collect();
    assert_eq!(names.len(), 19, "{names:?}");
    for extra in ["hashString", "layoutGroups"] {
        assert!(names.contains(&extra), "{extra}");
    }
    let pairs = adversarial_pairs(&workspace_root().join(ADVERSARIAL)).expect("fixture");
    assert!(
        pairs.len() >= 3,
        "H1 needs at least the three measured pairs"
    );
    let tail = lines(&out.join("cases.jsonl"));
    let tail = &tail[tail.len() - 4 * pairs.len()..];
    for line in tail {
        let case: Value = serde_json::from_str(line).expect("json");
        assert_eq!(
            (&case["fn"], &case["seed"]),
            (&json!("makeEdgeId"), &Value::Null)
        );
    }
    std::fs::remove_dir_all(&out).expect("cleanup");
}

#[test]
fn emitting_twice_writes_the_same_bytes_and_a_manifest_that_pins_them() {
    let (one, two) = (scratch("one"), scratch("two"));
    emit(2, &one).expect("emits");
    emit(2, &two).expect("emits again");
    for file in ["cases.jsonl", "expect.jsonl", "manifest.json"] {
        assert_eq!(
            file_sha256(&one.join(file)),
            file_sha256(&two.join(file)),
            "{file}"
        );
    }
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(one.join("manifest.json")).expect("read"))
            .expect("json");
    assert_eq!(manifest["seeds"], 2);
    let expect = file_sha256(&one.join("expect.jsonl")).expect("hash");
    assert_eq!(manifest["sha256"]["expect.jsonl"], Value::from(expect));
    for dir in [one, two] {
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}

#[test]
fn nothing_to_emit_and_malformed_pairs_are_refused() {
    assert!(emit(0, &scratch("zero")).is_err());
    assert!(!scratch("zero").exists(), "a refused emit writes nothing");
    assert!(default_out().ends_with("target/oracle-fixtures"));
    let dir = scratch("pairs");
    std::fs::create_dir_all(&dir).expect("dir");
    let bad = dir.join("bad.json");
    std::fs::write(&bad, r#"{"pairs":[{"a":"x","b":1}]}"#).expect("write");
    assert!(adversarial_pairs(&bad).is_err());
    std::fs::write(&bad, r#"{"pair":[]}"#).expect("write");
    assert!(adversarial_pairs(&bad).is_err());
    std::fs::write(&bad, r#"{"pairs":[{"a":"x","b":"y"}]}"#).expect("write");
    assert_eq!(adversarial_pairs(&bad), Ok(vec![("x".into(), "y".into())]));
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn the_harness_verdict_passes_through_and_everything_else_is_could_not_run() {
    let exit = |code: i32| {
        std::process::Command::new("sh")
            .args(["-c", &format!("exit {code}")])
            .status()
            .map_err(|e| e.to_string())
    };
    assert_eq!(diff_code(exit(0)), 0);
    assert_eq!(diff_code(exit(1)), 1);
    assert_eq!(diff_code(exit(2)), 2);
    assert_eq!(diff_code(exit(3)), 2);
    assert_eq!(diff_code(Err("no node".into())), 2);
}
