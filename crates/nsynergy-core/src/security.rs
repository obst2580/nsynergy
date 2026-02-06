use anyhow::{Context, Result};
use rand::Rng;
use rcgen::{CertificateParams, KeyPair};
use std::path::{Path, PathBuf};

/// A TLS identity consisting of a self-signed certificate and private key (PEM format).
#[derive(Debug, Clone)]
pub struct TlsIdentity {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Generates a self-signed TLS certificate for nsynergy communication.
///
/// The certificate uses the machine name as the common name and includes
/// `localhost` as a subject alternative name for local testing.
pub fn generate_self_signed_cert(machine_name: &str) -> Result<TlsIdentity> {
    let mut params = CertificateParams::new(vec![
        machine_name.to_string(),
        "localhost".to_string(),
    ])
    .context("creating certificate params")?;
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        rcgen::DnValue::Utf8String(machine_name.to_string()),
    );

    let key_pair = KeyPair::generate().context("generating key pair")?;
    let cert = params
        .self_signed(&key_pair)
        .context("generating self-signed certificate")?;

    Ok(TlsIdentity {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    })
}

/// Returns the default path for storing TLS certificates.
pub fn certs_dir() -> PathBuf {
    let base = crate::config::AppConfig::default_path();
    base.parent()
        .unwrap_or(Path::new("/tmp"))
        .join("certs")
}

/// Saves a TLS identity to disk (cert.pem and key.pem).
pub fn save_identity(dir: &Path, identity: &TlsIdentity) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating certs directory {}", dir.display()))?;

    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");

    std::fs::write(&cert_path, &identity.cert_pem)
        .with_context(|| format!("writing {}", cert_path.display()))?;
    std::fs::write(&key_path, &identity.key_pem)
        .with_context(|| format!("writing {}", key_path.display()))?;

    // Restrict key file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&key_path, perms)
            .with_context(|| "setting key file permissions")?;
    }

    Ok(())
}

/// Loads a TLS identity from disk.
pub fn load_identity(dir: &Path) -> Result<TlsIdentity> {
    let cert_pem = std::fs::read_to_string(dir.join("cert.pem"))
        .context("reading cert.pem")?;
    let key_pem = std::fs::read_to_string(dir.join("key.pem"))
        .context("reading key.pem")?;
    Ok(TlsIdentity { cert_pem, key_pem })
}

/// Loads or generates a TLS identity. If certificates exist on disk,
/// they are loaded; otherwise new ones are generated and saved.
pub fn load_or_generate_identity(machine_name: &str) -> Result<TlsIdentity> {
    let dir = certs_dir();
    if dir.join("cert.pem").exists() && dir.join("key.pem").exists() {
        load_identity(&dir)
    } else {
        let identity = generate_self_signed_cert(machine_name)?;
        save_identity(&dir, &identity)?;
        Ok(identity)
    }
}

/// Generates a random 6-digit pairing code for connection authentication.
pub fn generate_pairing_code() -> String {
    let mut rng = rand::thread_rng();
    let code: u32 = rng.gen_range(100_000..1_000_000);
    code.to_string()
}

/// Verifies that a submitted pairing code matches the expected one.
/// Uses constant-time comparison to prevent timing attacks.
pub fn verify_pairing_code(expected: &str, submitted: &str) -> bool {
    if expected.len() != submitted.len() {
        return false;
    }
    // Constant-time comparison
    let mut result = 0u8;
    for (a, b) in expected.bytes().zip(submitted.bytes()) {
        result |= a ^ b;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_cert_produces_valid_pem() {
        let identity = generate_self_signed_cert("test-machine").unwrap();
        assert!(identity.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(identity.cert_pem.contains("END CERTIFICATE"));
        assert!(identity.key_pem.contains("BEGIN PRIVATE KEY"));
        assert!(identity.key_pem.contains("END PRIVATE KEY"));
    }

    #[test]
    fn save_and_load_identity_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let identity = generate_self_signed_cert("roundtrip-test").unwrap();
        save_identity(dir.path(), &identity).unwrap();

        let loaded = load_identity(dir.path()).unwrap();
        assert_eq!(identity.cert_pem, loaded.cert_pem);
        assert_eq!(identity.key_pem, loaded.key_pem);
    }

    #[test]
    fn pairing_code_is_six_digits() {
        for _ in 0..100 {
            let code = generate_pairing_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
            let num: u32 = code.parse().unwrap();
            assert!(num >= 100_000 && num < 1_000_000);
        }
    }

    #[test]
    fn verify_matching_code() {
        assert!(verify_pairing_code("123456", "123456"));
    }

    #[test]
    fn verify_wrong_code() {
        assert!(!verify_pairing_code("123456", "654321"));
    }

    #[test]
    fn verify_different_length() {
        assert!(!verify_pairing_code("123456", "12345"));
        assert!(!verify_pairing_code("123456", "1234567"));
    }

    #[test]
    fn certs_dir_is_reasonable() {
        let dir = certs_dir();
        assert!(dir.ends_with("certs"));
    }

    #[test]
    fn load_or_generate_creates_new_certs() {
        let dir = tempfile::tempdir().unwrap();
        // Override the default directory by using save/load directly
        let identity = generate_self_signed_cert("new-machine").unwrap();
        save_identity(dir.path(), &identity).unwrap();

        let loaded = load_identity(dir.path()).unwrap();
        assert_eq!(identity.cert_pem, loaded.cert_pem);
    }

    #[cfg(unix)]
    #[test]
    fn key_file_has_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let identity = generate_self_signed_cert("perm-test").unwrap();
        save_identity(dir.path(), &identity).unwrap();

        let key_path = dir.path().join("key.pem");
        let perms = std::fs::metadata(&key_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }
}
