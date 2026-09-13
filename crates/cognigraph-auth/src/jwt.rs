//! Minimal HS256 JWT sessions: short-lived, stateless (no revocation —
//! that is what API tokens are for), verified without a database hit.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cognigraph_core::{CogniGraphError, Result};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{Role, User};

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    role: Role,
    #[serde(default = "crate::default_tenant_string")]
    tenant: String,
    iat: u64,
    exp: u64,
}

fn auth_err(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::AuthError(message.into())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn sign(message: &str, secret: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(message.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Issue a session token for a verified user.
pub fn issue(user: &User, secret: &str, ttl_secs: u64) -> String {
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let now = now_unix();
    let claims = Claims {
        sub: user.key.clone(),
        username: user.username.clone(),
        role: user.role,
        tenant: user.tenant.clone(),
        iat: now,
        exp: now + ttl_secs,
    };
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("claims serialize"));
    let message = format!("{header}.{payload}");
    let signature = sign(&message, secret);
    format!("{message}.{signature}")
}

/// Verify signature and expiry; returns the embedded user identity.
pub fn verify(token: &str, secret: &str) -> Result<User> {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(auth_err("malformed session token"));
    };
    let message = format!("{header}.{payload}");
    let expected = sign(&message, secret);
    // Constant-time comparison via HMAC verify.
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(message.as_bytes());
    let provided = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| auth_err("malformed session token"))?;
    mac.verify_slice(&provided)
        .map_err(|_| auth_err("invalid session token signature"))?;
    let _ = expected;

    let claims: Claims = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| auth_err("malformed session token"))?,
    )
    .map_err(|_| auth_err("malformed session claims"))?;
    if claims.exp <= now_unix() {
        return Err(auth_err("session token expired"));
    }
    Ok(User {
        key: claims.sub,
        username: claims.username,
        role: claims.role,
        tenant: claims.tenant,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> User {
        User {
            key: "u1".into(),
            username: "vera".into(),
            role: Role::Viewer,
            tenant: "acme".into(),
        }
    }

    #[test]
    fn roundtrip_preserves_identity() {
        let token = issue(&user(), "s3cret", 60);
        let verified = verify(&token, "s3cret").unwrap();
        assert_eq!(verified.username, "vera");
        assert_eq!(verified.role, Role::Viewer);
    }

    #[test]
    fn rejects_tampering_wrong_secret_and_expiry() {
        let token = issue(&user(), "s3cret", 60);
        assert!(verify(&token, "other").is_err());
        let mut tampered = token.clone();
        tampered.replace_range(..1, "X");
        assert!(verify(&tampered, "s3cret").is_err());
        assert!(verify("not.a.jwt", "s3cret").is_err());
        let expired = issue(&user(), "s3cret", 0);
        assert!(verify(&expired, "s3cret").is_err());
    }
}
