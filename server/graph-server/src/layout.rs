//! `POST /v1/layout`, in the order Verdict condition 4 fixes: a key in the query, the key, the
//! query and `Accept`, admission, and only then the body. A refusal at any step reads no byte
//! of the body. The build, the cap check, the run and the encoding happen on a blocking thread
//! that holds the request's slot (condition 5).

use crate::app::App;
use crate::error::ApiError;
use crate::observe::Facts;
use crate::query::{self, Face, LayoutQuery};
use crate::{auth, body, breaks, motor};
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::Instant;

/// The handler.
pub async fn layout(State(app): State<Arc<App>>, request: Request) -> Response {
    let mut facts = Facts::default();
    let response = serve(&app, request, &mut facts)
        .await
        .unwrap_or_else(IntoResponse::into_response);
    facts.attach(response)
}

/// One run's input.
struct Job {
    bytes: Bytes,
    ask: LayoutQuery,
    face: Face,
}

/// One run's outcome, with the graph's size once it was built (for the log line).
struct Ran {
    size: Option<(u32, u32)>,
    result: Result<Vec<u8>, ApiError>,
}

impl Ran {
    fn failed(refusal: ApiError) -> Self {
        Self {
            size: None,
            result: Err(refusal),
        }
    }
}

async fn serve(app: &Arc<App>, request: Request, facts: &mut Facts) -> Result<Response, ApiError> {
    let deadline = Instant::now() + app.limits.timeout;
    let (parts, body) = request.into_parts();
    let body = if breaks::on("body-before-auth") {
        Body::from(body::read(body, &parts.headers, &app.limits).await?)
    } else {
        body
    };
    let pairs = query::pairs(parts.uri.query())?;
    facts.key = auth::check(app, &parts.headers)?;
    let ask = query::layout(&pairs)?;
    let face = query::face(&parts.headers)?;
    facts.layout = Some(ask.layout.clone());
    facts.post.clone_from(&ask.post);
    let permit = app.gate.admit(deadline).await?;
    let bytes = body::read(body, &parts.headers, &app.limits).await?;
    let ran = compute(app, permit, Job { bytes, ask, face }, deadline).await;
    facts.size = ran.size;
    let snapshot = ran.result?;
    let headers = [
        (header::CONTENT_TYPE, face.media_type()),
        (header::VARY, "Accept"),
    ];
    Ok((headers, snapshot).into_response())
}

/// Runs `job` on a blocking thread until `deadline`. The permit moves into the closure: a run
/// past its deadline is answered 503 at once and keeps its slot until it ends, because a run
/// cannot be preempted (`docs/contract/service-api.md` "Limits and scheduling").
async fn compute(app: &Arc<App>, permit: OwnedSemaphorePermit, job: Job, deadline: Instant) -> Ran {
    let (inside, _outside) = if breaks::on("drop-permit") {
        (None, Some(permit))
    } else {
        (Some(permit), None)
    };
    let shared = Arc::clone(app);
    let run = tokio::task::spawn_blocking(move || {
        let _slot = inside;
        work(&shared, job)
    });
    match tokio::time::timeout_at(deadline, run).await {
        Ok(Ok(ran)) => ran,
        Ok(Err(failed)) if failed.is_panic() => Ran::failed(ApiError::internal("the run panicked")),
        Ok(Err(_)) => Ran::failed(ApiError::internal("the run was cancelled")),
        Err(_) => Ran::failed(ApiError::timeout()),
    }
}

/// Build, the cap check, the run and the encoding, on the blocking thread.
fn work(app: &App, job: Job) -> Ran {
    // The test seam, compiled only with the `test-hooks` feature (row `hooks-gated`).
    #[cfg(feature = "test-hooks")]
    if let Some(hook) = &app.hooks.before_run {
        hook();
    }
    let Job { bytes, ask, face } = job;
    let built = motor::build(&bytes, ask.source);
    drop(bytes);
    let topology = match built {
        Ok(topology) => topology,
        Err(code) => return Ran::failed(motor::refusal(code)),
    };
    let size = (topology.node_count(), topology.edge_count());
    let post = ask.post.as_deref();
    let result = app
        .caps
        .admit(&ask.layout, post, size)
        .and_then(|()| motor::run(&topology, &ask.layout, post).map_err(motor::refusal))
        .and_then(|snapshot| match face {
            Face::Binary => Ok(snapshot),
            Face::Json => motor::json_face(&snapshot),
        });
    Ran {
        size: Some(size),
        result,
    }
}
