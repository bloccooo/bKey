use crate::error::{Error, Result};
use futures::StreamExt;
use opendal::{services, Operator};
use serde::{Deserialize, Serialize};

const DOC_EXTENSION: &str = "envi.enc";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    Fs(FsConfig),
    S3(S3Config),
    R2(R2Config),
    Webdav(WebdavConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsConfig {
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2Config {
    pub account_id: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebdavConfig {
    pub endpoint: String,
    pub username: String,
    pub password: String,
}

pub fn build_operator(config: &StorageConfig) -> Result<Operator> {
    let op = match config {
        StorageConfig::Fs(c) => {
            let builder = services::Fs::default().root(&c.root);
            Operator::new(builder).map_err(Error::Storage)?.finish()
        }
        StorageConfig::S3(c) => {
            let mut builder = services::S3::default()
                .bucket(&c.bucket)
                .region(&c.region)
                .access_key_id(&c.access_key_id)
                .secret_access_key(&c.secret_access_key);
            if let Some(ep) = &c.endpoint {
                builder = builder.endpoint(ep);
            }
            Operator::new(builder).map_err(Error::Storage)?.finish()
        }
        StorageConfig::R2(c) => {
            let endpoint = format!("https://{}.r2.cloudflarestorage.com", c.account_id);
            let builder = services::S3::default()
                .bucket(&c.bucket)
                .region("auto")
                .endpoint(&endpoint)
                .access_key_id(&c.access_key_id)
                .secret_access_key(&c.secret_access_key);
            Operator::new(builder).map_err(Error::Storage)?.finish()
        }
        StorageConfig::Webdav(c) => {
            let mut builder = services::Webdav::default().endpoint(&c.endpoint);
            if !c.username.is_empty() {
                builder = builder.username(&c.username);
            }
            if !c.password.is_empty() {
                builder = builder.password(&c.password);
            }
            Operator::new(builder).map_err(Error::Storage)?.finish()
        }
    };
    Ok(op)
}

pub struct StorageBackend {
    op: Operator,
}

impl StorageBackend {
    pub fn new(config: &StorageConfig) -> Result<Self> {
        Ok(Self {
            op: build_operator(config)?,
        })
    }

    pub async fn push(&self, path: &str, data: Vec<u8>) -> Result<()> {
        self.op
            .write(path, data)
            .await
            .map(|_| ())
            .map_err(Error::Storage)
    }

    pub async fn pull(&self, prefix: &str) -> Result<Vec<Vec<u8>>> {
        let entries = match self.op.list(prefix).await {
            Ok(e) => e,
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(Error::Storage(e)),
        };

        let doc_entries: Vec<_> = entries
            .into_iter()
            .filter(|e| e.path().ends_with(&format!(".{DOC_EXTENSION}")))
            .collect();

        const MAX_CONCURRENT: usize = 8;

        let results = futures::stream::iter(doc_entries)
            .map(|e| {
                let path = e.path().to_owned();
                async move { self.op.read(&path).await }
            })
            .buffer_unordered(MAX_CONCURRENT)
            .filter_map(|r| async move {
                match r {
                    Ok(buf) => Some(buf.to_bytes().to_vec()),
                    Err(e) if e.kind() == opendal::ErrorKind::NotFound => None,
                    Err(_) => None,
                }
            })
            .collect()
            .await;

        Ok(results)
    }

    pub async fn check(&self) -> bool {
        self.op.check().await.is_ok()
    }
}

pub fn push_path(workspace_id: &str, member_id: &str) -> String {
    format!("_envi/{workspace_id}/{member_id}.{DOC_EXTENSION}")
}

pub fn pull_prefix(workspace_id: &str) -> String {
    format!("_envi/{workspace_id}/")
}
