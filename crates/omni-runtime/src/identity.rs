//! This machine's TLS identity on disk: generated once, then reused so the
//! certificate fingerprint — the identity peers pin — stays stable.

use crate::config::Paths;
use omni_security::{LocalIdentity, fingerprint_of, generate_identity};
use std::io;

/// Loads the persisted identity, or generates and persists a fresh one on
/// first run. The private key file is created owner-readable only.
pub fn load_or_generate(paths: &Paths) -> io::Result<LocalIdentity> {
    let cert_path = paths.certificate_file();
    let key_path = paths.key_file();

    match (std::fs::read(&cert_path), std::fs::read(&key_path)) {
        (Ok(cert), Ok(key)) => {
            let fingerprint = fingerprint_of(&cert);
            Ok(LocalIdentity::new(cert, key, fingerprint))
        }
        (Err(e), _) | (_, Err(e)) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => {
            let identity =
                generate_identity(&hostname()).map_err(|e| io::Error::other(e.to_string()))?;
            std::fs::write(&cert_path, identity.certificate_der())?;
            write_private(&key_path, identity.private_key_der())?;
            Ok(identity)
        }
    }
}

/// Writes key material with 0600 permissions.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

/// This machine's hostname, for the certificate common name. Purely cosmetic:
/// trust comes from the fingerprint, not the name.
pub fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "omni".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> Paths {
        let dir =
            std::env::temp_dir().join(format!("omni-test-identity-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Paths::at(dir)
    }

    #[test]
    fn first_run_generates_and_second_run_reloads() {
        let paths = temp_paths("stable");

        let first = load_or_generate(&paths).unwrap();
        let second = load_or_generate(&paths).unwrap();

        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.certificate_der(), second.certificate_der());
    }

    #[test]
    fn key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let paths = temp_paths("perms");

        load_or_generate(&paths).unwrap();

        let mode = std::fs::metadata(paths.key_file())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
