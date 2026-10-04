//! What every request goes through (Verdict condition 12): its `X-Request-Id`, the CORS headers
//! on `/v1/` for the configured origins only, the preflight, and one JSON log line on stdout,
//! the only place the service writes (`docs/contract/service-api.md` "The image": no disk
//! writes). The log line carries the key's name, never the key or its hash.

use crate::app::{App, LogSink};
use crate::{breaks, keys};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// The request id header, read and echoed.
pub const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
/// The longest caller id kept; a longer one is replaced.
/// Caveat: 128 is wider than any proxy's id and narrow enough that the id cannot be a place to hide
/// a key; a longer one is replaced by a generated id, not refused, so a caller with a long id still
/// gets a traceable answer and loses only the correlation.
const MAX_REQUEST_ID: usize = 128;
/// The preflight's answers.
const PREFLIGHT: [(HeaderName, &str); 3] = [
    (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST"),
    (
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "authorization, content-type, accept, x-request-id",
    ),
    (header::ACCESS_CONTROL_MAX_AGE, "600"),
];

/// What a handler learned, for the log line. It rides back on the response's extensions.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// The key's name, once the key was checked.
    pub key: Option<String>,
    /// The layout id asked for.
    pub layout: Option<String>,
    /// The POST pass asked for.
    pub post: Option<String>,
    /// Nodes and edges, once the graph was built.
    pub size: Option<(u32, u32)>,
}

impl Facts {
    /// `response`, carrying these facts to the log line.
    pub fn attach(self, mut response: Response) -> Response {
        response.extensions_mut().insert(self);
        response
    }
}

/// Generated request ids: a random per-process prefix and a counter, so two processes behind
/// one proxy do not hand out the same id.
#[derive(Debug)]
pub struct RequestIds {
    prefix: u64,
    next: AtomicU64,
}

impl RequestIds {
    /// A fresh prefix from `/dev/urandom`.
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self {
            prefix: u64::from_le_bytes(keys::random::<8>()?),
            next: AtomicU64::new(0),
        })
    }

    fn issue(&self) -> String {
        let count = self.next.fetch_add(1, Ordering::Relaxed);
        format!("{:016x}-{count:x}", self.prefix)
    }
}

/// The middleware around every route.
pub async fn observe(State(app): State<Arc<App>>, mut request: Request, next: Next) -> Response {
    let started = Instant::now();
    let headers = request.headers_mut();
    let id = request_id(&app.ids, headers);
    mark_sensitive(headers);
    let leaked = breaks::on("log-header").then(|| authorization(headers));
    let method = request.method().clone();
    let route = request.uri().path().to_owned();
    let origin = allowed_origin(&app.cors_origins, &route, request.headers());
    let mut response = match preflight(&method, request.headers(), origin.as_ref()) {
        Some(answer) => answer,
        None => next.run(request).await,
    };
    let facts = response
        .extensions_mut()
        .remove::<Facts>()
        .unwrap_or_default();
    decorate(response.headers_mut(), &id, origin);
    let line = Line {
        id: &id,
        method: method.as_str(),
        route: &route,
        facts,
        leaked,
    };
    app.log(&line.render(response.status(), started));
    response
}

/// The caller's id when it is 1-128 visible ASCII characters, a generated one otherwise.
fn request_id(ids: &RequestIds, headers: &HeaderMap) -> HeaderValue {
    let given = headers.get(&REQUEST_ID).filter(|value| {
        let bytes = value.as_bytes();
        (1..=MAX_REQUEST_ID).contains(&bytes.len()) && bytes.iter().all(u8::is_ascii_graphic)
    });
    match given {
        Some(value) => value.clone(),
        // A generated id is hex and `-` only, always a valid header value.
        None => HeaderValue::from_str(&ids.issue()).unwrap_or(HeaderValue::from_static("-")),
    }
}

/// Flags every `Authorization` value sensitive, so no layer below prints or indexes it.
fn mark_sensitive(headers: &mut HeaderMap) {
    if let header::Entry::Occupied(mut entry) = headers.entry(header::AUTHORIZATION) {
        for value in entry.iter_mut() {
            value.set_sensitive(true);
        }
    }
}

/// The negative control of row `svc-log`: the raw header, which must never reach a log line.
fn authorization(headers: &HeaderMap) -> String {
    let value = headers.get(header::AUTHORIZATION);
    value.and_then(|v| v.to_str().ok()).unwrap_or("").to_owned()
}

