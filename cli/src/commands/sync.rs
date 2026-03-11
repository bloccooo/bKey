use envilib::{
    config::read_config,
    crypto::derive_private_key,
    error::{Error, Result},
    store::{unlock, Store},
};

pub async fn run() -> Result<()> {
    let config = read_config().await?.ok_or(Error::NoConfig)?;

    if config.workspaces.is_empty() {
        return Err(Error::NoWorkspaces);
    }

    for workspace in &config.workspaces {
        print!("Syncing workspace '{}'... ", workspace.name);
        let store = Store::new(&workspace.id, &config.member_id, &workspace.storage)?;
        match store.pull().await {
            Ok(mut doc) => {
                // Verify we can still unlock
                let private_key = derive_private_key(&config.passphrase, &workspace.id)?;
                match unlock(&doc, &private_key) {
                    Ok(_) => {
                        store.persist(&mut doc).await?;
                        println!("ok");
                    }
                    Err(e) => println!("warning: {e}"),
                }
            }
            Err(e) => println!("error: {e}"),
        }
    }

    Ok(())
}
