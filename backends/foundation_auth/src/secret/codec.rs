//! The version 1 and version 2 JSON envelopes and the signed byte string.
//!
//! Version 1 signs exactly the bytes that the agent RPC branch signs today, so
//! an old secret still verifies. Version 2 adds the purpose, the audience, the
//! key ID, the use limit and the subject to the signed string.

use base64::Engine;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{
    DecodeLimits, SecretDomain, SecretError, SecretPurpose, SignedSecret, SigningAlgorithm,
    SECRET_BYTES, VERSION_1, VERSION_2,
};

/// The base64url alphabet without padding.
fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

/// The version 1 envelope. Exactly the seven fields of the branch format.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeV1 {
    v: u32,
    alg: String,
    public_key: String,
    secret: String,
    valid_from: u64,
    valid_until: u64,
    signature: String,
}

/// The version 2 envelope. Additive to version 1.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeV2 {
    v: u32,
    alg: String,
    kid: Option<String>,
    public_key: String,
    secret: String,
    valid_from: u64,
    valid_until: u64,
    purpose: String,
    #[serde(default)]
    aud: Option<String>,
    #[serde(default)]
    max_uses: Option<u32>,
    #[serde(default)]
    sub: Option<String>,
    signature: String,
}

/// Reads only the version, so the decoder chooses the strict type.
#[derive(Deserialize)]
struct VersionProbe {
    v: u32,
}

fn malformed(what: impl Into<String>) -> SecretError {
    SecretError::Malformed(what.into())
}

/// Encode a secret in the shared form.
pub(crate) fn encode(secret: &SignedSecret) -> Zeroizing<String> {
    let public_key = b64().encode(&secret.public_key);
    let secret_bytes = b64().encode(&*secret.secret);
    let signature = b64().encode(&secret.signature);
    let alg = secret.alg.as_str().to_string();

    let json = if secret.version == VERSION_1 {
        let envelope = EnvelopeV1 {
            v: VERSION_1,
            alg,
            public_key,
            secret: secret_bytes,
            valid_from: secret.valid_from,
            valid_until: secret.valid_until,
            signature,
        };
        Zeroizing::new(serde_json::to_vec(&envelope).unwrap_or_default())
    } else {
        let envelope = EnvelopeV2 {
            v: VERSION_2,
            alg,
            kid: secret.kid.clone(),
            public_key,
            secret: secret_bytes,
            valid_from: secret.valid_from,
            valid_until: secret.valid_until,
            purpose: secret.purpose.as_str().to_string(),
            aud: secret.audience.clone(),
            max_uses: secret.max_uses,
            sub: secret.subject.clone(),
            signature,
        };
        Zeroizing::new(serde_json::to_vec(&envelope).unwrap_or_default())
    };
    Zeroizing::new(b64().encode(&*json))
}

/// Decode a secret from the shared form. Format checks only.
pub(crate) fn decode(encoded: &str, limits: DecodeLimits) -> Result<SignedSecret, SecretError> {
    if encoded.len() > limits.max_encoded_bytes {
        return Err(malformed(format!(
            "the encoded secret is {} bytes; the limit is {}",
            encoded.len(),
            limits.max_encoded_bytes
        )));
    }
    let json = Zeroizing::new(
        b64()
            .decode(encoded.trim())
            .map_err(|e| malformed(format!("not base64url: {e}")))?,
    );
    let probe: VersionProbe = serde_json::from_slice(&json)
        .map_err(|e| malformed(format!("not a signed secret: {e}")))?;
    match probe.v {
        VERSION_1 => decode_v1(&json),
        VERSION_2 => decode_v2(&json),
        other => Err(SecretError::UnsupportedVersion(other)),
    }
}

fn decode_v1(json: &[u8]) -> Result<SignedSecret, SecretError> {
    let envelope: EnvelopeV1 =
        serde_json::from_slice(json).map_err(|e| malformed(format!("version 1: {e}")))?;
    let alg = parse_algorithm(&envelope.alg)?;
    let secret = Zeroizing::new(decode_field(&envelope.secret, "secret")?);
    let public_key = decode_field(&envelope.public_key, "public_key")?;
    let signature = decode_field(&envelope.signature, "signature")?;
    check_body(&secret, envelope.valid_from, envelope.valid_until)?;
    Ok(SignedSecret {
        version: VERSION_1,
        alg,
        kid: None,
        public_key,
        secret,
        valid_from: envelope.valid_from,
        valid_until: envelope.valid_until,
        // A version 1 secret is always an enrollment secret.
        purpose: SecretPurpose::Enroll,
        audience: None,
        max_uses: None,
        subject: None,
        signature,
    })
}

