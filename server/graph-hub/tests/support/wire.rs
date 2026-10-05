//! Requests to the hub **container** over TCP, for the cases that must outlive a process.
//!
//! `oneshot` drives a router inside the test process, and a container kill cannot reach that. The
//! container `scripts/orch/hub-run.sh` started writes its base URL to `target/hub-run/url` and the
//! plaintext of its one key to `target/hub-run/key`; both are read here on every request, because a
//! restarted container may get a new bridge IP. The key is sent and never printed.

use axum::body::Bytes;
use axum::http::{Request, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::path::PathBuf;
use std::time::Duration;

use super::Reply;
use super::fixtures::MANIFEST;

/// How long one request may take. Caveat: a guess above one batch on a loaded host; a request to
/// a killed container's released bridge IP can wait for the ARP timeout, and this bounds that wait
/// so a writer ends on a transport error instead of hanging.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The hub container `hub-run.sh` started.
pub struct Remote {
    client: Client<HttpConnector, Full<Bytes>>,
    timeout: Duration,
}

impl Remote {
    /// A client with no pool: a kill must not leave a writer holding a dead keep-alive socket.
    pub fn new() -> Self {
        let client = Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(0)
            .build_http();
        Self {
            client,
            timeout: REQUEST_TIMEOUT,
        }
    }

    /// The same client with `timeout` per request instead of [`REQUEST_TIMEOUT`], for a case whose
    /// requests wait on each other inside the hub.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Self::new()
        }
    }

    /// `PUT path` with `body`, panicking on a transport error: the callers are fixtures.
    pub async fn put(&self, path: &str, body: &str) -> Reply {
        let request = authorized("PUT", path).body(Full::new(Bytes::from(body.to_owned())));
        self.send(request.expect("the request"))
            .await
            .unwrap_or_else(|error| panic!("PUT {path}: {error}"))
    }

    /// `POST` one batch under `idem_key`; `Err` names the transport failure.
    pub async fn post_batch(
        &self,
        ws: &str,
        plugin: &str,
        body: &str,
        idem_key: &str,
    ) -> Result<Reply, String> {
        let path = format!("/v1/workspaces/{ws}/plugins/{plugin}/batches");
        let request = authorized("POST", &path)
            .header("idempotency-key", idem_key)
            .body(Full::new(Bytes::from(body.to_owned())))
            .expect("the request");
        self.send(request).await
    }

    /// The workspace and the fixture manifest, the state every durability case starts from.
    pub async fn ready(&self, ws: &str, plugin: &str) {
        for (path, body) in [
            (format!("/v1/workspaces/{ws}"), ""),
            (format!("/v1/workspaces/{ws}/plugins/{plugin}"), MANIFEST),
        ] {
            let reply = self.put(&path, body).await;
            assert!(
                reply.code() == 200 || reply.code() == 201,
                "PUT {path}: {} {}",
                reply.code(),
                reply.body()
            );
        }
    }

    async fn send(&self, request: Request<Full<Bytes>>) -> Result<Reply, String> {
        let exchange = async {
            let response = self
                .client
                .request(request)
                .await
                .map_err(|e| e.to_string())?;
            let (parts, body) = response.into_parts();
            let body = body.collect().await.map_err(|e| e.to_string())?.to_bytes();
            Ok(Reply {
                status: parts.status,
                headers: parts.headers,
                body,
            })
        };
        tokio::time::timeout(self.timeout, exchange)
            .await
            .map_err(|_| String::from("timed out"))?
    }
}

impl Reply {
    /// The `seq` of a batch's 200 answer.
    pub fn seq(&self) -> u64 {
        assert_eq!(
            self.status,
            StatusCode::OK,
            "a batch answer: {}",
            self.body()
        );
        let body: serde_json::Value = serde_json::from_slice(&self.body).expect("a JSON body");
        body["seq"].as_u64().expect("a numeric seq")
    }
}

/// A request to the container, carrying its key under `Authorization: Bearer`.
fn authorized(method: &str, path: &str) -> axum::http::request::Builder {
    let base = read(&run_dir().join("url"));
    let key = read(&run_dir().join("key"));
    Request::builder()
        .method(method)
        .uri(format!("{base}{path}"))
        .header("authorization", format!("Bearer {key}"))
}

/// `target/hub-run`, from the crate's manifest directory: `gr` mounts the repository at `/w`.
fn run_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/hub-run")
}

/// A file's text, trimmed; the panic names the file and never its content.
fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}; run hub-run.sh start", path.display()))
        .trim()
        .to_owned()
}
