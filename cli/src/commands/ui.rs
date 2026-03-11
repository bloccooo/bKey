use envilib::{
    config::read_config,
    crypto::derive_private_key,
    error::{Error, Result},
    invite::{generate_invite, WorkspacePayload},
    store::{unlock, Store},
};

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
    let private_key = derive_private_key(&config.passphrase, &workspace.id)?;
    let session = unlock(&doc, &private_key)?;

    let invite_link = generate_invite(
        &workspace.storage,
        WorkspacePayload {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
        },
    )?;

    crate::tui::run(doc, store, session, invite_link).await
}
