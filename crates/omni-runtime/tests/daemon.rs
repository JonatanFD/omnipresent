//! Daemon-level integration test: two real daemons in one process, talking the
//! actual QUIC transport over loopback and driven through the actual IPC
//! channel — the same path the `omni` CLI uses.
//!
//! It exercises the wiring `daemon.rs` owns and that the unit tests cannot
//! reach: the IPC accept loop, the connect → pending → accept → established
//! handshake (including TOFU pinning), and disconnect/teardown. Input capture
//! is irrelevant here and is expected to be unavailable in CI (the daemons run
//! target-only), so the test never depends on it.

use omni_runtime::Paths;
use omni_runtime::ipc::{Request, Response};
use omni_runtime::ipc_transport::connect_blocking;
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

/// Sends one request to the daemon at `paths` and returns its reply.
fn send(paths: &Paths, req: &Request) -> std::io::Result<Response> {
    let mut stream = connect_blocking(paths)?;
    let mut line = serde_json::to_string(req).unwrap();
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    Ok(serde_json::from_str(reply.trim_end()).expect("a JSON response line"))
}

/// Waits until the daemon at `paths` answers a status request, or panics.
fn wait_until_up(paths: &Paths, who: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(Response::Status(_)) = send(paths, &Request::Status) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("daemon {who} never came up");
}

/// Polls a daemon's status until `pred` holds, returning the matching status.
fn wait_for_status(
    paths: &Paths,
    secs: u64,
    pred: impl Fn(&omni_runtime::ipc::StatusInfo) -> bool,
) -> omni_runtime::ipc::StatusInfo {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if let Ok(Response::Status(status)) = send(paths, &Request::Status)
            && pred(&status)
        {
            return status;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("status condition never met");
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omni-it-{name}-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes a config file pinning the UDP port for a daemon's state dir.
fn write_port(paths: &Paths, port: u16) {
    let config = format!(r#"{{"port":{port}}}"#);
    std::fs::write(paths.config_file(), config).unwrap();
}

#[test]
fn two_daemons_connect_accept_and_disconnect() {
    // Distinct ports per process run, to avoid clashing with anything else.
    let base = 40000 + (std::process::id() % 9000) as u16;
    let (port_a, port_b) = (base, base + 1);

    let dir_a = temp_dir("a");
    let dir_b = temp_dir("b");
    let paths_a = Paths::at(dir_a.clone());
    let paths_b = Paths::at(dir_b.clone());
    write_port(&paths_a, port_a);
    write_port(&paths_b, port_b);

    // Launch both daemons; each owns its own Tokio runtime inside the thread.
    let run_a = paths_a.clone();
    let handle_a = std::thread::spawn(move || {
        let _ = omni_runtime::run_with_paths(run_a);
    });
    let run_b = paths_b.clone();
    let handle_b = std::thread::spawn(move || {
        let _ = omni_runtime::run_with_paths(run_b);
    });

    wait_until_up(&paths_a, "A");
    wait_until_up(&paths_b, "B");

    // A dials B. The Connect request blocks until B accepts, so issue it from a
    // worker thread while the main thread plays B's user and accepts.
    let connect_paths = paths_a.clone();
    let connect = std::thread::spawn(move || {
        send(
            &connect_paths,
            &Request::Connect {
                host: format!("127.0.0.1:{port_b}"),
            },
        )
    });

    // B should see a pending request; approve it.
    wait_for_status(&paths_b, 15, |s| !s.pending.is_empty());
    let accepted = send(
        &paths_b,
        &Request::Accept {
            selector: "127.0.0.1".into(),
        },
    )
    .expect("accept request");
    assert!(
        matches!(accepted, Response::Ok),
        "accept failed: {accepted:?}"
    );

    // The dialing side's Connect now completes.
    let connect_result = connect
        .join()
        .expect("connect thread")
        .expect("connect ipc");
    assert!(
        matches!(connect_result, Response::Ok),
        "connect failed: {connect_result:?}"
    );

    // Both sides now report exactly one session, with mirrored roles.
    let status_a = wait_for_status(&paths_a, 10, |s| s.sessions.len() == 1);
    assert_eq!(status_a.sessions[0].role, "controller");
    let status_b = wait_for_status(&paths_b, 10, |s| s.sessions.len() == 1);
    assert_eq!(status_b.sessions[0].role, "target");

    // TOFU: B pinned A on accept, so A is now a known peer there.
    let peers_b = send(&paths_b, &Request::Peers).expect("peers");
    match peers_b {
        Response::Peers { peers } => assert_eq!(peers.len(), 1, "B should have pinned A"),
        other => panic!("unexpected peers reply: {other:?}"),
    }

    // A disconnects; both sides drop the session.
    let disconnected = send(
        &paths_a,
        &Request::Disconnect {
            host: "127.0.0.1".into(),
        },
    )
    .expect("disconnect");
    assert!(matches!(disconnected, Response::Ok));
    wait_for_status(&paths_a, 10, |s| s.sessions.is_empty());
    wait_for_status(&paths_b, 10, |s| s.sessions.is_empty());

    // Shut both daemons down and let their threads finish.
    let _ = send(&paths_a, &Request::Stop);
    let _ = send(&paths_b, &Request::Stop);
    let _ = handle_a.join();
    let _ = handle_b.join();

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}
