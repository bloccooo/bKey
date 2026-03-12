use envilib::{config::delete_config, error::Result, store::cache_dir};

pub async fn run() -> Result<()> {
    // Kill the agent if running
    if let Some(client) = crate::agent::AgentClient::connect() {
        client.kill();
        println!("agent stopped");
    }

    // Remove the local cache directory
    let cache = cache_dir();
    if cache.exists() {
        std::fs::remove_dir_all(&cache)
            .map_err(|e| envilib::error::Error::Other(format!("failed to remove cache: {e}")))?;
        println!("cache cleared");
    }

    // Remove config from keychain
    delete_config().await?;
    println!("config removed");

    println!("all local data cleared");
    Ok(())
}
