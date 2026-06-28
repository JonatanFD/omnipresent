//! Where the daemon keeps its state on disk, and the small user-editable
//! configuration.
//!
//! Everything lives under one directory (`~/.config/omni` by platform
//! convention, overridable with `OMNI_CONFIG_DIR` for tests and side-by-side
//! runs): the identity key pair, the trust store, the config file, the IPC
//! socket, and the daemon log.

use omni_topology::Edge;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The UDP port the daemon listens on unless configured otherwise.
pub const DEFAULT_PORT: u16 = 4733;

/// The filesystem layout of the daemon's state directory.
#[derive(Debug, Clone)]
pub struct Paths {
    dir: PathBuf,
}

impl Paths {
    /// The standard location: `OMNI_CONFIG_DIR` if set, otherwise the
    /// platform config dir (`~/.config/omni` or its macOS equivalent).
    pub fn resolve() -> Result<Self, ConfigError> {
        if let Ok(dir) = std::env::var("OMNI_CONFIG_DIR") {
            return Ok(Self { dir: dir.into() });
        }
        let base = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(Self {
            dir: base.join("omni"),
        })
    }

    /// A layout rooted at an explicit directory (tests).
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Creates the directory if missing.
    pub fn ensure(&self) -> Result<(), ConfigError> {
        std::fs::create_dir_all(&self.dir).map_err(ConfigError::Io)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn config_file(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    pub fn certificate_file(&self) -> PathBuf {
        self.dir.join("identity.crt")
    }

    pub fn key_file(&self) -> PathBuf {
        self.dir.join("identity.key")
    }

    pub fn trust_file(&self) -> PathBuf {
        self.dir.join("trust.json")
    }

    pub fn socket_file(&self) -> PathBuf {
        self.dir.join("daemon.sock")
    }

    /// The Windows named-pipe name for this state directory. Derived from the
    /// directory path so two daemons with different `OMNI_CONFIG_DIR` (tests,
    /// side-by-side runs) get distinct pipes, just as they get distinct socket
    /// files on Unix.
    ///
    /// The derivation is a stable SHA-256 of the directory path (first 8 bytes,
    /// lowercase hex) rather than `DefaultHasher`, whose output is unspecified and
    /// not reproducible outside Rust. A stable, documented hash lets a non-Rust
    /// client (the native GUIs) compute the same name and reach the daemon.
    pub fn pipe_name(&self) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(self.dir.to_string_lossy().as_bytes());
        let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        format!(r"\\.\pipe\omni-{hex}")
    }

    pub fn log_file(&self) -> PathBuf {
        self.dir.join("daemon.log")
    }
}

/// User configuration. Absent fields fall back to defaults, and a missing
/// file means "all defaults".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Config {
    /// UDP port to listen on.
    pub port: Option<u16>,
    /// Screen size override as `[width, height]`, for platforms where it
    /// cannot be detected (Linux under Wayland/X11 without a display query).
    pub screen: Option<(u32, u32)>,
    /// Which edge of this machine's screen a given peer sits past, keyed by
    /// host. Lets the arrangement be more than the default left/right chain.
    /// A peer not listed here falls back to the default for how it connected.
    #[serde(default)]
    pub placements: HashMap<String, Edge>,
    /// Opt-in clipboard sharing. Off by default: while disabled the daemon never
    /// reads the local clipboard nor applies a remote one. A config file written
    /// before this field existed loads as `false`.
    #[serde(default)]
    pub clipboard_sharing_enabled: bool,
}

impl Config {
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    /// The configured edge for a peer host, if one was set.
    pub fn edge_for(&self, host: &str) -> Option<Edge> {
        self.placements.get(host).copied()
    }

