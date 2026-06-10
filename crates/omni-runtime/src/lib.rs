//! Runtime: the composition root and long-running daemon.
//!
//! Wires concrete adapters into every module's ports and drives the
//! capture → route → send and receive → inject pipelines. Exposes a local IPC
//! surface (a Unix socket in the config directory) so the `omni` CLI can issue
//! commands and receive answers. The only place that depends on every module;
//! nothing depends on Runtime except the CLI binary.

pub mod config;
pub mod daemon;
pub mod doctor;
pub mod identity;
pub mod ipc;
pub mod ratelimit;
pub mod trust;

pub use config::{Config, Paths};
pub use daemon::{DaemonError, run};
