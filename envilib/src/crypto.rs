use crate::error::{Error, Result};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

// --- Key derivation ---

/// Derive an X25519 private key from a passphrase and workspace ID.
/// The workspace ID acts as the argon2id salt, binding the key to this workspace.
/// Parameters match the TypeScript proto: t=3, m=65536, p=1, dkLen=32.
pub fn derive_private_key(passphrase: &str, workspace_id: &str) -> Result<[u8; 32]> {
    let params = Params::new(65536, 3, 1, Some(32))
        .map_err(|e| Error::Other(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), workspace_id.as_bytes(), &mut output)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(output)
}

/// Derive X25519 public key from private key bytes.
pub fn get_public_key(private_key: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*private_key);
    *PublicKey::from(&secret).as_bytes()
}

// --- DEK generation ---

pub fn generate_dek() -> [u8; 32] {
    let mut dek = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut dek);
    dek
}

// --- DEK wrapping (ECIES: X25519 + HKDF-SHA256 + AES-256-GCM) ---

/// Wrap a DEK for a recipient's public key.
/// Output: base64( ephPub[32] || nonce[12] || ciphertext[48] )
pub fn wrap_dek(dek: &[u8; 32], recipient_public_key: &[u8; 32]) -> Result<String> {
    let rng = rand::thread_rng();
    let ephemeral_secret = EphemeralSecret::random_from_rng(rng);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    let recipient_pub = PublicKey::from(*recipient_public_key);
    let shared = ephemeral_secret.diffie_hellman(&recipient_pub);

    let wrapping_key = hkdf_derive(shared.as_bytes(), ephemeral_public.as_bytes(), b"bkey-dek-wrap-v1")?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = Key::<Aes256Gcm>::from_slice(&wrapping_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, dek.as_ref())
        .map_err(|_| Error::EncryptionFailed)?;

    let mut out = Vec::with_capacity(32 + 12 + ciphertext.len());
    out.extend_from_slice(ephemeral_public.as_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(B64.encode(&out))
}

/// Unwrap a DEK using the recipient's private key.
pub fn unwrap_dek(wrapped: &str, private_key: &[u8; 32]) -> Result<[u8; 32]> {
    let bytes = B64.decode(wrapped).map_err(|_| Error::DecryptionFailed)?;
    if bytes.len() < 44 {
        return Err(Error::DecryptionFailed);
    }

    let eph_pub_bytes: [u8; 32] = bytes[..32].try_into().map_err(|_| Error::DecryptionFailed)?;
    let nonce_bytes = &bytes[32..44];
    let ciphertext = &bytes[44..];

    let secret = StaticSecret::from(*private_key);
    let eph_pub = PublicKey::from(eph_pub_bytes);
    let shared = secret.diffie_hellman(&eph_pub);

    let wrapping_key = hkdf_derive(shared.as_bytes(), &eph_pub_bytes, b"bkey-dek-wrap-v1")?;

    let key = Key::<Aes256Gcm>::from_slice(&wrapping_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| Error::DecryptionFailed)?;

    plaintext.try_into().map_err(|_| Error::DecryptionFailed)
}

// --- Field encryption (AES-256-GCM) ---

/// Encrypt a plaintext string with the DEK.
/// Output: base64( nonce[12] || ciphertext )
pub fn encrypt_field(value: &str, dek: &[u8; 32]) -> Result<String> {
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|_| Error::EncryptionFailed)?;

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(B64.encode(&out))
}

/// Decrypt an encrypted field produced by encrypt_field.
pub fn decrypt_field(encrypted: &str, dek: &[u8; 32]) -> Result<String> {
    let bytes = B64.decode(encrypted).map_err(|_| Error::DecryptionFailed)?;
    if bytes.len() < 12 {
        return Err(Error::DecryptionFailed);
    }

    let nonce_bytes = &bytes[..12];
    let ciphertext = &bytes[12..];

    let key = Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| Error::DecryptionFailed)?;

    String::from_utf8(plaintext).map_err(|_| Error::DecryptionFailed)
}

// --- Helpers ---

fn hkdf_derive(ikm: &[u8], salt: &[u8], info: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(okm)
}