    /// Loads the config file, or defaults if it does not exist.
    pub fn load(paths: &Paths) -> Result<Self, ConfigError> {
        match std::fs::read(paths.config_file()) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(ConfigError::Parse),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io(e)),
        }
    }

    /// Writes the config back to disk as pretty JSON.
    pub fn save(&self, paths: &Paths) -> Result<(), ConfigError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(ConfigError::Parse)?;
        std::fs::write(paths.config_file(), bytes).map_err(ConfigError::Io)
    }
}

/// Why configuration could not be loaded.
#[derive(Debug)]
pub enum ConfigError {
    /// The platform reports no config directory at all.
    NoConfigDir,
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NoConfigDir => f.write_str("no configuration directory on this platform"),
            ConfigError::Io(e) => write!(f, "config i/o failed: {e}"),
            ConfigError::Parse(e) => write!(f, "config file is invalid: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omni-test-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_config_file_means_defaults() {
        let paths = Paths::at(temp_dir("defaults"));
        let config = Config::load(&paths).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.port(), DEFAULT_PORT);
    }

    #[test]
    fn config_round_trips_through_the_file() {
        let paths = Paths::at(temp_dir("roundtrip"));
        let mut placements = HashMap::new();
        placements.insert("laptop".to_string(), Edge::Top);
        let config = Config {
            port: Some(5000),
            screen: Some((1920, 1080)),
            placements,
            clipboard_sharing_enabled: true,
        };
        std::fs::write(paths.config_file(), serde_json::to_vec(&config).unwrap()).unwrap();

        let loaded = Config::load(&paths).unwrap();
        assert_eq!(loaded, config);
        assert_eq!(loaded.port(), 5000);
        assert_eq!(loaded.edge_for("laptop"), Some(Edge::Top));
        assert!(loaded.clipboard_sharing_enabled);
    }

    #[test]
    fn clipboard_sharing_is_off_by_default() {
        // The opt-in guarantee: defaults and legacy config files leave it off.
        assert!(!Config::default().clipboard_sharing_enabled);
        let paths = Paths::at(temp_dir("clipboard-legacy"));
        std::fs::write(paths.config_file(), br#"{"port":4733}"#).unwrap();
        let loaded = Config::load(&paths).unwrap();
        assert!(!loaded.clipboard_sharing_enabled);
    }

    #[test]
    fn config_without_placements_still_loads() {
        let paths = Paths::at(temp_dir("legacy"));
        // A config file written before placements existed must still parse.
        std::fs::write(paths.config_file(), br#"{"port":4733}"#).unwrap();
        let loaded = Config::load(&paths).unwrap();
        assert_eq!(loaded.port(), 4733);
        assert!(loaded.placements.is_empty());
    }

    #[test]
    fn config_saves_and_reloads_placements() {
        let paths = Paths::at(temp_dir("save"));
        let mut config = Config::default();
        config.placements.insert("desktop".to_string(), Edge::Right);
        config.save(&paths).unwrap();

        let loaded = Config::load(&paths).unwrap();
        assert_eq!(loaded.edge_for("desktop"), Some(Edge::Right));
    }

    #[test]
    fn paths_live_under_the_root() {
        let paths = Paths::at("/tmp/omni-x");
        assert!(paths.socket_file().starts_with("/tmp/omni-x"));
        assert!(paths.trust_file().starts_with("/tmp/omni-x"));
    }

    #[test]
    fn pipe_name_is_a_stable_hash_of_the_dir() {
        // A stable, documented derivation so non-Rust clients (the native GUIs)
        // can reproduce it. This exact vector is also asserted by the C# client
        // (`clients/omni-windows/.../OmniPathsTests.cs`); keep them in lock-step.
        let name = Paths::at(r"C:\example\omni").pipe_name();
        assert_eq!(name, r"\\.\pipe\omni-3bf61631564ef580");

        // Deterministic and directory-specific.
        assert_eq!(name, Paths::at(r"C:\example\omni").pipe_name());
        assert_ne!(name, Paths::at(r"C:\other\omni").pipe_name());
    }
}
