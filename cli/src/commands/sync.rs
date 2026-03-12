use envilib::{
    config::read_config,
    error::{Error, Result},
    store::Store,
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
                store.persist(&mut doc).await?;
                println!("ok");
            }
            Err(e) => println!("error: {e}"),
        }
    }

    Ok(())
}
