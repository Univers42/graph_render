//! The listener: hyper's HTTP/1 connection loop around the router, and the image's own
//! `healthcheck` probe beside it.
//!
//! axum's `serve` exposes neither the header timeout nor the read-buffer cap nor a connection cap,
//! so the loop is spelled out here, exactly as `server/graph-server/src/serve.rs` spells it out
//! for the motor. Task 1 serves the two routes that exist; Task 4's limits read them from
//! [`graph_hub::config`] instead of the literals here.

use crate::app::App;
use axum::Router;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use std::io;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// How long the runtime waits for in-flight work after a `SIGTERM`. Caveat: 100 ms is short
/// enough that a container stop is quick and long enough for a finished task to hand its answer
/// back; past it the process exits with that task still running.
pub const RUNTIME_GRACE: Duration = Duration::from_millis(100);

/// The pause after a failed `accept`. Caveat: a guess, not a measurement; a persistent failure (a
/// full descriptor table) still logs ten lines a second until a connection closes.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

/// Serves until `SIGTERM` or `SIGINT`, then drains. Exit 0 after a drain, 1 when the listener
/// could not start.
pub fn run(addr: SocketAddr, limits: Limits, app: Arc<App>) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => return fail(&error),
    };
    let result = runtime.block_on(serve(addr, limits, app));
    runtime.shutdown_timeout(RUNTIME_GRACE);
    result.map_or_else(|error| fail(&error), |()| ExitCode::SUCCESS)
}

/// §6's three connection limits, as literals until Task 2 reads them from the settings. Caveat on
/// every one of them: a guess about a caller, not a measurement of one, and the escape hatch is
/// the variable Task 2 introduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// `GRAPH_HUB_HEADER_TIMEOUT_MS`: receiving the request head, per read.
    pub header_timeout: Duration,
    /// `GRAPH_HUB_MAX_HEADER_BYTES`: hyper's read buffer, so the largest request head. Past it
    /// hyper answers a bare 431, never the JSON error body, and the request never reaches a log
    /// line.
    pub max_header_bytes: usize,
    /// `GRAPH_HUB_MAX_CONNECTIONS`: open connections, held from before `accept`, so past it a
    /// connection waits in the kernel backlog and the client, not the hub, gives up.
    pub max_connections: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            header_timeout: Duration::from_millis(5_000),
            max_header_bytes: 16_384,
            max_connections: 256,
        }
    }
}

async fn serve(addr: SocketAddr, limits: Limits, app: Arc<App>) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    app.log(&serde_json::json!({ "event": "listening", "addr": listener.local_addr()?.to_string() }));
    let accept = Acceptor::new(crate::router(Arc::clone(&app)), limits);
    loop {
        tokio::select! {
            _ = terminate.recv() => break,
            _ = interrupt.recv() => break,
            (accepted, slot) = accept.next(&listener) => match accepted {
                Ok(stream) => accept.spawn(stream, slot, &app),
                Err(error) => {
                    app.log(&serde_json::json!({ "event": "accept", "error": error.to_string() }));
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            },
        }
    }
    drop(listener);
    Ok(())
}

/// Accepts connections under the connection cap and serves each on its own task.
struct Acceptor {
    router: Router,
    limits: Limits,
    slots: Arc<Semaphore>,
}

impl Acceptor {
    fn new(router: Router, limits: Limits) -> Self {
        let slots = Arc::new(Semaphore::new(limits.max_connections));
        Self { router, limits, slots }
    }

    /// The next connection once a slot is free; past the cap the kernel backlog holds it.
    async fn next(&self, listener: &TcpListener) -> (io::Result<TcpStream>, OwnedSemaphorePermit) {
        // The semaphore is never closed, so the acquire cannot fail.
        let slot = Arc::clone(&self.slots)
            .acquire_owned()
            .await
            .expect("connection slots are never closed");
        (listener.accept().await.map(|(stream, _)| stream), slot)
    }

    fn spawn(&self, stream: TcpStream, slot: OwnedSemaphorePermit, app: &Arc<App>) {
        let mut builder = http1::Builder::new();
        builder
            .timer(TokioTimer::new())
            .header_read_timeout(Some(self.limits.header_timeout))
            .max_buf_size(self.limits.max_header_bytes);
        let service = TowerToHyperService::new(self.router.clone());
        let connection = builder.serve_connection(TokioIo::new(stream), service);
        let app = Arc::clone(app);
        tokio::spawn(async move {
            let _slot = slot;
            if let Err(error) = connection.await {
                app.log(&serde_json::json!({ "event": "connection", "error": error.to_string() }));
            }
        });
    }
}

fn fail(error: &io::Error) -> ExitCode {
    eprintln!("graph-hub: {error}");
    ExitCode::FAILURE
}

