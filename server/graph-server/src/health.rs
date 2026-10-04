//! `graph-server healthcheck` (Verdict condition 11): the image's `HEALTHCHECK`, without curl and
//! without an HTTP client crate. One `GET /healthz` over a std socket; exit 0 only on a 200.

use crate::breaks;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// The whole check's budget: connect, write and read together.
pub const BUDGET: Duration = Duration::from_secs(2);

/// The request head is never longer than this; a status line that does not fit is not a 200.
const MAX_STATUS_LINE: usize = 1024;

/// True only when `127.0.0.1:port/healthz` answers 200 within [`BUDGET`].
pub fn healthcheck(port: u16) -> bool {
    if breaks::on("always-healthy") {
        return true;
    }
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

#[cfg(test)]
mod tests {
    use super::probe;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    fn answer_with(reply: &'static [u8]) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0u8; 256];
            let _ = stream.read(&mut request);
            if !reply.is_empty() {
                let _ = stream.write_all(reply);
            }
            std::thread::sleep(Duration::from_secs(1));
        });
        addr
    }

    #[test]
    fn only_a_200_is_healthy() {
        let ok = answer_with(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        assert!(probe(ok, Duration::from_secs(2)).expect("probe"));
        let down = answer_with(b"HTTP/1.1 503 Service Unavailable\r\n\r\n");
        assert!(!probe(down, Duration::from_secs(2)).expect("probe"));
    }

    #[test]
    fn a_silent_server_is_unhealthy_within_the_budget() {
        let silent = answer_with(b"");
        let start = Instant::now();
        assert!(probe(silent, Duration::from_millis(300)).is_err());
        assert!(
            start.elapsed() < Duration::from_millis(900),
            "{:?}",
            start.elapsed()
        );
    }
}
