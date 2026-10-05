//! One answer and the requests that produce one: the fixture's own request builders and the
//! `oneshot` plumbing every test binary shares.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::Hub;

/// One answer.
pub struct Reply {
    /// The status line.
    pub status: StatusCode,
    /// The response head.
    pub headers: HeaderMap,
    /// The whole body.
    pub body: Bytes,
}

impl Reply {
    /// The status as a number, so an assertion reads `200` rather than a `StatusCode` name.
    pub fn code(&self) -> u16 {
        self.status.as_u16()
    }

    /// The body's `error` field.
    pub fn error(&self) -> String {
        self.field("error")
    }

    /// The body's `message` field.
    pub fn message(&self) -> String {
        self.field("message")
    }

    /// One field of a JSON body.
    fn field(&self, name: &str) -> String {
        let body: serde_json::Value = serde_json::from_slice(&self.body).expect("a JSON body");
        body[name].as_str().expect("a JSON string field").to_owned()
    }

    /// The whole body as text, for Review Focus 1's byte-for-byte refusal comparisons.
    pub fn body(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// A header as text, `""` when absent.
    pub fn header(&self, name: &str) -> &str {
        let value = self.headers.get(name);
        value.and_then(|value| value.to_str().ok()).unwrap_or("")
    }
}

impl Hub {
    /// Sends `request` and reads the whole answer.
    pub async fn send(&self, request: Request<Body>) -> Reply {
        send_to(self.router.clone(), request).await
    }

    /// [`Hub::send`] on its own task, so a test can hold one request open while another arrives.
    pub fn spawn(&self, request: Request<Body>) -> tokio::task::JoinHandle<Reply> {
        tokio::spawn(send_to(self.router.clone(), request))
    }

    /// `GET path`, with no credential: `/healthz` takes none.
    pub async fn get(&self, path: &str) -> Reply {
        self.send(
            Request::builder()
                .method("GET")
                .uri(path)
                .body(Body::empty())
                .expect("the test request"),
        )
        .await
    }

    /// `GET path` with this fixture's key.
    pub async fn get_with(&self, path: &str) -> Reply {
        self.send(
            self.request("GET", path)
                .body(Body::empty())
                .expect("the request"),
        )
        .await
    }

    /// `GET path` with `key`, which may be a key nobody minted or a name nobody holds.
    pub async fn get_as(&self, key: &str, path: &str) -> Reply {
        self.send(
            self.request_as(key, "GET", path)
                .body(Body::empty())
                .expect("the request"),
        )
        .await
    }

    /// `POST path` with this fixture's key and `body`.
    pub async fn post(&self, path: &str, body: impl Into<Body>) -> Reply {
        self.send(
            self.request("POST", path)
                .body(body.into())
                .expect("the request"),
        )
        .await
    }

    /// `PUT path` with this fixture's key and `body`.
    pub async fn put(&self, path: &str, body: impl Into<Body>) -> Reply {
        self.send(
            self.request("PUT", path)
                .body(body.into())
                .expect("the request"),
        )
        .await
    }

    /// `GET path` with no credential, as `GET path` with a header this fixture did not set.
    pub async fn get_anonymous(&self, path: &str) -> Reply {
        self.get(path).await
    }

    /// A request builder carrying this fixture's key. No body: the caller adds one when it has one.
    pub fn request(&self, method: &str, uri: &str) -> axum::http::request::Builder {
        self.request_as(&self.key, method, uri)
    }

    /// A request builder carrying `key` under `Authorization: Bearer`.
    pub fn request_as(&self, key: &str, method: &str, uri: &str) -> axum::http::request::Builder {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {key}"))
    }

    /// A request builder carrying no credential at all.
    pub fn anonymous(&self, method: &str, uri: &str) -> axum::http::request::Builder {
        Request::builder().method(method).uri(uri)
    }

    /// The log lines so far, parsed.
    pub fn lines(&self) -> Vec<serde_json::Value> {
        let lines = self.log.lock().unwrap();
        lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect()
    }
}

async fn send_to(router: Router, request: Request<Body>) -> Reply {
    let response = router.oneshot(request).await.expect("infallible");
    let (parts, body) = response.into_parts();
    let body = body.collect().await.expect("the response body").to_bytes();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body,
    }
}
