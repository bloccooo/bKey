use crate::{error::{Error, Result}, storage::StorageConfig};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnviConfig {
    pub version: String,
    pub member_name: String,
    pub member_id: String,
    /// Passphrase stored in plaintext for now (keychain integration planned)
    pub passphrase: String,
    pub workspaces: Vec<WorkspaceConfig>,
}

fn config_path() -> PathBuf {
    ProjectDirs::from("", "", "envi")
        .map(|d| d.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from(".envi-config.json"))
}

pub async fn read_config() -> Result<Option<EnviConfig>> {
    let path = config_path();
    match fs::read_to_string(&path).await {
        Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Io(e)),
    }
}

pub async fn write_config(config: &EnviConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(&path, json).await?;
    Ok(())
}
