//! Check the `Authorization: Signature <hex>` header Xsolla puts on webhooks.
//!
//! The signature is `sha1(raw_body + secret_key)`, hex encoded, where the
//! secret key is the project's webhook secret from Publisher Account.
//! The demo polls order state instead of receiving webhooks, so nothing
//! routes here yet. It exists so a webhook can be added without guessing.

use secrecy::{ExposeSecret, SecretString};
use sha1::{Digest, Sha1};

/// Why a webhook was rejected.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    /// Header is missing or does not start with `Signature `.
    #[error("authorization header is not `Signature <hex>`")]
    Malformed,
    /// Header is well formed but does not match the body.
    #[error("signature does not match body")]
    Mismatch,
}

/// Hex signature Xsolla would send for `body` under `secret`.
#[must_use]
pub fn sign(body: &[u8], secret: &SecretString) -> String {
    let mut hasher = Sha1::new();
    hasher.update(body);
    hasher.update(secret.expose_secret().as_bytes());
    hex::encode(hasher.finalize())
}

/// Verifies `authorization` (the full header value) against the raw request body.
///
/// # Errors
/// [`SignatureError::Malformed`] for a bad header shape, [`SignatureError::Mismatch`]
/// when the digest differs. Comparison is constant time over the hex strings.
pub fn verify(
    authorization: &str,
    body: &[u8],
    secret: &SecretString,
) -> Result<(), SignatureError> {
    let given = authorization
        .strip_prefix("Signature ")
        .map(str::trim)
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(SignatureError::Malformed)?;
    let expected = sign(body, secret);
    if constant_time_eq(given.to_ascii_lowercase().as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(SignatureError::Mismatch)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &[u8] = br#"{"notification_type":"payment","transaction":{"id":1}}"#;

    fn secret() -> SecretString {
        SecretString::from("demo-webhook-secret")
    }

    #[test]
    fn accepts_matching_signature() -> Result<(), SignatureError> {
        let header = format!("Signature {}", sign(BODY, &secret()));
        verify(&header, BODY, &secret())
    }

    #[test]
    fn accepts_uppercase_hex() -> Result<(), SignatureError> {
        let header = format!("Signature {}", sign(BODY, &secret()).to_ascii_uppercase());
        verify(&header, BODY, &secret())
    }

    #[test]
    fn rejects_tampered_body() {
        let header = format!("Signature {}", sign(BODY, &secret()));
        let tampered = br#"{"notification_type":"payment","transaction":{"id":2}}"#;
        assert_eq!(verify(&header, tampered, &secret()), Err(SignatureError::Mismatch));
    }

    #[test]
    fn rejects_wrong_secret() {
        let header = format!("Signature {}", sign(BODY, &SecretString::from("other")));
        assert_eq!(verify(&header, BODY, &secret()), Err(SignatureError::Mismatch));
    }

    #[test]
    fn rejects_malformed_header() {
        for h in ["", "Signature", "Signature zz", "Bearer abc", "Signature abc"] {
            assert_eq!(verify(h, BODY, &secret()), Err(SignatureError::Malformed), "{h:?}");
        }
    }

    #[test]
    fn matches_known_vector() {
        // sha1("abc" + "key") computed independently with `printf 'abckey' | shasum`.
        assert_eq!(
            sign(b"abc", &SecretString::from("key")),
            "9d8012e71d1125f5a0f01ea2469b2bf5df949142"
        );
    }
}
