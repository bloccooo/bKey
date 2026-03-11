use crate::{error::{Error, Result}, storage::StorageConfig};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use serde::{Deserialize, Serialize};

const INVITE_PREFIX: &str = "envi-invite:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePayload {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePayload {
    pub workspace: WorkspacePayload,
    pub storage: StorageConfig,
}

pub fn generate_invite(storage: &StorageConfig, workspace: WorkspacePayload) -> Result<String> {
    let payload = InvitePayload {
        workspace,
        storage: storage.clone(),
    };
    let json = serde_json::to_string(&payload)?;
    let b64 = B64URL.encode(json.as_bytes());
    Ok(format!("{INVITE_PREFIX}{b64}"))
}

pub fn parse_invite(link: &str) -> Result<InvitePayload> {
    let b64 = link
        .strip_prefix(INVITE_PREFIX)
        .ok_or(Error::InvalidInviteLink)?;
    let bytes = B64URL.decode(b64).map_err(|_| Error::InvalidInviteLink)?;
    let json = String::from_utf8(bytes).map_err(|_| Error::InvalidInviteLink)?;
    serde_json::from_str(&json).map_err(|_| Error::InvalidInviteLink)
}
