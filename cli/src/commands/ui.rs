use lib::{
    config::read_config,
    crypto::derive_private_key,
    error::{Error, Result},
    invite::{generate_invite, VaultPayload},
    storage::StorageConfig,
    store::{unlock, Store},
};

use crate::passphrase::prompt_passphrase;

pub async fn run() -> Result<()> {
    let config = read_config().await?.ok_or(Error::NoConfig)?;

    if config.vaults.is_empty() {
        return Err(Error::NoVaults);
    }

    let vault = if config.vaults.len() == 1 {
        config.vaults.into_iter().next().unwrap()
    } else {
        let names: Vec<_> = config.vaults.iter().map(|w| w.name.as_str()).collect();
        let idx = dialoguer::Select::new()
            .with_prompt("Select vault")
            .items(&names)
            .default(0)
            .interact()
            .map_err(|e| Error::Other(e.to_string()))?;
        config.vaults.into_iter().nth(idx).unwrap()
    };

    let store = Store::new(&vault.id, &config.member_id, &vault.storage)?;
    let doc = store.pull().await?;
    let agent = crate::agent::AgentClient::connect_or_start();
    let private_key = if let Some(ref agent) = agent {
        if let Some(key) = agent.get_key(&vault.id) {
            key
        } else {
            derive_private_key(&prompt_passphrase()?, &vault.id, &config.member_id)?
        }
    } else {
        derive_private_key(&prompt_passphrase()?, &vault.id, &config.member_id)?
    };
    let session = unlock(&doc, &private_key)?;
    if let Some(ref agent) = agent {
        agent.store_key(&vault.id, &private_key);
    }

    let invite_token = generate_invite(
        &vault.storage,
        VaultPayload {
            id: vault.id.clone(),
            name: vault.name.clone(),
        },
    )?;

    let storage_backend = match &vault.storage {
        StorageConfig::Fs(_) => "Local FS",
        StorageConfig::S3(_) => "S3",
        StorageConfig::R2(_) => "R2",
        StorageConfig::Webdav(_) => "WebDAV",
        StorageConfig::Github(_) => "GitHub",
    }.to_string();

    crate::tui::run(doc, store, session, invite_token, config.member_name, vault.name, storage_backend).await
}
