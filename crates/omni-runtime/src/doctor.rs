//! Environment checks behind `omni doctor`: the platform's input permission
//! checks plus what the daemon itself needs (a usable screen size and a
//! writable state directory). Pure inspection — nothing here changes state.

use crate::config::{Config, Paths};
pub use omni_input::diag::Check;

/// Runs every check. Order is the order they should be read in.
pub fn run_checks(paths: &Paths) -> Vec<Check> {
    let mut checks = omni_input::platform::diagnose();
    checks.push(screen_check(paths));
    checks.push(state_dir_check(paths));
    checks
}

/// The daemon needs the local screen geometry for edge detection: detected
/// where the platform can, configured where it cannot (Linux).
fn screen_check(paths: &Paths) -> Check {
    if let Some((width, height)) = omni_input::platform::primary_screen_size() {
        return Check::ok("screen size", format!("detected {width}x{height}"));
    }
    let configured = Config::load(paths).ok().and_then(|config| config.screen);
    match configured {
        Some((width, height)) => Check::ok("screen size", format!("from config: {width}x{height}")),
        None => Check::failed(
            "screen size",
            format!(
                "not detectable on this platform and not configured — set \
                 {{\"screen\": [width, height]}} in {}",
                paths.config_file().display()
            ),
        ),
    }
}

/// Identity, trust store, socket, and log all live in the state directory.
fn state_dir_check(paths: &Paths) -> Check {
    match paths.ensure() {
        Ok(()) => Check::ok(
            "state directory",
            format!("{} is writable", paths.dir().display()),
        ),
        Err(e) => Check::failed(
            "state directory",
            format!("cannot create {}: {e}", paths.dir().display()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> Paths {
        let dir =
            std::env::temp_dir().join(format!("omni-test-doctor-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Paths::at(dir)
    }

    #[test]
    fn reports_platform_and_runtime_checks() {
        let paths = temp_paths("all");
        let checks = run_checks(&paths);
        // At least one platform check plus screen and state dir.
        assert!(checks.len() >= 3);
        let names: Vec<_> = checks.iter().map(|c| c.name).collect();
        assert!(names.contains(&"screen size"));
        assert!(names.contains(&"state directory"));
    }

    #[test]
    fn state_dir_check_creates_and_passes() {
        let paths = temp_paths("dir");
        let check = state_dir_check(&paths);
        assert!(check.ok, "{}", check.detail);
        assert!(paths.dir().is_dir());
        let _ = std::fs::remove_dir_all(paths.dir());
    }
}
