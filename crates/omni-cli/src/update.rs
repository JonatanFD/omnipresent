//! `omni update`: replace this binary with the latest GitHub release.
//!
//! Keeps the dependency footprint tiny: it shells out to `curl` (for the
//! release metadata and the download) and `tar` (to unpack) — both present on
//! modern macOS, Linux, and Windows — and uses the `self-replace` crate to swap
//! the running binary in place, which handles the locked-image problem on
//! Windows. If the daemon is running it is stopped first and restarted after,
//! so the new version takes over cleanly.

use omni_runtime::ipc::{Request, Response};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

const REPO: &str = "JonatanFD/omnipresent";

/// Runs the update flow and returns a process exit code.
pub fn update(send: impl Fn(Request) -> Result<Response, String>) -> ExitCode {
    let current = env!("CARGO_PKG_VERSION");
    let target = match target_triple() {
        Some(t) => t,
        None => {
            eprintln!(
                "omni: self-update is not available for this platform ({} {}) — \
                 reinstall from https://github.com/{REPO}/releases/latest",
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            return ExitCode::FAILURE;
        }
    };

    println!("current version: {current}");
    let latest = match latest_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("omni: could not check for updates: {e}");
            return ExitCode::FAILURE;
        }
    };

    if latest == current {
        println!("already up to date.");
        return ExitCode::SUCCESS;
    }
    println!("updating to {latest} ...");

    // Stop the daemon if it is running, so its binary is not in use; remember to
    // bring it back afterwards.
    let was_running = send(Request::Status).is_ok();
    if was_running {
        let _ = send(Request::Stop);
        // Give the daemon a moment to release the binary on Windows.
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let result = download_and_replace(target);

    match result {
        Ok(()) => {
            println!("updated to {latest}.");
            if was_running {
                match restart_daemon() {
                    Ok(()) => println!("daemon restarted on the new version."),
                    Err(e) => eprintln!(
                        "omni: updated, but could not restart the daemon: {e} — run `omni start`"
                    ),
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("omni: update failed: {e}");
            if was_running {
                eprintln!("omni: the daemon was stopped — run `omni start` to bring it back");
            }
            ExitCode::FAILURE
        }
    }
}

/// The release asset target triple for this machine, or `None` if unsupported.
fn target_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

/// Queries the GitHub API for the latest release tag, normalised (no leading
/// `v`) so it compares against `CARGO_PKG_VERSION`.
fn latest_version() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = curl(&[
        "-H",
        "User-Agent: omni-update",
        "-H",
        "Accept: application/vnd.github+json",
        &url,
    ])?;
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| format!("unexpected response from GitHub: {e}"))?;
    let tag = json
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("the latest release has no tag")?;
    Ok(tag.trim_start_matches('v').to_string())
}

/// Downloads the latest archive for `target`, unpacks it, and replaces this
/// running binary with the new one.
fn download_and_replace(target: &str) -> Result<(), String> {
    let windows = std::env::consts::OS == "windows";
    let (archive, binary) = if windows {
        (format!("omni-{target}.zip"), "omni.exe")
    } else {
        (format!("omni-{target}.tar.gz"), "omni")
    };
    let url = format!("https://github.com/{REPO}/releases/latest/download/{archive}");

    let tmp = std::env::temp_dir().join(format!("omni-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("could not create a temp dir: {e}"))?;
    let cleanup = TmpDir(tmp.clone());

    let archive_path = tmp.join(&archive);
    println!("downloading {archive} ...");
    run(Command::new("curl")
        .arg("--proto")
        .arg("=https")
        .arg("--tlsv1.2")
        .arg("-fsSL")
        .arg("-o")
        .arg(&archive_path)
        .arg(&url))
    .map_err(|e| format!("download failed: {e}"))?;

    // `tar` unpacks .tar.gz on macOS/Linux and .zip via bsdtar on Windows.
    run(Command::new("tar")
        .arg("-xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&tmp))
    .map_err(|e| format!("could not unpack {archive}: {e}"))?;

    let new_binary = tmp.join(binary);
    if !new_binary.exists() {
        return Err(format!("the archive did not contain {binary}"));
    }
    make_executable(&new_binary);

    self_replace::self_replace(&new_binary)
        .map_err(|e| format!("could not replace the running binary: {e}"))?;
    drop(cleanup);
    Ok(())
}

/// Spawns a detached `omni start` using the freshly-replaced binary.
fn restart_daemon() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Command::new(exe)
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Runs `curl` with the given args and returns stdout, or an error.
fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("curl")
        .args(args)
        .output()
        .map_err(|e| format!("could not run curl (is it installed?): {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// Runs a command to completion, mapping a non-zero exit to an error.
fn run(cmd: &mut Command) -> Result<(), String> {
    let status = cmd
        .stdout(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("exited with {status}"))
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Removes a temp directory on drop.
struct TmpDir(std::path::PathBuf);
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
