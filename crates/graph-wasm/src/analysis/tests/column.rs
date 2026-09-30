//! Column kind and length tests.

#[test]
fn kind_names_the_element_type_of_both_column_kinds() {
    assert_eq!(
        crate::analysis::report::Column::F64(vec![0.5, 1.5]).kind(),
        "f64"
    );
    assert_eq!(
        crate::analysis::report::Column::U32(vec![0, 1]).kind(),
        "u32"
    );
    assert_eq!(
        crate::analysis::report::Column::F64(vec![0.5, 1.5]).len(),
        2
    );
    assert_eq!(crate::analysis::report::Column::U32(vec![0, 1, 2]).len(), 3);
    assert_eq!(crate::analysis::report::Column::U32(vec![]).len(), 0);
}
