use lib::{
    config::read_config,
    crypto::derive_private_key,
    envi_file::read_envi_file,
    error::{Error, Result},
    secrets::list_secrets,
    store::{unlock, Store},
};

use crate::passphrase::prompt_passphrase;

pub async fn run(tag_arg: Option<String>, dry_run: bool, cmd: Vec<String>) -> Result<()> {
    // Resolve tag filter: flag → .envi file → all secrets
    let tag_filter = if tag_arg.is_some() {
        tag_arg
    } else {
        read_envi_file(".").await?.tag
    };

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

    let all_secrets = list_secrets(&doc, &session.dek)?;

    let env_vars: Vec<(String, String)> = all_secrets
        .into_iter()
        .filter(|s| {
            tag_filter
                .as_ref()
                .map(|tag| s.tags.iter().any(|t| t == tag))
                .unwrap_or(true)
        })
        .map(|s| (s.name, s.value))
        .collect();

    if dry_run {
        let label = tag_filter
            .as_deref()
            .map(|t| format!("tag \"{t}\""))
            .unwrap_or_else(|| "all secrets".to_string());
        println!("\nEnv vars that would be injected ({label}):\n");
        for (k, v) in &env_vars {
            println!("  {k}={v}");
        }
        println!();
        return Ok(());
    }

    if cmd.is_empty() {
        return Err(Error::Other(
            "no command given. Usage: envi exec [options] -- <command>".to_string(),
        ));
    }

    let status = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .envs(std::env::vars())
        .envs(env_vars)
        .status()
        .map_err(|e| Error::Other(format!("failed to run command: {e}")))?;

    std::process::exit(status.code().unwrap_or(1));
}
