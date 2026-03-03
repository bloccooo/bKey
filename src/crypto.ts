import { x25519 } from "@noble/curves/ed25519.js";
import { argon2id } from "@noble/hashes/argon2.js";
import { hkdf } from "@noble/hashes/hkdf.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { gcm } from "@noble/ciphers/aes.js";
import { randomBytes } from "@noble/ciphers/utils.js";

// --- Key derivation ---

/**
 * Derive an X25519 private key from a passphrase and workspace ID.
 * The workspace ID acts as the argon2id salt, binding the key to this workspace.
 */
export function derivePrivateKey(passphrase: string, workspaceId: string): Uint8Array {
  const salt = new TextEncoder().encode(workspaceId);
  return argon2id(passphrase, salt, { t: 3, m: 65536, p: 1, dkLen: 32 });
}

export function getPublicKey(privateKey: Uint8Array): Uint8Array {
  return x25519.getPublicKey(privateKey);
}

// --- DEK generation ---

export function generateDek(): Uint8Array {
  return randomBytes(32);
}

// --- DEK wrapping (ECIES: X25519 + HKDF + AES-256-GCM) ---

/**
 * Wrap a DEK for a recipient's public key.
 * Output: base64( ephPub[32] || nonce[12] || ciphertext[48] )
 */
export function wrapDek(dek: Uint8Array, recipientPublicKey: Uint8Array): string {
  const ephPrivate = randomBytes(32);
  const ephPublic = x25519.getPublicKey(ephPrivate);
  const shared = x25519.getSharedSecret(ephPrivate, recipientPublicKey);
  const wrappingKey = hkdf(sha256, shared, ephPublic, new TextEncoder().encode("bkey-dek-wrap-v1"), 32);
  const nonce = randomBytes(12);
  const ciphertext = gcm(wrappingKey, nonce).encrypt(dek);
  return Buffer.from(concat(ephPublic, nonce, ciphertext)).toString("base64");
}

/**
 * Unwrap a DEK using the recipient's private key.
 */
export function unwrapDek(wrapped: string, privateKey: Uint8Array): Uint8Array {
  const bytes = Buffer.from(wrapped, "base64");
  const ephPublic = bytes.subarray(0, 32);
  const nonce = bytes.subarray(32, 44);
  const ciphertext = bytes.subarray(44);
  const shared = x25519.getSharedSecret(privateKey, ephPublic);
  const wrappingKey = hkdf(sha256, shared, ephPublic, new TextEncoder().encode("bkey-dek-wrap-v1"), 32);
  return gcm(wrappingKey, nonce).decrypt(ciphertext);
}

// --- Field encryption (AES-256-GCM) ---

/**
 * Encrypt a plaintext string with the DEK.
 * Output: base64( nonce[12] || ciphertext )
 */
export function encryptField(value: string, dek: Uint8Array): string {
  const nonce = randomBytes(12);
  const plaintext = new TextEncoder().encode(value);
  const ciphertext = gcm(dek, nonce).encrypt(plaintext);
  return Buffer.from(concat(nonce, ciphertext)).toString("base64");
}

/**
 * Decrypt an encrypted field produced by encryptField.
 */
export function decryptField(encrypted: string, dek: Uint8Array): string {
  const bytes = Buffer.from(encrypted, "base64");
  const nonce = bytes.subarray(0, 12);
  const ciphertext = bytes.subarray(12);
  const plaintext = gcm(dek, nonce).decrypt(ciphertext);
  return new TextDecoder().decode(plaintext);
}

// --- Helpers ---

function concat(...arrays: Uint8Array[]): Uint8Array {
  const total = arrays.reduce((n, a) => n + a.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const a of arrays) { out.set(a, offset); offset += a.length; }
  return out;
}
