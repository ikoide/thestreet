use anyhow::anyhow;
use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    Key, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub struct X25519Identity {
    pub secret: StaticSecret,
    pub public_b64: String,
}

pub fn identity_from_signing_key(signing: &ed25519_dalek::SigningKey) -> X25519Identity {
    let secret = StaticSecret::from(signing.to_bytes());
    let public = PublicKey::from(&secret);
    let public_b64 = general_purpose::STANDARD.encode(public.as_bytes());
    X25519Identity { secret, public_b64 }
}

pub fn shared_key(
    secret: &StaticSecret,
    peer_pub_b64: &str,
    context: &[u8],
) -> anyhow::Result<[u8; 32]> {
    let peer_bytes = general_purpose::STANDARD.decode(peer_pub_b64)?;
    let peer_arr: [u8; 32] = peer_bytes
        .try_into()
        .map_err(|_| anyhow!("invalid x25519 pubkey length"))?;
    let peer = PublicKey::from(peer_arr);
    let shared = secret.diffie_hellman(&peer);
    let hk = Hkdf::<Sha256>::new(None, shared.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(context, &mut okm)
        .map_err(|_| anyhow!("hkdf expand failed"))?;
    Ok(okm)
}

pub fn encrypt_with_shared(
    secret: &StaticSecret,
    peer_pub_b64: &str,
    context: &[u8],
    plaintext: &[u8],
) -> anyhow::Result<(String, String)> {
    let key_bytes = shared_key(secret, peer_pub_b64, context)?;
    encrypt_with_key(&key_bytes, plaintext)
}

pub fn decrypt_with_shared(
    secret: &StaticSecret,
    peer_pub_b64: &str,
    context: &[u8],
    nonce_b64: &str,
    ciphertext_b64: &str,
) -> anyhow::Result<Vec<u8>> {
    let key_bytes = shared_key(secret, peer_pub_b64, context)?;
    decrypt_with_key(&key_bytes, nonce_b64, ciphertext_b64)
}

pub fn generate_room_key() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

pub fn encrypt_with_key(
    key_bytes: &[u8; 32],
    plaintext: &[u8],
) -> anyhow::Result<(String, String)> {
    let key = Key::from_slice(key_bytes);
    let cipher = XChaCha20Poly1305::new(key);
    let mut nonce_bytes = [0u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| anyhow!("encrypt failed"))?;
    Ok((
        general_purpose::STANDARD.encode(nonce_bytes),
        general_purpose::STANDARD.encode(ciphertext),
    ))
}

pub fn decrypt_with_key(
    key_bytes: &[u8; 32],
    nonce_b64: &str,
    ciphertext_b64: &str,
) -> anyhow::Result<Vec<u8>> {
    let key = Key::from_slice(key_bytes);
    let cipher = XChaCha20Poly1305::new(key);
    let nonce_bytes = general_purpose::STANDARD.decode(nonce_b64)?;
    let nonce_arr: [u8; 24] = nonce_bytes
        .try_into()
        .map_err(|_| anyhow!("invalid nonce length"))?;
    let nonce = XNonce::from_slice(&nonce_arr);
    let ciphertext = general_purpose::STANDARD.decode(ciphertext_b64)?;
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| anyhow!("decrypt failed"))?;
    Ok(plaintext)
}
