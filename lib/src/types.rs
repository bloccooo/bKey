use autosurgeon::{Hydrate, Reconcile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Automerge document — the entire workspace.
/// Collections are maps keyed by UUID for stable CRDT keys.
/// Secret fields (name, value, description, tags) are AES-256-GCM encrypted
/// using the workspace DEK. The DEK itself is X25519/ECIES-wrapped per member.
#[derive(Debug, Clone, Reconcile, Hydrate, Default)]
pub struct EnviDocument {
    pub id: String,
    pub name: String,
    pub doc_version: u64,
    pub members: HashMap<String, Member>,
    pub secrets: HashMap<String, Secret>,
    /// Ed25519 signature over the canonical document bytes (excluding this field).
    /// Format: "member_id:base64(signature)". Empty on unsigned documents.
    pub document_signature: String,
}

#[derive(Debug, Clone, Reconcile, Hydrate, Default)]
pub struct Member {
    pub id: String,
    pub email: String,
    /// Base64-encoded X25519 public key
    pub public_key: String,
    /// ECIES-wrapped DEK; empty string = pending access
    pub wrapped_dek: String,
    /// Base64-encoded Ed25519 verifying key; empty string = old client / pending
    pub signing_key: String,
    /// HMAC-SHA256(DEK, member_id || ":" || public_key || ":" || signing_key).
    /// Allows any DEK-holder to verify public keys haven't been tampered with.
    /// Empty string = pending (set by granter when wrapping the DEK).
    pub key_mac: String,
}

#[derive(Debug, Clone, Reconcile, Hydrate, Default)]
pub struct Secret {
    pub id: String,
    /// base64(nonce[12] || ciphertext) — AES-256-GCM encrypted
    pub name: String,
    pub value: String,
    pub description: String,
    /// Encrypted JSON array of tag strings
    pub tags: String,
}

/// In-memory plaintext view of a secret (after decryption)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaintextSecret {
    pub id: String,
    pub name: String,
    pub value: String,
    pub description: String,
    pub tags: Vec<String>,
}
