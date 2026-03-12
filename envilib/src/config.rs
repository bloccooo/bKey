use crate::{error::Result, keychain, storage::StorageConfig};
use serde::{Deserialize, Serialize};

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
    pub workspaces: Vec<WorkspaceConfig>,
}

pub async fn read_config() -> Result<Option<EnviConfig>> {
    keychain::load_keychain()
}

pub async fn write_config(config: &EnviConfig) -> Result<()> {
    keychain::save_keychain(config)
}

pub async fn delete_config() -> Result<()> {
    keychain::delete_keychain()
}
