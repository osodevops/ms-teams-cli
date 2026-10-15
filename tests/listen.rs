//! Black-box tests of `teams listen` over real TCP on 127.0.0.1.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;
use predicates::prelude::*;

const WAIT: Duration = Duration::from_secs(20);

fn teams(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("teams"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env("APPDATA", home)
        .env("TEAMS_CLI_DISABLE_KEYRING", "1")
        .env("TEAMS_CLI_TOKEN_STORE", "file")
        .env_remove("TEAMS_CLI_CLIENT_STATE")
        .env_remove("TEAMS_CLI_PROFILE")
        .env_remove("TEAMS_CLI_ACCESS_TOKEN")
        .env_remove("RUST_LOG");
    cmd
}

/// A running `teams listen --port 0`, killed on drop.
struct Listener {
    child: Child,
    port: u16,
    stdout: Option<mpsc::Receiver<String>>,
    /// Held unread when the test stalls the consumer.
    #[cfg_attr(not(unix), allow(dead_code))]
    raw_stdout: Option<ChildStdout>,
    stderr: Arc<Mutex<String>>,
    _home: tempfile::TempDir,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Listener {
    fn start(args: &[&str], envs: &[(&str, &str)]) -> Self {
        Self::spawn(args, envs, true)
    }

    fn spawn(args: &[&str], envs: &[(&str, &str)], read_stdout: bool) -> Self {
        let home = tempfile::tempdir().unwrap();
        let mut cmd = teams(home.path());
        cmd.args(["listen", "--port", "0"]).args(args);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        // Drain stderr continuously so the process never blocks on it.
        let stderr = Arc::new(Mutex::new(String::new()));
        let (port_tx, port_rx) = mpsc::channel();
        {
            let stderr = stderr.clone();
            let pipe = child.stderr.take().unwrap();
            thread::spawn(move || {
                for line in BufReader::new(pipe).lines() {
                    let Ok(line) = line else { break };
                    {
                        let mut all = stderr.lock().unwrap();
                        all.push_str(&line);
                        all.push('\n');
                    }
                    if let Some(rest) = line.split("started on http://").nth(1) {
                        let port = rest.rsplit(':').next().unwrap().trim().parse::<u16>();
                        let _ = port_tx.send(port.unwrap());
                    }
                }
            });
        }

        let (stdout, raw_stdout) = if read_stdout {
            let (tx, rx) = mpsc::channel();
            let pipe = child.stdout.take().unwrap();
            thread::spawn(move || {
                for line in BufReader::new(pipe).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            (Some(rx), None)
        } else {
            (None, child.stdout.take())
        };

        let port = port_rx
            .recv_timeout(WAIT)
            .expect("listener did not report its address");
        Listener {
            child,
            port,
            stdout,
            raw_stdout,
            stderr,
            _home: home,
        }
    }

    fn next_line(&self) -> serde_json::Value {
        let line = self
            .stdout
            .as_ref()
            .unwrap()
            .recv_timeout(WAIT)
            .expect("no notification printed");
        serde_json::from_str(&line).unwrap()
    }

    fn no_more_lines(&self) {
        let got = self
            .stdout
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_millis(500));
        assert!(got.is_err(), "unexpected output: {got:?}");
    }

    fn stderr_eventually_contains(&self, needle: &str) -> String {
        let deadline = Instant::now() + WAIT;
        loop {
            let text = self.stderr.lock().unwrap().clone();
            if text.contains(needle) || Instant::now() > deadline {
                return text;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn assert_alive(&mut self) {
        assert!(self.child.try_wait().unwrap().is_none(), "listener exited");
    }

    fn connect(&self) -> TcpStream {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        stream
    }

    /// Send a raw request and return (status, body).
    fn raw(&self, request: &[u8]) -> (u16, String) {
        let mut stream = self.connect();
        stream.write_all(request).unwrap();
        read_response(&mut stream)
    }

    fn post(&self, target: &str, body: &str) -> (u16, String) {
        self.raw(post_request(target, body).as_bytes())
    }
}

fn post_request(target: &str, body: &str) -> String {
    format!(
        "POST {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn read_response(stream: &mut TcpStream) -> (u16, String) {
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("no HTTP response: {text:?}"));
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

const VALID: &str = r#"{"value":[{"subscriptionId":"sub-ok","changeType":"created","resource":"chats/1/messages/2","clientState":"s3cret-value","tenantId":"t","subscriptionExpirationDateTime":"2026-10-12T00:00:00Z"}]}"#;

#[test]
fn client_state_filters_forged_notifications() {
    let listener = Listener::start(&[], &[("TEAMS_CLI_CLIENT_STATE", "s3cret-value")]);

    let forged = r#"{"value":[{"subscriptionId":"sub-forged","changeType":"created","clientState":"guessed-value"}]}"#;
    let missing = r#"{"value":[{"subscriptionId":"sub-missing","changeType":"created"}]}"#;
    assert_eq!(listener.post("/", forged).0, 202);
    assert_eq!(listener.post("/webhook", missing).0, 202);
    assert_eq!(listener.post("/", VALID).0, 202);

    // Only the valid notification is printed, with kind and without clientState.
    let line = listener.next_line();
    assert_eq!(line["subscriptionId"], "sub-ok");
    assert_eq!(line["kind"], "change");
    assert_eq!(
        line["subscriptionExpirationDateTime"],
        "2026-10-12T00:00:00Z"
    );
    assert!(line.get("clientState").is_none(), "{line}");
    listener.no_more_lines();

    let stderr = listener.stderr_eventually_contains("clientState missing or wrong");
    assert!(stderr.contains("clientState missing or wrong"), "{stderr}");
    // stderr is a pipe here, so log lines carry no colour codes.
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
    assert!(!stderr.contains("s3cret-value"), "{stderr}");
    assert!(!stderr.contains("guessed-value"), "{stderr}");
    assert!(!stderr.contains("not authenticated"), "{stderr}");
}

#[test]
fn lifecycle_notifications_are_printed_and_unauthenticated_mode_warns() {
    let listener = Listener::start(&[], &[]);
    let stderr = listener.stderr_eventually_contains("not authenticated");
    assert!(stderr.contains("http://127.0.0.1:"), "{stderr}");
    assert!(stderr.contains("not authenticated"), "{stderr}");

    let body = r#"{"value":[
        {"subscriptionId":"s","lifecycleEvent":"reauthorizationRequired","clientState":"x","organizationId":"o"},
        {"subscriptionId":"s","lifecycleEvent":"subscriptionRemoved"},
        {"subscriptionId":"s","lifecycleEvent":"missed"}
    ]}"#;
    assert_eq!(listener.post("/", body).0, 202);
    for event in ["reauthorizationRequired", "subscriptionRemoved", "missed"] {
        let line = listener.next_line();
        assert_eq!(line["kind"], "lifecycle");
        assert_eq!(line["lifecycleEvent"], event);
        assert!(line.get("clientState").is_none(), "{line}");
    }
}

#[test]
fn oversized_body_is_413_and_listener_survives() {
    let mut listener = Listener::start(&[], &[]);
    let size = 5 * 1024 * 1024;

    // Announced size: answered before any body byte is sent.
    let head = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {size}\r\n\
         Expect: 100-continue\r\nConnection: close\r\n\r\n"
    );
    assert_eq!(listener.raw(head.as_bytes()).0, 413);

    // A real 5 MiB body. The server may reset the connection once it has
    // answered, so a reset is accepted here; what matters is what follows.
    let mut stream = listener.connect();
    let mut writer = stream.try_clone().unwrap();
    let request = post_request("/", &format!(r#"{{"value":["{}"]}}"#, "x".repeat(size)));
    let sender = thread::spawn(move || {
        let _ = writer.write_all(request.as_bytes());
    });
    let mut buf = [0u8; 12];
    if stream.read_exact(&mut buf).is_ok() {
        assert_eq!(&buf, b"HTTP/1.1 413");
    }
    drop(stream);
    sender.join().unwrap();

    listener.assert_alive();
    assert_eq!(listener.post("/", VALID).0, 202);
    assert_eq!(listener.next_line()["subscriptionId"], "sub-ok");
}

#[test]
fn stalled_connections_do_not_block_requests() {
    let mut listener = Listener::start(&[], &[]);

    // Fewer than the 64 connection slots: idle sockets, unfinished headers,
    // and complete headers with an unfinished body.
    let mut stalled = Vec::new();
    for _ in 0..30 {
        stalled.push(listener.connect());
    }
    let mut partial_head = Vec::new();
    for _ in 0..10 {
        let mut s = listener.connect();
        s.write_all(b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n")
            .unwrap();
        partial_head.push(s);
    }
    let mut partial_body = Vec::new();
    for _ in 0..10 {
        let mut s = listener.connect();
        s.write_all(
            b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 1000\r\n\r\n{\"value\"",
        )
        .unwrap();
        partial_body.push(s);
    }

    let started = Instant::now();
    assert_eq!(listener.post("/", VALID).0, 202);
    assert_eq!(
        listener.post("/webhook?validationToken=abc%20123", ""),
        (200, "abc 123".to_string())
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(listener.next_line()["subscriptionId"], "sub-ok");

    // The deadlines then close the stalled ones.
    let (status, _) = read_response(&mut partial_body[0]);
    assert_eq!(status, 408);
    let mut rest = Vec::new();
    let closed = partial_head[0].read_to_end(&mut rest);
    assert!(closed.is_ok(), "header deadline did not close: {closed:?}");
    listener.assert_alive();
}

#[test]
fn more_stalled_connections_than_slots_only_delay_requests() {
    let listener = Listener::start(&[], &[]);
    let stalled: Vec<_> = (0..70).map(|_| listener.connect()).collect();

    // Slots free when the header deadline closes the idle sockets.
    let mut stream = TcpStream::connect(("127.0.0.1", listener.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(40)))
        .unwrap();
    stream
        .write_all(post_request("/?validationToken=t", "").as_bytes())
        .unwrap();
    assert_eq!(read_response(&mut stream), (200, "t".to_string()));
    drop(stalled);
}

#[test]
fn stalled_stdout_answers_503_and_validation_still_works() {
    let listener = Listener::spawn(&[], &[], false);
    let pad = "x".repeat(900 * 1024);
    let body =
        format!(r#"{{"value":[{{"subscriptionId":"s","resourceData":{{"pad":"{pad}"}}}}]}}"#);

    // Pipe buffer plus the 8 MiB queue hold at most about ten of these.
    let mut statuses = Vec::new();
    for _ in 0..40 {
        let started = Instant::now();
        let (status, _) = listener.post("/", &body);
        assert!(started.elapsed() < Duration::from_secs(10), "request hung");
        statuses.push(status);
        if status == 503 {
            break;
        }
    }
    assert_eq!(statuses.last(), Some(&503), "{statuses:?}");
    assert!(statuses[..statuses.len() - 1].iter().all(|s| *s == 202));

    assert_eq!(
        listener.post("/?validationToken=still-here", ""),
        (200, "still-here".to_string())
    );
}

#[test]
fn port_in_use_is_a_config_error_naming_the_address() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let home = tempfile::tempdir().unwrap();
    let output = assert_cmd::Command::from_std(teams(home.path()))
        .args(["listen", "--port", &port.to_string()])
        .timeout(WAIT)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(10), "{text}");
    assert!(text.contains(&format!("127.0.0.1:{port}")), "{text}");
    assert!(text.contains("CONFIG_ERROR"), "{text}");
}

#[test]
fn listen_help_documents_flags_without_leaking_client_state() {
    let home = tempfile::tempdir().unwrap();
    teams(home.path())
        .args(["listen", "--help"])
        .env("TEAMS_CLI_CLIENT_STATE", "sentinel-client-state")
        .assert()
        .success()
        .stdout(predicate::str::contains("--bind"))
        .stdout(predicate::str::contains("127.0.0.1"))
        .stdout(predicate::str::contains("TEAMS_CLI_CLIENT_STATE"))
        .stdout(predicate::str::contains("sentinel-client-state").not());
}

/// Fill the stdout pipe and queue more behind it; returns the ids acknowledged.
#[cfg(unix)]
fn post_backlog(listener: &Listener) -> Vec<String> {
    let pad = "x".repeat(200 * 1024);
    (0..20)
        .map(|i| {
            let id = format!("n{i}");
            let body = format!(
                r#"{{"value":[{{"subscriptionId":"{id}","resourceData":{{"pad":"{pad}"}}}}]}}"#
            );
            assert_eq!(listener.post("/", &body).0, 202);
            id
        })
        .collect()
}

#[cfg(unix)]
fn send_signal(listener: &Listener, signal: &str) {
    let status = Command::new("kill")
        .args([signal, &listener.child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(unix)]
fn wait_exit(listener: &mut Listener) -> std::process::ExitStatus {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = listener.child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "listener did not exit");
        thread::sleep(Duration::from_millis(50));
    }
}

// Signals: Unix only. The queue logic itself is unit-tested on every OS.
#[cfg(unix)]
#[test]
fn ctrl_c_writes_out_every_acknowledged_notification() {
    let mut listener = Listener::spawn(&[], &[], false);
    let acked = post_backlog(&listener);

    send_signal(&listener, "-INT");
    listener.stderr_eventually_contains("Shutting down");
    // No longer admitting: the port closes.
    let deadline = Instant::now() + WAIT;
    while TcpStream::connect(("127.0.0.1", listener.port)).is_ok() {
        assert!(Instant::now() < deadline, "still accepting after Ctrl+C");
        thread::sleep(Duration::from_millis(50));
    }

    // The reader resumes and gets everything that was acknowledged.
    let stdout = listener.raw_stdout.take().unwrap();
    let printed: Vec<String> = BufReader::new(stdout)
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(&l.unwrap()).unwrap();
            v["subscriptionId"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(printed, acked);
    let status = wait_exit(&mut listener);
    assert_eq!(
        status.code(),
        Some(0),
        "{}",
        listener.stderr.lock().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn second_signal_abandons_the_queue_and_reports_the_loss() {
    let mut listener = Listener::spawn(&[], &[], false);
    post_backlog(&listener);

    send_signal(&listener, "-INT");
    listener.stderr_eventually_contains("Shutting down");
    send_signal(&listener, "-TERM");
    let status = wait_exit(&mut listener);
    let stderr = listener.stderr_eventually_contains("were not written");
    assert_eq!(status.code(), Some(1), "{stderr}");
    let lost: u64 = stderr
        .split("Error: ")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no lost count: {stderr}"));
    assert!(lost > 0 && lost <= 20, "{stderr}");
}
