use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::Serialize;

use crate::messages::{Envelope, SignableEnvelope};

pub fn sign_envelope<T: Serialize>(
    signing: &SigningKey,
    message_type: &str,
    id: &str,
    ts: i64,
    payload: &T,
) -> anyhow::Result<Envelope> {
    let payload_value = serde_json::to_value(payload)?;
    let signable = SignableEnvelope {
        message_type: message_type.to_string(),
        id: id.to_string(),
        ts,
        payload: payload_value.clone(),
    };
    let bytes = serde_json::to_vec(&signable)?;
    let signature: Signature = signing.sign(&bytes);
    let sig_b64 = general_purpose::STANDARD.encode(signature.to_bytes());
    Ok(Envelope {
        message_type: message_type.to_string(),
        id: id.to_string(),
        ts,
        sig: Some(sig_b64),
        payload: payload_value,
    })
}

pub fn unsigned_envelope<T: Serialize>(
    message_type: &str,
    id: &str,
    ts: i64,
    payload: &T,
) -> anyhow::Result<Envelope> {
    let payload_value = serde_json::to_value(payload)?;
    Ok(Envelope {
        message_type: message_type.to_string(),
        id: id.to_string(),
        ts,
        sig: None,
        payload: payload_value,
    })
}

pub fn verify_envelope(envelope: &Envelope, verifying: &VerifyingKey) -> anyhow::Result<bool> {
    let sig_b64 = match &envelope.sig {
        Some(sig) => sig,
        None => return Ok(false),
    };
    let sig_bytes = general_purpose::STANDARD.decode(sig_b64)?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid signature length"))?;
    let signature = Signature::from_bytes(&sig_arr);

    let signable = SignableEnvelope {
        message_type: envelope.message_type.clone(),
        id: envelope.id.clone(),
        ts: envelope.ts,
        payload: envelope.payload.clone(),
    };
    let bytes = serde_json::to_vec(&signable)?;
    Ok(verifying.verify_strict(&bytes, &signature).is_ok())
}
