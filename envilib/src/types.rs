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
    pub projects: HashMap<String, Project>,
    pub secrets: HashMap<String, Secret>,
}

#[derive(Debug, Clone, Reconcile, Hydrate, Default)]
pub struct Member {
    pub id: String,
    pub email: String,
    /// Base64-encoded X25519 public key
    pub public_key: String,
    /// ECIES-wrapped DEK; empty string = pending access
    pub wrapped_dek: String,
}

#[derive(Debug, Clone, Reconcile, Hydrate, Default)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub secret_ids: Vec<String>,
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
