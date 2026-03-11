use autosurgeon::hydrate;
use envilib::{
    config::read_config,
    crypto::derive_private_key,
    envi_file::read_envi_file,
    error::{Error, Result},
    secrets::list_secrets,
    store::{unlock, Store},
    types::EnviDocument,
};

pub async fn run(project_arg: Option<String>, dry_run: bool, cmd: Vec<String>) -> Result<()> {
    // Resolve project name: flag → .envi file → all secrets
    let project_name = if project_arg.is_some() {
        project_arg
    } else {
        read_envi_file(".").await?.project
    };

    let config = read_config().await?.ok_or(Error::NoConfig)?;

    if config.workspaces.is_empty() {
        return Err(Error::NoWorkspaces);
    }

    let workspace = if config.workspaces.len() == 1 {
        config.workspaces.into_iter().next().unwrap()
    } else {
        // Multiple workspaces: prompt
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

    let all_secrets = list_secrets(&doc, &session.dek)?;
    let state: EnviDocument = hydrate(&doc)?;

    // Build env vars from project's secrets (or all secrets if no project)
    let mut env_vars: Vec<(String, String)> = Vec::new();

    if let Some(name) = &project_name {
        let project = state
            .projects
            .values()
            .find(|p| p.name == *name)
            .ok_or_else(|| Error::ProjectNotFound(name.clone()))?;

        for id in &project.secret_ids {
            if let Some(secret) = all_secrets.iter().find(|s| &s.id == id) {
                env_vars.push((secret.name.clone(), secret.value.clone()));
            }
        }
    } else {
        for secret in &all_secrets {
            env_vars.push((secret.name.clone(), secret.value.clone()));
        }
    }

    if dry_run {
        let label = project_name
            .as_deref()
            .map(|n| format!("project \"{n}\""))
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
            "no command given. Usage: envi run [options] -- <command>".to_string(),
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