/// The request's `Origin` when the route is under `/v1/` and the origin is configured.
fn allowed_origin(origins: &[String], route: &str, headers: &HeaderMap) -> Option<HeaderValue> {
    let origin = headers.get(header::ORIGIN)?;
    let listed = origins.iter().any(|o| o.as_bytes() == origin.as_bytes());
    (route.starts_with("/v1/") && listed).then(|| origin.clone())
}

/// A CORS preflight from a configured origin: 204, no key needed. Any other `OPTIONS` goes on
/// to the router, which answers 404.
fn preflight(
    method: &Method,
    headers: &HeaderMap,
    origin: Option<&HeaderValue>,
) -> Option<Response> {
    let asks = headers.contains_key(header::ACCESS_CONTROL_REQUEST_METHOD);
    if method != Method::OPTIONS || !asks {
        return None;
    }
    origin?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    for (name, value) in PREFLIGHT {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    Some(response)
}

/// The id echoed on every response; the CORS headers on an allowed `/v1/` origin.
fn decorate(headers: &mut HeaderMap, id: &HeaderValue, origin: Option<HeaderValue>) {
    headers.insert(REQUEST_ID, id.clone());
    if let Some(origin) = origin {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.append(header::VARY, HeaderValue::from_static("origin"));
        let exposed = HeaderValue::from_static("x-request-id");
        headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, exposed);
    }
}

/// One request's log line.
struct Line<'a> {
    id: &'a HeaderValue,
    method: &'a str,
    route: &'a str,
    facts: Facts,
    leaked: Option<String>,
}

impl Line<'_> {
    fn render(self, status: StatusCode, started: Instant) -> serde_json::Value {
        let (n, m) = self.facts.size.unzip();
        let mut line = serde_json::json!({
            "event": "request",
            "id": self.id.to_str().unwrap_or("-"),
            "method": self.method,
            "key": self.facts.key,
            "route": self.route,
            "status": status.as_u16(),
            "ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "layout": self.facts.layout,
            "post": self.facts.post,
            "n": n,
            "m": m,
        });
        if let Some(raw) = self.leaked {
            line["authorization"] = raw.into();
        }
        line
    }
}

/// The binary's sink: one line on stdout, under the stdout lock so lines never interleave.
pub fn stdout_sink() -> LogSink {
    Arc::new(|line: &str| {
        // A closed stdout leaves nowhere to report the failure; the request is still served.
        let _ = writeln!(std::io::stdout().lock(), "{line}");
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Settings;
    use axum::Router;
    use axum::body::Body;
    use axum::routing::get;
    use std::ffi::OsString;
    use std::sync::atomic::AtomicBool;
    use tower::ServiceExt;

    /// `observe` flags the `Authorization` before the layer below can read, print or index it.
    /// The probe layer sits inside `observe`, so it sees the header exactly as a handler does:
    /// delete `mark_sensitive` from `observe` and this goes red.
    #[tokio::test]
    async fn the_authorization_reaching_the_handler_is_marked_sensitive() {
        let marked = Arc::new(AtomicBool::new(false));
        let probe = Arc::clone(&marked);
        let app = Arc::new(App::from_settings(&dev(), Arc::new(|_| {})).expect("the app"));
        let layer = axum::middleware::from_fn_with_state(Arc::clone(&app), super::observe);
        let router = Router::new()
            .route("/probe", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn(
                move |request: Request, next: Next| {
                    let probe = Arc::clone(&probe);
                    async move {
                        let value = request.headers().get(header::AUTHORIZATION);
                        probe.store(
                            value.is_some_and(HeaderValue::is_sensitive),
                            Ordering::SeqCst,
                        );
                        next.run(request).await
                    }
                },
            ))
            .layer(layer)
            .with_state(app);
        let request = Request::builder()
            .uri("/probe")
            .header(header::AUTHORIZATION, "Bearer gm_probe")
            .body(Body::empty())
            .expect("the request");
        let answered = router.oneshot(request).await.expect("the response");
        assert_eq!(answered.status(), StatusCode::OK);
        assert!(
            marked.load(Ordering::SeqCst),
            "the Authorization the handler sees is not marked sensitive"
        );
    }

    /// Auth off and one worker, so the state builds without a key file or a memory ceiling.
    fn dev() -> Settings {
        let vars = [("GRAPH_AUTH", "off"), ("GRAPH_WORKERS", "1")];
        Settings::from_env(&|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(*value))
        })
        .expect("the settings")
    }
}
