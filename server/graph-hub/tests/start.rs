//! §6's start checks: the limits as `config::Settings` reads them, and the seven refusals.
//!
//! Two files, because they need different things: [`settings`] reads no database and could run in
//! the no-database floor, while [`refusals`] reads the image's own PostgreSQL. Both sit behind
//! `db-tests` because they share this one test target and condition (c) requires that no test which
//! needs a connection runs under a plain `cargo test`.
//!
//! Every refusal goes through `Settings::check` and never through the binary: a refusal only `main`
//! can produce cannot be told apart from one `main` reports for the wrong reason.
#![cfg(feature = "db-tests")]

#[path = "start/refusals.rs"]
mod refusals;
#[path = "start/settings.rs"]
mod settings;
mod support;

use std::collections::BTreeMap;

use graph_hub::config::Settings;

/// The settings `env` describes: §6's defaults, a scratch credential pair so nothing here needs a
/// database to name, and `env` on top.
///
/// The credential files are written at 0640 because both loaders refuse anything wider, and nothing
/// reads them here: only `Settings::from_env` runs, and it reads names.
pub(crate) fn settings(env: &[(&str, &str)]) -> Result<Settings, graph_hub::config::ConfigError> {
    let dir = support::scratch();
    let keys = support::write_private(&dir.join("keys"), "tester 0000\n");
    let grants = support::write_private(&dir.join("grants"), "tester * admin\n");
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    vars.insert("GRAPH_HUB_KEYS_FILE".into(), keys.display().to_string());
    vars.insert("GRAPH_HUB_GRANTS_FILE".into(), grants.display().to_string());
    vars.insert(
        "GRAPH_HUB_DB_URL".into(),
        String::from("postgres://hub:hub@127.0.0.1:5432/hub"),
    );
    for (name, value) in env {
        vars.insert((*name).to_owned(), (*value).to_owned());
    }
    let lookup = |name: &str| vars.get(name).map(Into::into);
    Settings::from_env(&lookup)
}
