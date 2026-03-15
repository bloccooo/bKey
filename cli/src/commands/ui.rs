use lib::{
    config::read_config,
    crypto::derive_private_key,
    error::{Error, Result},
    invite::{generate_invite, WorkspacePayload},
    storage::StorageConfig,
    store::{unlock, Store},
};

use crate::passphrase::prompt_passphrase;

pub async fn run() -> Result<()> {
    let config = read_config().await?.ok_or(Error::NoConfig)?;

    if config.workspaces.is_empty() {
        return Err(Error::NoWorkspaces);
    }

    let workspace = if config.workspaces.len() == 1 {
        config.workspaces.into_iter().next().unwrap()
    } else {
        let names: Vec<_> = config.workspaces.iter().map(|w| w.name.as_str()).collect();
        let idx = dialoguer::Select::new()
            .with_prompt("Select workspace")
            .items(&names)
            .default(0)
            .interact()
            .map_err(|e| Error::Other(e.to_string()))?;
        config.workspaces.into_iter().nth(idx).unwrap()
    };

    let store = Store::new(&workspace.id, &config.member_id, &workspace.storage)?;
    let doc = store.pull().await?;
    let agent = crate::agent::AgentClient::connect_or_start();
    let private_key = if let Some(ref agent) = agent {
        if let Some(key) = agent.get_key(&workspace.id) {
            key
        } else {
            derive_private_key(&prompt_passphrase()?, &workspace.id, &config.member_id)?
        }
    } else {
        derive_private_key(&prompt_passphrase()?, &workspace.id, &config.member_id)?
    };
    let session = unlock(&doc, &private_key)?;
    if let Some(ref agent) = agent {
        agent.store_key(&workspace.id, &private_key);
    }

    let invite_link = generate_invite(
        &workspace.storage,
        WorkspacePayload {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
        },
    )?;

    let storage_backend = match &workspace.storage {
        StorageConfig::Fs(_) => "Local FS",
        StorageConfig::S3(_) => "S3",
        StorageConfig::R2(_) => "R2",
        StorageConfig::Webdav(_) => "WebDAV",
        StorageConfig::Github(_) => "GitHub",
    }.to_string();

    crate::tui::run(doc, store, session, invite_link, config.member_name, workspace.name, storage_backend).await
}
