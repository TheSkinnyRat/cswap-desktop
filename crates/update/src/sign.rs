//! Release signatures. Each asset `X` ships with `X.sig`: one line of base64, the ed25519
//! signature over [`message`]. The message binds the asset's NAME and the release VERSION
//! to its SHA-512, so a validly signed file cannot be replayed under another name (a
//! portable exe served as the installer) or as a newer release (an old, vulnerable build
//! re-uploaded under a new tag).

use std::io::Read;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha512};

/// The public half of the production release key (base64 of the raw 32 bytes; hex
/// 2b4a42b8e18050a8993948d0384fbcb04899fe764b0f1c585e0f003ae5db3451). Only the main
/// session holds the private half (secret `CSWAP_UPDATE_SIGNING_KEY`, base64 of the 32-byte
/// seed). Changing it strands every installed copy: they would refuse all later releases.
pub const RELEASE_PUBLIC_KEY: &str = "K0pCuOGAUKiZOUjQOE+8sEiZ/nZLDxxYXg8AOuXbNFE=";

/// Environment variable holding the private key (base64 of the 32-byte seed).
pub const SIGNING_KEY_ENV: &str = "CSWAP_UPDATE_SIGNING_KEY";

const DOMAIN: &[u8] = b"claude-swap release v1\n";

pub fn message(name: &str, version: &str, sha512: &[u8; 64]) -> Vec<u8> {
    let mut m = Vec::with_capacity(DOMAIN.len() + name.len() + version.len() + 66);
    m.extend_from_slice(DOMAIN);
    m.extend_from_slice(name.as_bytes());
    m.push(b'\n');
    m.extend_from_slice(version.as_bytes());
    m.push(b'\n');
    m.extend_from_slice(sha512);
    m
}

pub fn sha512_file(path: &Path) -> std::io::Result<[u8; 64]> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha512::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

fn b64_32(s: &str, what: &str) -> Result<[u8; 32], String> {
    let bytes = B64.decode(s.trim()).map_err(|e| format!("{what} is not base64: {e}"))?;
    bytes.try_into().map_err(|b: Vec<u8>| format!("{what} must be 32 bytes, got {}", b.len()))
}

pub fn public_key(b64: &str) -> Result<VerifyingKey, String> {
    VerifyingKey::from_bytes(&b64_32(b64, "public key")?).map_err(|e| format!("bad public key: {e}"))
}

pub fn signing_key(b64: &str) -> Result<SigningKey, String> {
    Ok(SigningKey::from_bytes(&b64_32(b64, "signing key")?))
}

/// The compiled-in release key, or why there is none. `CSWAP_UPDATE_PUBLIC_KEY` at BUILD
/// time overrides the constant: a trial build can carry a test key without the source
/// ever naming one as trusted.
pub fn release_key() -> Result<VerifyingKey, String> {
    let key = option_env!("CSWAP_UPDATE_PUBLIC_KEY").filter(|k| !k.trim().is_empty()).unwrap_or(RELEASE_PUBLIC_KEY);
    if key.is_empty() {
        return Err("this build carries no release key, so it cannot verify updates".into());
    }
    public_key(key)
}

/// The `.sig` file's text for an asset.
pub fn sign(key: &SigningKey, name: &str, version: &str, sha512: &[u8; 64]) -> String {
    let sig = key.sign(&message(name, version, sha512));
    format!("{}\n", B64.encode(sig.to_bytes()))
}

/// Checks `sig_text` (a `.sig` file) against the asset's name, version and digest.
pub fn verify(key: &VerifyingKey, name: &str, version: &str, sha512: &[u8; 64], sig_text: &str) -> Result<(), String> {
    let raw = B64
        .decode(sig_text.trim().trim_start_matches('\u{feff}'))
        .map_err(|e| format!("{name}.sig is not base64: {e}"))?;
    let raw: [u8; 64] = raw.try_into().map_err(|_| format!("{name}.sig is not an ed25519 signature"))?;
    key.verify_strict(&message(name, version, sha512), &Signature::from_bytes(&raw))
        .map_err(|_| format!("{name} does not carry a valid signature for {version}"))
}

/// A new key pair as (private seed, public key), both base64. For the release owner; the
/// app never calls it.
pub fn keygen() -> Result<(String, String), String> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no randomness from the OS: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    Ok((B64.encode(seed), B64.encode(key.verifying_key().to_bytes())))
}

pub fn public_of(key: &SigningKey) -> String {
    B64.encode(key.verifying_key().to_bytes())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fixed TEST key. Never the release key: [`RELEASE_PUBLIC_KEY`] is separate.
    pub fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn digest(b: &[u8]) -> [u8; 64] {
        Sha512::digest(b).into()
    }

    #[test]
    fn signature_binds_name_version_and_bytes() {
        let k = test_key();
        let vk = k.verifying_key();
        let d = digest(b"installer bytes");
        let sig = sign(&k, "claude-swap-desktop-win-x64-setup.exe", "1.0.0", &d);
        assert!(verify(&vk, "claude-swap-desktop-win-x64-setup.exe", "1.0.0", &d, &sig).is_ok());
        // Any of the three changed: refused.
        assert!(verify(&vk, "claude-swap-desktop-win-x64-portable.exe", "1.0.0", &d, &sig).is_err());
        assert!(verify(&vk, "claude-swap-desktop-win-x64-setup.exe", "1.0.1", &d, &sig).is_err());
        assert!(verify(&vk, "claude-swap-desktop-win-x64-setup.exe", "1.0.0", &digest(b"other"), &sig).is_err());
        // Another key: refused.
        let other = SigningKey::from_bytes(&[9u8; 32]).verifying_key();
        assert!(verify(&other, "claude-swap-desktop-win-x64-setup.exe", "1.0.0", &d, &sig).is_err());
        // Garbage signature text: refused, not a panic.
        assert!(verify(&vk, "x", "1.0.0", &d, "not base64!").is_err());
        assert!(verify(&vk, "x", "1.0.0", &d, "AAAA").is_err());
    }

    #[test]
    fn the_compiled_release_key_is_the_production_key() {
        let k = public_key(RELEASE_PUBLIC_KEY).expect("a valid ed25519 public key");
        let hex: String = k.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "2b4a42b8e18050a8993948d0384fbcb04899fe764b0f1c585e0f003ae5db3451");
        // The test key is a different key: nothing it signs passes as a release.
        assert_ne!(test_key().verifying_key(), k);
    }

    #[test]
    fn keys_round_trip_through_base64() {
        let (private, public) = keygen().unwrap();
        let k = signing_key(&private).unwrap();
        assert_eq!(public_of(&k), public);
        assert!(public_key(&public).is_ok());
        assert!(signing_key("short").is_err());
    }

    #[test]
    fn file_digest_matches_in_memory_digest() {
        let dir = std::env::temp_dir().join(format!("cswap-sign-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.bin");
        let data: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
        std::fs::write(&f, &data).unwrap();
        assert_eq!(sha512_file(&f).unwrap(), digest(&data));
        let _ = std::fs::remove_dir_all(dir);
    }
}
