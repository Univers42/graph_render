//! A real `graph-server` process, for what the in-process router cannot show: exit codes,
//! signals, the header timeout and the process's own memory. The binary is cargo's build of this
//! crate, started with a cleared environment holding only what the test names, so the host's
//! `GRAPH_*` variables never leak in. `GM_SVC_BREAK` is passed through for the negative controls.

use super::scratch;
use graph_server::keys;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long a child may take to start, refuse, log a line or exit. Caveat: a fixed bound, so a
/// host slower than this reads as a failure rather than a hang.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// What a child starts from: a scratch directory and a 0600 key file holding one minted key.
pub struct Setup {
    pub dir: PathBuf,
    pub keys_file: PathBuf,
    /// The minted key; its hash is the key file's only line.
    pub key: String,
    env: Vec<(String, String)>,
}

/// A started server; dropped, it is killed.
pub struct Child {
    process: std::process::Child,
    pub addr: SocketAddr,
    pub key: String,
    pub keys_file: PathBuf,
    /// Every log line read so far, the ones [`Child::wait_line`] skipped included.
    pub seen: Vec<serde_json::Value>,
    lines: Receiver<String>,
}

/// A fresh setup with `env` over the defaults: a free port, two workers, the minted key file.
pub fn setup(env: &[(&str, &str)]) -> Setup {
    let dir = scratch();
    let minted = keys::keygen("tester").expect("a key from /dev/urandom");
    let keys_file = dir.join("keys");
    write_private(&keys_file, &format!("{}\n", minted.line));
    let mut vars = vec![
        ("GRAPH_PORT".to_owned(), "0".to_owned()),
        ("GRAPH_WORKERS".to_owned(), "2".to_owned()),
        ("GRAPH_API_KEYS_FILE".to_owned(), keys_file.display().to_string()),
    ];
    vars.extend(env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())));
    Setup {
        dir,
        keys_file,
        key: minted.key,
        env: vars,
    }
}

/// Writes `text` at mode 0600, whatever the umask.
pub fn write_private(path: &Path, text: &str) {
    std::fs::write(path, text).expect("a scratch file");
    let private = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, private).expect("chmod 0600");
}

impl Setup {
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_graph-server"));
        command.env_clear().envs(self.env.iter().map(|(k, v)| (k, v)));
        if let Some(breaks) = std::env::var_os("GM_SVC_BREAK") {
            command.env("GM_SVC_BREAK", breaks);
        }
        command
    }

    /// Starts the server and waits for its `listening` line.
    pub fn spawn(self) -> Child {
        let mut process = self.command();
        let mut process = process
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the server starts");
        let lines = read_lines(process.stdout.take().expect("piped stdout"));
        let mut child = Child {
            process,
            addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            key: self.key,
            keys_file: self.keys_file,
            seen: Vec::new(),
            lines,
        };
        let listening = child.wait_line(|line| line["event"] == "listening");
        let addr = listening["addr"].as_str().expect("the listening address");
        child.addr = addr.parse().expect("a socket address");
        child
    }

    /// Runs the server to its exit, expecting a refusal: the exit status and stderr.
    pub fn refused(&self) -> (ExitStatus, String) {
        let mut process = self.command();
        let mut process = process
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let status = exit_within(&mut process, PATIENCE)
            .unwrap_or_else(|| panic!("the server served instead of refusing to start"));
        let mut stderr = String::new();
        let pipe = process.stderr.as_mut().expect("piped stderr");
        pipe.read_to_string(&mut stderr).expect("stderr");
        (status, stderr)
    }
}

/// Every stdout line, on a channel fed by a reader thread.
fn read_lines(stdout: std::process::ChildStdout) -> Receiver<String> {
    let (send, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if send.send(line).is_err() {
                return;
            }
        }
    });
    lines
}

/// The exit status, or `None` (the process killed) when it outlives `within`.
fn exit_within(process: &mut std::process::Child, within: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(status) = process.try_wait().expect("try_wait") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = process.kill();
    let _ = process.wait();
    None
}

impl Child {
    /// The next log line `wanted` accepts, skipping the others.
    pub fn wait_line(&mut self, wanted: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => panic!("no such log line within {PATIENCE:?}"),
                Err(RecvTimeoutError::Disconnected) => panic!("the server closed its stdout"),
            };
            let parsed: serde_json::Value = serde_json::from_str(&line).expect("a JSON line");
            self.seen.push(parsed.clone());
            if wanted(&parsed) {
                return parsed;
            }
        }
    }

    /// Sends `name` (`TERM`, `HUP`) through `kill`. A builtin of `sh`: the image has no
    /// `/bin/kill`, and the crate forbids the `unsafe` a direct `kill(2)` needs.
    pub fn signal(&self, name: &str) {
        let pid = self.process.id().to_string();
        let sent = Command::new("sh")
            .args(["-c", "kill -s \"$0\" \"$1\"", name, &pid])
            .status()
            .expect("sh runs");
        assert!(sent.success(), "kill -s {name} failed");
    }

    /// The exit status, or `None` (the process killed) when it outlives `within`.
    pub fn exit_within(&mut self, within: Duration) -> Option<ExitStatus> {
        exit_within(&mut self.process, within)
    }

    /// One `/proc/<pid>/status` field in kB (`VmRSS`, `VmHWM`).
    pub fn vm_kb(&self, field: &str) -> u64 {
        let path = format!("/proc/{}/status", self.process.id());
        let status = std::fs::read_to_string(path).expect("/proc status");
        let line = status.lines().find(|l| l.starts_with(&format!("{field}:")));
        let value = line.and_then(|l| l.split_whitespace().nth(1));
        value.and_then(|v| v.parse().ok()).expect("a kB figure")
    }

    /// Resets `VmHWM` to the current RSS (`proc(5)`, `clear_refs` value 5, Linux 4.0+).
    pub fn reset_peak(&self) {
        let path = format!("/proc/{}/clear_refs", self.process.id());
        std::fs::write(path, "5").expect("clear_refs resets the peak");
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// A request head ending in the blank line, `Connection: close`, with `headers` added.
pub fn head(method: &str, target: &str, headers: &[String]) -> String {
    let mut head = format!("{method} {target} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n");
    for header in headers {
        head.push_str(header);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    head
}

/// The whole answer to `request`, read to the close: its status and its text.
pub fn exchange(addr: SocketAddr, request: &[u8]) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(PATIENCE)).expect("timeout");
    stream.write_all(request).expect("the request");
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).expect("the answer");
    let text = String::from_utf8_lossy(&answer).into_owned();
    (status_of(&text), text)
}

/// The status code of an answer's text, 0 when it holds no status line.
pub fn status_of(text: &str) -> u16 {
    let code = text.strip_prefix("HTTP/1.1 ").and_then(|rest| rest.get(..3));
    code.and_then(|c| c.parse().ok()).unwrap_or(0)
}

/// `Authorization` for `key`.
pub fn bearer(key: &str) -> String {
    format!("Authorization: Bearer {key}")
}
