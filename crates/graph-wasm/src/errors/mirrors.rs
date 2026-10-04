//! The wire's codes and its revision are written in three places that ship together:
//! `Code`, `Code::name` and `ABI_VERSION` here, the Errors table in
//! `docs/contract/wasm-abi.md`, and the SDK's `CODE_NAMES` and `ABI_VERSION`. A code
//! added to one and not the others is a caller reading a refusal under the wrong name, or
//! under none.
//!
//! Ponytail: a text scan, not a Markdown or TypeScript parser. Failing input: a
//! `CODE_NAMES` written with single quotes, or an Errors row not shaped
//! `| value | \`Name\` |`. Direction: it reads too few names and fails loud, never passes
//! a mismatch. Escape hatch: keep the two shapes, or teach the scan the new one.

use super::Code;

const ABI_DOC: &str = include_str!("../../../../docs/contract/wasm-abi.md");
const SDK_ERRORS: &str = include_str!("../../../graph-sdk-js/src/errors.ts");
const SDK_WASM: &str = include_str!("../../../graph-sdk-js/src/wasm.ts");

const ALL: [Code; 24] = [
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
    Code::ColumnsInvalid,
];

/// The `Name` cell of each `| value | \`Name\` |` row of the doc's Errors section.
fn documented_names() -> Vec<String> {
    let section = ABI_DOC
        .split("\n## Errors")
        .nth(1)
        .expect("an Errors section");
    let section = section.split("\n## ").next().unwrap_or(section);
    let rows = section
        .lines()
        .filter(|line| line.starts_with("| ") && line.contains('`'));
    rows.filter_map(|row| row.split('`').nth(1).map(str::to_owned))
        .collect()
}

/// The quoted strings of the SDK's `CODE_NAMES = [ ... ]`.
fn sdk_names() -> Vec<&'static str> {
    let start = SDK_ERRORS
        .find("CODE_NAMES = [")
        .expect("CODE_NAMES in errors.ts");
    let body = &SDK_ERRORS[start..];
    let body = &body[..body.find(']').expect("a closed CODE_NAMES")];
    body.split('"').skip(1).step_by(2).collect()
}

#[test]
fn every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order() {
    let names: Vec<&str> = ALL.iter().map(Code::name).collect();
    for (value, code) in ALL.iter().enumerate() {
        assert_eq!(*code as usize, value, "{code:?} is not at its wire value");
        assert_eq!(
            code.name(),
            format!("{code:?}"),
            "`Code::name` is not the variant's"
        );
    }
    assert_eq!(documented_names(), names, "wasm-abi.md's Errors table");
    assert_eq!(sdk_names(), names, "graph-sdk-js CODE_NAMES");
}

#[test]
fn the_sdk_and_the_doc_state_this_modules_abi_version() {
    let version = crate::ABI_VERSION;
    let sdk = format!("export const ABI_VERSION = {version};");
    assert!(SDK_WASM.contains(&sdk), "wasm.ts lacks `{sdk}`");
    let doc = format!("`gm_abi_version()` returns `{version}`");
    assert!(ABI_DOC.contains(&doc), "wasm-abi.md lacks {doc}");
}
