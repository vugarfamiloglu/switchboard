//! Operator auth primitives: bcrypt passcodes and an HMAC-signed session token
//! carrying role claims. Roles: owner, admin, operator, viewer. `viewer` is
//! read-only, enforced by the write-guard middleware in `api`.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64U;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const SESSION_COOKIE: &str = "sb_session";
pub const SESSION_TTL_SECS: i64 = 12 * 3600;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claims {
    pub exp: i64,
    pub oid: String,
    pub role: String,
    pub name: String,
}

pub fn hash_passcode(pass: &str) -> anyhow::Result<String> {
    Ok(bcrypt::hash(pass, bcrypt::DEFAULT_COST)?)
}

pub fn verify_passcode(hash: &str, pass: &str) -> bool {
    !hash.is_empty() && bcrypt::verify(pass, hash).unwrap_or(false)
}

pub fn new_secret() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    B64U.encode(b)
}

pub fn issue_token(secret: &str, oid: &str, role: &str, name: &str) -> anyhow::Result<String> {
    let claims = Claims {
        exp: now() + SESSION_TTL_SECS,
        oid: oid.to_string(),
        role: role.to_string(),
        name: name.to_string(),
    };
    let payload = B64U.encode(serde_json::to_vec(&claims)?);
    let sig = sign(secret, &payload);
    Ok(format!("{payload}.{sig}"))
}

pub fn parse_token(secret: &str, token: &str) -> Option<Claims> {
    let (payload, sig) = token.split_once('.')?;
    if sign(secret, payload) != sig {
        return None;
    }
    let claims: Claims = serde_json::from_slice(&B64U.decode(payload).ok()?).ok()?;
    if claims.exp < now() {
        return None;
    }
    Some(claims)
}

fn sign(secret: &str, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(payload.as_bytes());
    B64U.encode(mac.finalize().into_bytes())
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Read-only roles cannot perform mutating requests.
pub fn is_read_only(role: &str) -> bool {
    role == "viewer"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip() {
        let secret = new_secret();
        let tok = issue_token(&secret, "op_x", "admin", "Rza").unwrap();
        let claims = parse_token(&secret, &tok).expect("valid token");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.name, "Rza");
    }

    #[test]
    fn tampered_or_wrong_secret_rejected() {
        let tok = issue_token(&new_secret(), "op_x", "owner", "A").unwrap();
        assert!(parse_token(&new_secret(), &tok).is_none());
    }

    #[test]
    fn passcode_hash_verifies() {
        let h = hash_passcode("switchboard").unwrap();
        assert!(verify_passcode(&h, "switchboard"));
        assert!(!verify_passcode(&h, "wrong"));
        assert!(!verify_passcode("", "switchboard"));
    }
}
