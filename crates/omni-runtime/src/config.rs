//! Where the daemon keeps its state on disk, and the small user-editable
//! configuration.
//!
//! Everything lives under one directory (`~/.config/omni` by platform
//! convention, overridable with `OMNI_CONFIG_DIR` for tests and side-by-side
//! runs): the identity key pair, the trust store, the config file, the IPC
//! socket, and the daemon log.

use serde::{Deserialize, Serialize};
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
}

impl Config {
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    /// Loads the config file, or defaults if it does not exist.
    pub fn load(paths: &Paths) -> Result<Self, ConfigError> {
        match std::fs::read(paths.config_file()) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(ConfigError::Parse),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io(e)),
        }
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
        let config = Config {
            port: Some(5000),
            screen: Some((1920, 1080)),
        };
        std::fs::write(paths.config_file(), serde_json::to_vec(&config).unwrap()).unwrap();

        let loaded = Config::load(&paths).unwrap();
        assert_eq!(loaded, config);
        assert_eq!(loaded.port(), 5000);
    }

    #[test]
    fn paths_live_under_the_root() {
        let paths = Paths::at("/tmp/omni-x");
        assert!(paths.socket_file().starts_with("/tmp/omni-x"));
        assert!(paths.trust_file().starts_with("/tmp/omni-x"));
    }
}
