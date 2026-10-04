//! `GRAPH_CORS_ORIGINS` (Verdict condition 12): the origins allowed on `/v1/`, compared byte for
//! byte against the request's `Origin` header. A wildcard is refused, so a misconfiguration fails
//! at start instead of opening `/v1/` to every origin.

use super::{ConfigError, Env, refuse};

/// The trimmed comma list, or a refusal naming the variable and never its value.
pub(super) fn read_origins(env: &Env<'_>) -> Result<Vec<String>, ConfigError> {
    let Some(list) = env.text("GRAPH_CORS_ORIGINS")? else {
        return Ok(Vec::new());
    };
    let origins: Vec<String> = list
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::to_owned)
        .collect();
    for origin in &origins {
        if !is_origin(origin) {
            return Err(refuse(
                "GRAPH_CORS_ORIGINS",
                "each entry is an http(s) origin with no path; a wildcard is refused",
            ));
        }
    }
    Ok(origins)
}

/// A serialized origin: a scheme, a host and an optional port, no path and no wildcard.
fn is_origin(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    rest.is_some_and(|host| {
        !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_graphic() && !matches!(b, b'/' | b'*' | b','))
    })
}
