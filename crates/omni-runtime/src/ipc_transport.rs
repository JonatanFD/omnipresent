//! The local IPC channel between the `omni` CLI and the daemon, abstracted over
//! the platform's native local-IPC primitive:
//!
//! - **Unix**: a Unix-domain socket file in the config directory, mode `0600`
//!   so only the owner can command the daemon.
//! - **Windows**: a named pipe whose name is derived from the config directory.
//!   The pipe rejects remote (network) clients and claims the first instance,
//!   so another process cannot squat the name; access is otherwise governed by
//!   the pipe's default security, scoped to the local machine.
//!
//! The server side (`IpcListener`) is async, driven by the daemon's Tokio
//! runtime. The client side (`connect_blocking`) is synchronous, for the CLI,
//! which is a thin one-shot request/response tool with no runtime of its own.
//! Both client handle types implement [`std::io::Read`] and [`std::io::Write`].

use crate::config::Paths;
use std::io;

#[cfg(unix)]
mod imp {
    use super::*;
    use std::path::PathBuf;
    use tokio::net::{UnixListener, UnixStream};

    /// One accepted CLI connection, as the daemon sees it.
    pub type IpcStream = UnixStream;
    /// A synchronous client handle for the CLI.
    pub type IpcClient = std::os::unix::net::UnixStream;

    /// The daemon's IPC listener: a Unix-domain socket, owner-only.
    pub struct IpcListener {
        inner: UnixListener,
        path: PathBuf,
    }

    impl IpcListener {
        pub fn bind(paths: &Paths) -> io::Result<Self> {
            use std::os::unix::fs::PermissionsExt;
            let path = paths.socket_file();
            let _ = std::fs::remove_file(&path);
            let inner = UnixListener::bind(&path)?;
            // Only the owner may command the daemon.
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            Ok(Self { inner, path })
        }

        pub async fn accept(&mut self) -> io::Result<IpcStream> {
            let (stream, _) = self.inner.accept().await?;
            Ok(stream)
        }
    }

    impl Drop for IpcListener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Connects to the running daemon, or fails if it is not listening.
    pub fn connect_blocking(paths: &Paths) -> io::Result<IpcClient> {
        std::os::unix::net::UnixStream::connect(paths.socket_file())
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// One accepted CLI connection, as the daemon sees it.
    pub type IpcStream = NamedPipeServer;
    /// A synchronous client handle for the CLI. A named pipe opened as a file
    /// behaves as a bidirectional byte stream.
    pub type IpcClient = std::fs::File;

    /// The daemon's IPC listener: a named pipe with one instance always waiting
    /// to be connected.
    pub struct IpcListener {
        /// The next instance, already created and waiting for a client.
        next: NamedPipeServer,
        name: String,
    }

    impl IpcListener {
        pub fn bind(paths: &Paths) -> io::Result<Self> {
            let name = paths.pipe_name();
            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(&name)?;
            Ok(Self { next, name })
        }

        pub async fn accept(&mut self) -> io::Result<IpcStream> {
            // Wait for a client to connect to the waiting instance.
            self.next.connect().await?;
            // Stand up a fresh instance for the next client, and hand back the
            // one that just connected.
            let server = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&self.name)?;
            Ok(std::mem::replace(&mut self.next, server))
        }
    }

    /// Connects to the running daemon, or fails if it is not listening.
    pub fn connect_blocking(paths: &Paths) -> io::Result<IpcClient> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(paths.pipe_name())
    }
}

pub use imp::{IpcClient, IpcListener, IpcStream, connect_blocking};
