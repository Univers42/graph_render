use super::*;

#[test]
fn last_write_wins_and_starts_at_none() {
    // Cleared explicitly: the thread-local is process-wide across tests.
    clear();
    assert_eq!(get(), Code::None as u32);
    set(Code::InvalidHandle);
    assert_eq!(get(), Code::InvalidHandle as u32);
    set(Code::AllocFailed);
    assert_eq!(get(), Code::AllocFailed as u32, "the more recent call wins");
    clear();
    assert_eq!(get(), 0, "clear is the same as recording None");
}

#[test]
fn every_code_is_distinct_and_none_is_the_wires_zero() {
    let codes = [
        Code::None,
        Code::InvalidHandle,
        Code::AllocFailed,
        Code::FreeRefused,
        Code::IngestInvalid,
        Code::UnknownLayoutId,
        Code::ParamsMustBeEmpty,
        Code::HandlesExhausted,
        Code::LayoutFailed,
        Code::TamperedGeometry,
        Code::NoGeometryYet,
        Code::BuildSourceInvalid,
        Code::IndexOutOfRange,
        Code::PostFailed,
        Code::ContractInvalid,
        Code::InvalidSession,
        Code::SessionParamsInvalid,
        Code::SessionRefused,
        Code::AnalysisFailed,
        Code::IngestTooLarge,
        Code::ParamOutOfRange,
        Code::ParamsMalformed,
        Code::ParamsNotAccepted,
    ];
    let mut values: Vec<u32> = codes.iter().map(|&c| c as u32).collect();
    values.sort_unstable();
    values.dedup();
    assert_eq!(
        values.len(),
        codes.len(),
        "every Code has its own wire value"
    );
    assert_eq!(
        Code::None as u32,
        0,
        "0 is both a data value and a code: ambiguous"
    );
}

/// Append-only, and this is what makes that a promise rather than a hope: a code
/// added in the middle would renumber every later one, and `CODE_NAMES` in
/// `crates/graph-sdk-js/src/errors.ts` indexes the same order — an ABI rename that
/// silently turned a caller's `UnknownLayoutId` into a `NoGeometryYet`.
#[test]
fn the_new_code_appends_and_does_not_move_any_other() {
    assert_eq!(
        [
            Code::None as u32,
            Code::InvalidHandle as u32,
            Code::AllocFailed as u32,
            Code::FreeRefused as u32,
            Code::IngestInvalid as u32,
            Code::UnknownLayoutId as u32,
            Code::ParamsMustBeEmpty as u32,
            Code::HandlesExhausted as u32,
            Code::LayoutFailed as u32,
            Code::TamperedGeometry as u32,
            Code::NoGeometryYet as u32,
            Code::BuildSourceInvalid as u32,
            Code::IndexOutOfRange as u32,
            Code::PostFailed as u32,
            Code::ContractInvalid as u32,
            Code::InvalidSession as u32,
            Code::SessionParamsInvalid as u32,
            Code::SessionRefused as u32,
            Code::AnalysisFailed as u32,
            Code::IngestTooLarge as u32,
            Code::ParamOutOfRange as u32,
            Code::ParamsMalformed as u32,
            Code::ParamsNotAccepted as u32,
        ],
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
        ],
        "every code keeps the wire value it already had"
    );
    assert_ne!(
        Code::InvalidSession as u32,
        Code::InvalidHandle as u32,
        "a dead session must not read as a dead graph handle, and the other way round"
    );
    assert_ne!(
        Code::ContractInvalid as u32,
        Code::IngestInvalid as u32,
        "a refused contract document must not read as a refused node/edge one"
    );
}
