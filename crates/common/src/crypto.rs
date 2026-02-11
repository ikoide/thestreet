use anyhow::anyhow;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;

#[derive(Debug, Clone)]
pub struct Keypair {
    pub signing: SigningKey,
    pub verifying: VerifyingKey,
}

impl Keypair {
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let signing = SigningKey::generate(&mut rng);
        let verifying = VerifyingKey::from(&signing);
        Self { signing, verifying }
    }

    pub fn from_signing_key_bytes(bytes: [u8; 32]) -> Self {
        let signing = SigningKey::from_bytes(&bytes);
        let verifying = VerifyingKey::from(&signing);
        Self { signing, verifying }
    }

    pub fn signing_key_base64(&self) -> String {
        general_purpose::STANDARD.encode(self.signing.to_bytes())
    }

    pub fn verifying_key_base64(&self) -> String {
        general_purpose::STANDARD.encode(self.verifying.to_bytes())
    }
}

pub fn decode_signing_key(base64_str: &str) -> anyhow::Result<SigningKey> {
    let bytes = general_purpose::STANDARD.decode(base64_str)?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow!("invalid signing key length"))?;
    Ok(SigningKey::from_bytes(&arr))
}

pub fn decode_verifying_key(base64_str: &str) -> anyhow::Result<VerifyingKey> {
    let bytes = general_purpose::STANDARD.decode(base64_str)?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow!("invalid verifying key length"))?;
    Ok(VerifyingKey::from_bytes(&arr)?)
}

pub fn sign_bytes(signing: &SigningKey, bytes: &[u8]) -> String {
    let signature: Signature = signing.sign(bytes);
    general_purpose::STANDARD.encode(signature.to_bytes())
}

pub fn verify_signature(verifying: &VerifyingKey, bytes: &[u8], signature_b64: &str) -> bool {
    let sig_bytes = match general_purpose::STANDARD.decode(signature_b64) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let sig_arr: [u8; 64] = match sig_bytes.try_into() {
        Ok(arr) => arr,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_arr);
    verifying.verify_strict(bytes, &signature).is_ok()
}
