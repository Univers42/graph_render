//! `graph-hub healthcheck`: the image's `HEALTHCHECK`, without curl and without an HTTP client
//! crate. One `GET /healthz` over a std socket; exit 0 only on a 200. The conventions and the
//! bounds are `server/graph-server/src/health.rs`'s, which this file mirrors rather than reuses
//! (`graph-server` is a dependency for `bearer` and `KeySet` only).

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// The whole check's budget: connect, write and read together.
/// Caveat: 2 s is the image's `HEALTHCHECK` interval and docker's own start-period reasoning, not
/// a measurement of a loaded hub, so it fails fast on a hub too busy to answer `/healthz` in 2 s,
/// which restarts it. There is no retry inside the budget, so one lost segment counts as unhealthy.
pub const BUDGET: Duration = Duration::from_secs(2);

/// The request head is never longer than this; a status line that does not fit is not a 200.
/// Caveat: 1024 bytes is wider than any status line an HTTP/1.1 server writes and narrow enough to
/// stay off the heap; the loop stops there and treats what it has as not-a-200, so a hub that
/// prefixes a long reason phrase reads as unhealthy rather than as malformed.
const MAX_STATUS_LINE: usize = 1024;

/// True only when `127.0.0.1:port/healthz` answers 200 within [`BUDGET`].
pub fn healthcheck(port: u16) -> bool {
    probe(SocketAddr::from((Ipv4Addr::LOCALHOST, port)), BUDGET).unwrap_or(false)
}

/// One probe of `addr` under `budget`.
pub fn probe(addr: SocketAddr, budget: Duration) -> io::Result<bool> {
    let deadline = Instant::now() + budget;
    let mut stream = TcpStream::connect_timeout(&addr, budget)?;
    stream.set_write_timeout(Some(remaining(deadline)?))?;
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
    let mut head = Vec::with_capacity(64);
    let mut chunk = [0u8; 128];
    while !head.windows(2).any(|pair| pair == b"\r\n") && head.len() < MAX_STATUS_LINE {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        head.extend_from_slice(&chunk[..read]);
    }
    Ok(head.starts_with(b"HTTP/1.1 200 "))
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "healthcheck budget spent"))
}