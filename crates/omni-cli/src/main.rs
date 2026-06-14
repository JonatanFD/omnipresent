//! The `omni` binary: the single entry point for all user interaction.
//!
//! Every command except `start` is a thin client: one JSON request line over
//! the daemon's local IPC channel (a Unix socket, or a named pipe on Windows),
//! one response line back, pretty-printed. `start` re-executes this binary with
//! the hidden `daemon` subcommand, detached, so the daemon keeps running after
//! the terminal closes.

mod update;

use clap::{Parser, Subcommand};
use omni_runtime::Paths;
use omni_runtime::ipc::{Request, Response, StatusInfo};
use omni_runtime::ipc_transport::connect_blocking;
use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "omni",
    version,
    about = "One keyboard and mouse across machines"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the background daemon.
    Start,
    /// Stop the running daemon.
    Stop,
    /// Show whether the daemon is running and list active connections.
    Status,
    /// Request control of a remote machine.
    Connect { host: String },
    /// End an active session with a remote machine.
    Disconnect { host: String },
    /// Approve an incoming connection request (by host or fingerprint).
    Accept { peer: String },
    /// Deny an incoming connection request (by host or fingerprint).
    Reject { peer: String },
    /// List known peers, or remove one.
    Peers {
        #[command(subcommand)]
        action: Option<PeersAction>,
    },
    /// Show where peers sit in the virtual desktop, or place one. With no
    /// arguments, lists placements; with a host and an edge
    /// (left/right/top/bottom), places that peer.
    Layout {
        host: Option<String>,
        edge: Option<String>,
    },
    /// Check that the OS permissions and environment the daemon needs are in place.
    Doctor,
    /// Update omni to the latest release.
    Update,
    /// Stop the daemon and remove all config, certs, and peer data.
    Uninstall,
    /// Run the daemon in the foreground (what `omni start` launches).
    #[command(hide = true)]
    Daemon,
}

