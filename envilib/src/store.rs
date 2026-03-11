use crate::{
    crypto::{get_public_key, unwrap_dek},
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
            Ok(Ok(files)) => merge_files(files),
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

    pub async fn persist(&self, doc: &mut AutoCommit) -> Result<()> {
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
            Ok(files) => merge_files(files),
            Err(_) => None,
        }
    }
}

fn merge_files(files: Vec<Vec<u8>>) -> Option<AutoCommit> {
    if files.is_empty() {
        return None;
    }
    let mut docs: Vec<AutoCommit> = files
        .into_iter()
        .filter_map(|b| AutoCommit::load(&b).ok())
        .collect();

    if docs.is_empty() {
        return None;
    }

    let mut base = docs.remove(0);
    for mut other in docs {
        let _ = base.merge(&mut other);
    }
    Some(base)
}

fn init_doc(workspace_id: &str) -> AutoCommit {
    let mut doc = AutoCommit::new();
    let state = EnviDocument {
        id: workspace_id.to_string(),
        name: "my-workspace".to_string(),
        doc_version: 0,
        members: Default::default(),
        projects: Default::default(),
        secrets: Default::default(),
    };
    reconcile(&mut doc, &state).expect("init_doc reconcile failed");
    doc
}

fn cache_dir() -> PathBuf {
    ProjectDirs::from("", "", "envi")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".envi-cache"))
}

// --- Session ---

pub struct Session {
    pub member_id: String,
    pub dek: [u8; 32],
}

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
    Ok(Session {
        member_id: member.id.clone(),
        dek,
    })
}