fn decode_v2(json: &[u8]) -> Result<SignedSecret, SecretError> {
    let envelope: EnvelopeV2 =
        serde_json::from_slice(json).map_err(|e| malformed(format!("version 2: {e}")))?;
    let alg = parse_algorithm(&envelope.alg)?;
    let purpose = SecretPurpose::parse(&envelope.purpose)
        .ok_or_else(|| malformed(format!("unknown purpose {:?}", envelope.purpose)))?;
    let secret = Zeroizing::new(decode_field(&envelope.secret, "secret")?);
    let public_key = decode_field(&envelope.public_key, "public_key")?;
    let signature = decode_field(&envelope.signature, "signature")?;
    check_body(&secret, envelope.valid_from, envelope.valid_until)?;
    Ok(SignedSecret {
        version: VERSION_2,
        alg,
        kid: envelope.kid,
        public_key,
        secret,
        valid_from: envelope.valid_from,
        valid_until: envelope.valid_until,
        purpose,
        audience: envelope.aud,
        max_uses: envelope.max_uses,
        subject: envelope.sub,
        signature,
    })
}

fn parse_algorithm(name: &str) -> Result<SigningAlgorithm, SecretError> {
    SigningAlgorithm::parse(name)
        .ok_or_else(|| SecretError::UnsupportedAlgorithm(format!("unknown algorithm {name:?}")))
}

fn decode_field(text: &str, field: &str) -> Result<Vec<u8>, SecretError> {
    b64()
        .decode(text.as_bytes())
        .map_err(|e| malformed(format!("{field}: {e}")))
}

fn check_body(
    secret: &Zeroizing<Vec<u8>>,
    valid_from: u64,
    valid_until: u64,
) -> Result<(), SecretError> {
    if secret.len() != SECRET_BYTES {
        return Err(malformed(format!(
            "the secret must be {SECRET_BYTES} bytes, not {}",
            secret.len()
        )));
    }
    if valid_until <= valid_from {
        return Err(malformed("empty validity window"));
    }
    Ok(())
}

fn push_len_prefixed(message: &mut Vec<u8>, part: &[u8]) {
    // Every part is far below 4 GiB.
    message.extend_from_slice(&u32::try_from(part.len()).unwrap_or(u32::MAX).to_be_bytes());
    message.extend_from_slice(part);
}

/// The byte string that the signature covers.
///
/// The domain is a property of the issuer and the verifier, not of the wire
/// format. An agent RPC secret and a proxy registration secret never verify
/// each other, because their signed strings start with different domains.
pub(crate) fn signed_message(secret: &SignedSecret, domain: SecretDomain) -> Zeroizing<Vec<u8>> {
    let version = if secret.version == VERSION_1 {
        VERSION_1
    } else {
        VERSION_2
    };
    let mut message = Zeroizing::new(Vec::new());
    message.extend_from_slice(domain.0.as_bytes());
    message.extend_from_slice(match version {
        VERSION_1 => b".v1",
        _ => b".v2",
    });
    message.push(0);

    if version == VERSION_1 {
        push_len_prefixed(&mut message, secret.alg.as_str().as_bytes());
        push_len_prefixed(&mut message, &secret.public_key);
        push_len_prefixed(&mut message, &secret.secret);
        message.extend_from_slice(&secret.valid_from.to_be_bytes());
        message.extend_from_slice(&secret.valid_until.to_be_bytes());
        return message;
    }

    push_len_prefixed(&mut message, secret.alg.as_str().as_bytes());
    push_len_prefixed(
        &mut message,
        secret.kid.as_deref().unwrap_or_default().as_bytes(),
    );
    push_len_prefixed(&mut message, &secret.public_key);
    push_len_prefixed(&mut message, &secret.secret);
    push_len_prefixed(&mut message, secret.purpose.as_str().as_bytes());
    push_len_prefixed(
        &mut message,
        secret.audience.as_deref().unwrap_or_default().as_bytes(),
    );
    message.extend_from_slice(&secret.valid_from.to_be_bytes());
    message.extend_from_slice(&secret.valid_until.to_be_bytes());
    message.extend_from_slice(&secret.max_uses.unwrap_or(0).to_be_bytes());
    push_len_prefixed(
        &mut message,
        secret.subject.as_deref().unwrap_or_default().as_bytes(),
    );
    message
}