#[derive(Subcommand)]
enum PeersAction {
    /// Remove a peer from the trusted list.
    Remove { host: String },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Daemon => match omni_runtime::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("omni daemon: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Start => start(),
        Command::Stop => match request(Request::Stop) {
            Ok(Response::Ok) => {
                println!("daemon stopped");
                ExitCode::SUCCESS
            }
            Ok(other) => unexpected(other),
            Err(e) => {
                eprintln!("omni: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Status => match request(Request::Status) {
            Ok(Response::Status(status)) => {
                print_status(&status);
                ExitCode::SUCCESS
            }
            Ok(other) => unexpected(other),
            Err(_) => {
                println!("daemon: not running");
                ExitCode::SUCCESS
            }
        },
        Command::Connect { host } => simple(Request::Connect { host }, "connected"),
        Command::Disconnect { host } => simple(Request::Disconnect { host }, "disconnected"),
        Command::Accept { peer } => simple(Request::Accept { selector: peer }, "accepted"),
        Command::Reject { peer } => simple(Request::Reject { selector: peer }, "rejected"),
        Command::Peers { action: None } => match request(Request::Peers) {
            Ok(Response::Peers { peers }) => {
                if peers.is_empty() {
                    println!("no known peers");
                } else {
                    for peer in peers {
                        let host = peer.host.as_deref().unwrap_or("<unknown host>");
                        let state = if peer.connected {
                            "connected"
                        } else {
                            "trusted"
                        };
                        println!("{host}  {}  {state}", short(&peer.fingerprint));
                    }
                }
                ExitCode::SUCCESS
            }
            Ok(other) => unexpected(other),
            Err(e) => {
                eprintln!("omni: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Peers {
            action: Some(PeersAction::Remove { host }),
        } => simple(Request::RemovePeer { selector: host }, "removed"),
        Command::Layout { host, edge } => layout(host, edge),
        Command::Doctor => doctor(),
        Command::Update => update::update(request),
        Command::Uninstall => uninstall(),
    }
}

/// Lists peer placements, or sets one when a host and edge are given.
fn layout(host: Option<String>, edge: Option<String>) -> ExitCode {
    match (host, edge) {
        (Some(host), Some(edge)) => simple(
            Request::Layout {
                host: Some(host),
                edge: Some(edge),
            },
            "placed",
        ),
        (host @ Some(_), None) => {
            eprintln!(
                "omni: give an edge too, e.g. `omni layout {} right`",
                host.unwrap()
            );
            ExitCode::FAILURE
        }
        (None, _) => match request(Request::Layout {
            host: None,
            edge: None,
        }) {
            Ok(Response::Layout { placements }) => {
                if placements.is_empty() {
                    println!("no placements — peers use the default left/right chain");
                } else {
                    for p in placements {
                        let state = if p.connected { "connected" } else { "saved" };
                        println!("{}  past the {} edge  ({state})", p.host, p.edge);
                    }
                }
                ExitCode::SUCCESS
            }
            Ok(other) => unexpected(other),
            Err(e) => {
                eprintln!("omni: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

/// Sends a request whose happy answer is a bare `Ok`.
fn simple(req: Request, done: &str) -> ExitCode {
    match request(req) {
        Ok(Response::Ok) => {
            println!("{done}");
            ExitCode::SUCCESS
        }
        Ok(other) => unexpected(other),
        Err(e) => {
            eprintln!("omni: {e}");
            ExitCode::FAILURE
        }
    }
}

fn unexpected(response: Response) -> ExitCode {
    eprintln!("omni: unexpected reply from the daemon: {response:?}");
    ExitCode::FAILURE
}

/// One request, one reply, over the daemon's IPC channel.
fn request(req: Request) -> Result<Response, String> {
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    let mut stream = connect_blocking(&paths)
        .map_err(|_| "the daemon is not running (start it with `omni start`)".to_string())?;
    let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("could not reach the daemon: {e}"))?;
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| format!("the daemon did not answer: {e}"))?;
    if reply.is_empty() {
        return Err("the daemon closed the connection without answering".into());
    }
    let response: Response =
        serde_json::from_str(reply.trim_end()).map_err(|e| format!("bad reply: {e}"))?;
    if let Response::Error { message } = response {
        return Err(message);
    }
    Ok(response)
}

fn print_status(status: &StatusInfo) {
    println!("daemon: running (udp port {})", status.port);
    println!("fingerprint: {}", status.fingerprint);
    if !status.capturing {
        println!("input capture: unavailable — target only (run `omni doctor`)");
    }
    if status.sessions.is_empty() {
        println!("sessions: none");
    } else {
        println!("sessions:");
        for s in &status.sessions {
            let marker = if s.active { " (input here)" } else { "" };
            println!(
                "  {}  {}  {}{marker}",
                s.host,
                short(&s.fingerprint),
                s.role
            );
        }
    }
    for p in &status.pending {
        println!(
            "pending request from {} ({}) — `omni accept {}` to approve",
            p.host,
            short(&p.fingerprint),
            p.host
        );
    }
}

/// Fingerprints are 64 hex chars; the first 16 are plenty to tell peers apart.
fn short(fingerprint: &str) -> String {
    let head: String = fingerprint.chars().take(16).collect();
    format!("{head}…")
}

/// Prints every environment check, then what the daemon itself reports.
/// Exits non-zero if any requirement is unmet.
fn doctor() -> ExitCode {
    let paths = match Paths::resolve() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("omni: {e}");
            return ExitCode::FAILURE;
        }
    };

    let checks = omni_runtime::doctor::run_checks(&paths);
    let mut all_ok = true;
    for check in &checks {
        let mark = if check.ok { "ok " } else { "FAIL" };
        println!("[{mark}] {} — {}", check.name, check.detail);
        all_ok &= check.ok;
    }

    // The daemon's own view matters too: it may have been started from a
    // context with different permissions than this shell.
    match request(Request::Status) {
        Ok(Response::Status(status)) => {
            if status.capturing {
                println!("[ok ] daemon — running, input capture active");
            } else {
                println!(
                    "[FAIL] daemon — running but capture is off (target only);                      fix the permission above, then `omni stop && omni start`                      from this terminal"
                );
                all_ok = false;
            }
        }
        _ => println!("[ -- ] daemon — not running (`omni start`)"),
    }

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn start() -> ExitCode {
    if request(Request::Status).is_ok() {
        println!("daemon already running");
        return ExitCode::SUCCESS;
    }
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("omni: cannot locate own binary: {e}");
            return ExitCode::FAILURE;
        }
    };
    let spawned = std::process::Command::new(exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Err(e) = spawned {
        eprintln!("omni: could not start the daemon: {e}");
        return ExitCode::FAILURE;
    }

    // Wait briefly for the socket to come up so `start` can report honestly.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(Response::Status(status)) = request(Request::Status) {
            println!("daemon started (udp port {})", status.port);
            println!("fingerprint: {}", status.fingerprint);
            return ExitCode::SUCCESS;
        }
    }
    eprintln!("omni: the daemon did not come up — check the log in the config directory");
    ExitCode::FAILURE
}

fn uninstall() -> ExitCode {
    // Best effort: the daemon may not be running, and that is fine.
    let _ = request(Request::Stop);

    let paths = match Paths::resolve() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("omni: {e}");
            return ExitCode::FAILURE;
        }
    };
    match std::fs::remove_dir_all(paths.dir()) {
        Ok(()) => println!("removed {}", paths.dir().display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!("omni: could not remove {}: {e}", paths.dir().display());
            return ExitCode::FAILURE;
        }
    }

    // Deleting the running binary is allowed on Unix (the inode lives on).
    // Windows locks a running image, so deletion there fails and the user is
    // told to remove it — by then nothing of omnipresent is left running.
    if let Ok(exe) = std::env::current_exe() {
        match std::fs::remove_file(&exe) {
            Ok(()) => println!("removed {}", exe.display()),
            Err(e) => eprintln!("omni: remove {} manually: {e}", exe.display()),
        }
    }
    println!("omnipresent uninstalled");
    ExitCode::SUCCESS
}
