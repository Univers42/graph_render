//! The listener: hyper's HTTP/1 connection loop around the router. axum's own `serve` exposes
//! neither the header timeout nor the read-buffer cap nor a connection cap (Verdict condition
//! 4), so the loop is spelled out here. `SIGTERM` stops accepting and drains (condition 12);
//! `SIGHUP` re-reads the key file (condition 9).

use crate::app::App;
use crate::breaks;
use crate::config::{Connections, Settings};
use axum::Router;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use hyper_util::service::TowerToHyperService;
use serde_json::json;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// How long the runtime waits for blocking work after the drain. A run cannot be preempted, so
/// a run past its request's timeout is abandoned here rather than waited for.
const RUNTIME_GRACE: Duration = Duration::from_millis(100);

/// The pause after a failed `accept`. Caveat: a guess, not a measurement; a persistent failure
/// (a full descriptor table) still logs ten lines a second until a connection closes.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

/// Serves until `SIGTERM` or `SIGINT`, then drains. Exit 0 after a drain, 1 when the listener
/// could not start.
pub fn run(settings: &Settings, app: Arc<App>) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return fail(&error),
    };
    let result = runtime.block_on(serve(settings, app));
    runtime.shutdown_timeout(RUNTIME_GRACE);
    result.map_or_else(|error| fail(&error), |()| ExitCode::SUCCESS)
}

async fn serve(settings: &Settings, app: Arc<App>) -> io::Result<()> {
    let listener = TcpListener::bind((settings.bind, settings.port)).await?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    spawn_reload(&app, settings.keys_file.clone())?;
    // `listening` is the readiness line, so it comes after every handler: logged first, a SIGHUP
    // sent on it met the default action and killed the server (tests/reload.rs, 1 run in 5).
    app.log(&json!({ "event": "listening", "addr": listener.local_addr()?.to_string() }));
    let graceful = GracefulShutdown::new();
    let accept = Acceptor::new(crate::router(Arc::clone(&app)), settings.connections);
    loop {
        tokio::select! {
            _ = terminate.recv() => break,
            _ = interrupt.recv() => break,
            (accepted, slot) = accept.next(&listener) => match accepted {
                Ok(stream) => accept.spawn(stream, slot, &graceful, &app),
                Err(error) => {
                    app.log(&json!({ "event": "accept", "error": error.to_string() }));
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            },
        }
    }
    drop(listener);
    drain(graceful, settings.limits.timeout, &app).await;
    Ok(())
}

/// Accepts connections under the connection cap and serves each on its own task.
struct Acceptor {
    router: Router,
    connections: Connections,
    slots: Arc<Semaphore>,
}

impl Acceptor {
    fn new(router: Router, connections: Connections) -> Self {
        let slots = Arc::new(Semaphore::new(connections.max_connections));
        Self {
            router,
            connections,
            slots,
        }
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

    fn spawn(
        &self,
        stream: TcpStream,
        slot: OwnedSemaphorePermit,
        graceful: &GracefulShutdown,
        app: &Arc<App>,
    ) {
        let mut builder = http1::Builder::new();
        let header_timeout =
            (!breaks::on("no-header-timeout")).then_some(self.connections.header_timeout);
        builder
            .timer(TokioTimer::new())
            .header_read_timeout(header_timeout)
            .max_buf_size(self.connections.max_header_bytes);
        let service = TowerToHyperService::new(self.router.clone());
        let connection = graceful.watch(builder.serve_connection(TokioIo::new(stream), service));
        let app = Arc::clone(app);
        tokio::spawn(async move {
            let _slot = slot;
            if let Err(error) = connection.await {
                app.log(&json!({ "event": "connection", "error": error.to_string() }));
            }
        });
    }
}

/// `SIGHUP` re-reads the key file. The handler is installed even with auth off: the default
/// action of `SIGHUP` would end the process. The break keeps the handler and drops the reload, so
/// the process still survives the signal — ignoring `SIGHUP`, not dying of it.
fn spawn_reload(app: &Arc<App>, path: Option<PathBuf>) -> io::Result<()> {
    let mut hangup = signal(SignalKind::hangup())?;
    let app = Arc::clone(app);
    tokio::spawn(async move {
        while hangup.recv().await.is_some() {
            if breaks::on("ignore-sighup") {
                continue;
            }
            app.reload_keys(path.as_deref());
        }
    });
    Ok(())
}

/// Lets in-flight requests finish for up to `budget` (`GRAPH_TIMEOUT_MS`), then returns.
async fn drain(graceful: GracefulShutdown, budget: Duration, app: &App) {
    if breaks::on("no-drain") {
        return;
    }
    let drained = tokio::time::timeout(budget, graceful.shutdown())
        .await
        .is_ok();
    app.log(&json!({ "event": "stopped", "drained": drained }));
}

fn fail(error: &io::Error) -> ExitCode {
    eprintln!("graph-server: {error}");
    ExitCode::FAILURE
}
