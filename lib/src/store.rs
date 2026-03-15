use crate::{
    crypto::{
        canonical_document_bytes, derive_signing_key, get_public_key, sign_document,
        unwrap_dek, verify_document_signature, verify_key_mac,
    },
    error::{Error, Result},
    storage::{pull_prefix, push_path, StorageBackend, StorageConfig},
    types::EnviDocument,
};
use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use directories::ProjectDirs;
use std::path::PathBuf;
use tokio::time::{timeout, Duration};

const REMOTE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Store {
    workspace_id: String,
    member_id: String,
    remote: StorageBackend,
    local: StorageBackend,
}

impl Store {
    pub fn new(workspace_id: &str, member_id: &str, storage: &StorageConfig) -> Result<Self> {
        let remote = StorageBackend::new(storage)?;

        let cache_root = cache_dir();
        let local_config = crate::storage::StorageConfig::Fs(crate::storage::FsConfig {
            root: cache_root.to_string_lossy().into_owned(),
        });
        let local = StorageBackend::new(&local_config)?;

        Ok(Self {
            workspace_id: workspace_id.to_string(),
            member_id: member_id.to_string(),
            remote,
            local,
        })
    }

    pub async fn pull(&self) -> Result<AutoCommit> {
        let prefix = pull_prefix(&self.workspace_id);

        let local_doc = self.load_local(&prefix).await;

        let remote_doc = match timeout(REMOTE_TIMEOUT, self.remote.pull(&prefix)).await {
            Ok(Ok(files)) => load_and_verify_files(files),
            _ => None,
        };

        let doc = match (local_doc, remote_doc) {
            (Some(mut l), Some(mut r)) => {
                l.merge(&mut r)?;
                l
            }
            (Some(d), None) | (None, Some(d)) => d,
            (None, None) => init_doc(&self.workspace_id),
        };

        Ok(doc)
    }

    /// Sign the document then push to local cache and remote storage.
    pub async fn persist(
        &self,
        doc: &mut AutoCommit,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<()> {
        // Sign: compute canonical bytes, produce signature, store in document
        let mut state: EnviDocument = hydrate(doc as &AutoCommit)?;
        let canonical = canonical_document_bytes(&state);
        state.document_signature = sign_document(&canonical, &self.member_id, signing_key);
        reconcile(doc, &state)?;

        let data = doc.save();
        let push = push_path(&self.workspace_id, &self.member_id);

        // Always write to local cache
        self.local.push(&push, data.clone()).await?;

        // Best-effort remote push with timeout
        let _ = timeout(REMOTE_TIMEOUT, self.remote.push(&push, data)).await;

        Ok(())
    }

    async fn load_local(&self, prefix: &str) -> Option<AutoCommit> {
        match self.local.pull(prefix).await {
            Ok(files) => load_and_verify_files(files),
            Err(_) => None,
        }
    }
}

/// Load each file, verify its document signature, and merge the valid ones.
/// Files without a signature or with an invalid signature are skipped with a warning.
fn load_and_verify_files(files: Vec<Vec<u8>>) -> Option<AutoCommit> {
    if files.is_empty() {
        return None;
    }

    let docs: Vec<AutoCommit> = files
        .into_iter()
        .filter_map(|bytes| {
            let doc = AutoCommit::load(&bytes).ok()?;
            let state: EnviDocument = hydrate(&doc).ok()?;

            if state.document_signature.is_empty() {
                eprintln!("warning: skipping unsigned member file");
                return None;
            }

            // Parse "member_id:base64_sig" and look up the signer's key
            let member_id = state.document_signature.splitn(2, ':').next()?;
            let member = state.members.get(member_id)?;

            if member.signing_key.is_empty() {
                eprintln!(
                    "warning: skipping file signed by member {member_id} with no registered signing key"
                );
                return None;
            }

            let canonical = canonical_document_bytes(&state);
            match verify_document_signature(&canonical, &state.document_signature, &member.signing_key) {
                Ok(()) => Some(doc),
                Err(_) => {
                    eprintln!(
                        "warning: skipping member file with invalid signature (member {member_id})"
                    );
                    None
                }
            }
        })
        .collect();

    if docs.is_empty() {
        return None;
    }

    docs.into_iter().reduce(|mut a, mut b| {
        let _ = a.merge(&mut b);
        a
    })
}

fn init_doc(workspace_id: &str) -> AutoCommit {
    let mut doc = AutoCommit::new();
    let state = EnviDocument {
        id: workspace_id.to_string(),
        name: String::new(),
        doc_version: 0,
        members: Default::default(),
        secrets: Default::default(),
        document_signature: String::new(),
    };
    reconcile(&mut doc, &state).expect("init_doc reconcile failed");
    doc
}

pub fn cache_dir() -> PathBuf {
    ProjectDirs::from("", "", "envi")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".envi-cache"))
}

// --- Session ---

pub struct Session {
    pub member_id: String,
    pub dek: [u8; 32],
    pub signing_key: ed25519_dalek::SigningKey,
}

/// Unlock the document for the given private key.
/// Verifies all member key MACs using the decrypted DEK.
pub fn unlock(doc: &AutoCommit, private_key: &[u8; 32]) -> Result<Session> {
    let state: EnviDocument = hydrate(doc)?;

    let public_key = get_public_key(private_key);
    let pub_key_b64 = B64.encode(public_key);

    let member = state
        .members
        .values()
        .find(|m| m.public_key == pub_key_b64)
        .ok_or(Error::NotAMember)?;

    if member.wrapped_dek.is_empty() {
        return Err(Error::AccessPending);
    }

    let dek = unwrap_dek(&member.wrapped_dek, private_key)?;

    // Verify key MACs for all members that have one
    for m in state.members.values() {
        if m.key_mac.is_empty() {
            continue; // pending member or old client — skip
        }
        verify_key_mac(&dek, &m.id, &m.public_key, &m.signing_key, &m.key_mac)
            .map_err(|_| Error::InvalidKeyMac(m.id.clone()))?;
    }

    let signing_key = derive_signing_key(private_key);

    Ok(Session {
        member_id: member.id.clone(),
        dek,
        signing_key,
    })
}
