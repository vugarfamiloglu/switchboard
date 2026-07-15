//! AES-256-GCM secret vault. A 32-byte key is generated on first boot to
//! `data/.vault-key` and used to seal provider secrets (device credentials, API
//! keys) at rest. Sealed form is base64(nonce ‖ ciphertext+tag).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;

#[derive(Clone)]
pub struct Vault {
    key: [u8; 32],
}

impl Vault {
    pub fn load_or_create(path: &str) -> Result<Self> {
        let key = match std::fs::read(path) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(&bytes);
                k
            }
            _ => {
                let mut k = [0u8; 32];
                OsRng.fill_bytes(&mut k);
                if let Some(dir) = std::path::Path::new(path).parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(path, k)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
                }
                k
            }
        };
        Ok(Self { key })
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key))
    }

    pub fn seal(&self, plaintext: &str) -> Result<String> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ct = self
            .cipher()
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| anyhow!("seal failed"))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        Ok(B64.encode(out))
    }

    pub fn open(&self, sealed: &str) -> Result<String> {
        let raw = B64.decode(sealed)?;
        if raw.len() < 12 {
            return Err(anyhow!("ciphertext too short"));
        }
        let (nonce, ct) = raw.split_at(12);
        let pt = self
            .cipher()
            .decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|_| anyhow!("open failed"))?;
        Ok(String::from_utf8(pt)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip() {
        let v = Vault { key: [7u8; 32] };
        let sealed = v.seal("hunter2").unwrap();
        assert_ne!(sealed, "hunter2");
        assert_eq!(v.open(&sealed).unwrap(), "hunter2");
    }

    #[test]
    fn wrong_key_fails() {
        let a = Vault { key: [1u8; 32] };
        let b = Vault { key: [2u8; 32] };
        let sealed = a.seal("secret").unwrap();
        assert!(b.open(&sealed).is_err());
    }
}
