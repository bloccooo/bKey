use autosurgeon::{hydrate, reconcile};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use dialoguer::{Input, Password, Select};
use envilib::{
    config::{read_config, write_config, EnviConfig, WorkspaceConfig},
    crypto::{derive_private_key, generate_dek, get_public_key, wrap_dek},
    error::Result,
    invite::parse_invite,
    storage::StorageConfig,
    store::Store,
    types::{EnviDocument, Member},
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Derive a deterministic member ID from the member name (SHA-256 hash formatted as UUID).
fn member_id_from_name(name: &str) -> String {
    let hash = Sha256::digest(name.as_bytes());
    let h = hex::encode(hash);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub async fn run(invite_link_arg: Option<String>) -> Result<()> {
    println!("envi setup");

    let mut config = read_config().await?;

    // First-time setup: collect member name and passphrase
    if config.is_none() {
        let member_name: String = Input::new()
            .with_prompt("Member name")
            .interact_text()
            .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

        let passphrase: String = Password::new()
            .with_prompt("Passphrase")
            .interact()
            .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

        let cfg = EnviConfig {
            version: "v1".to_string(),
            member_name: member_name.clone(),
            member_id: member_id_from_name(&member_name),
            passphrase,
            workspaces: vec![],
        };
        write_config(&cfg).await?;
        config = Some(cfg);
    }

    let mut config = config.unwrap();

    // Choose: create new workspace or join via invite
    let action = if invite_link_arg.is_some() {
        1 // import
    } else {
        Select::new()
            .with_prompt("Initialize workspace")
            .items(&[
                "Create new workspace",
                "Join existing workspace (invite link)",
            ])
            .default(0)
            .interact()
            .map_err(|e| envilib::error::Error::Other(e.to_string()))?
    };

    if action == 1 {
        // Join via invite link
        let invite_link = if let Some(link) = invite_link_arg {
            link
        } else {
            Input::new()
                .with_prompt("Invite link")
                .interact_text()
                .map_err(|e| envilib::error::Error::Other(e.to_string()))?
        };

        let payload = parse_invite(&invite_link)?;

        let store = Store::new(&payload.workspace.id, &config.member_id, &payload.storage)?;
        let mut doc = store.pull().await?;

        let private_key = derive_private_key(&config.passphrase, &payload.workspace.id)?;
        let public_key = get_public_key(&private_key);

        config.workspaces.push(WorkspaceConfig {
            id: payload.workspace.id.clone(),
            name: payload.workspace.name.clone(),
            storage: payload.storage,
        });
        write_config(&config).await?;

        // Add ourselves as a pending member
        let mut state: EnviDocument = hydrate(&doc)?;
        state.members.insert(
            config.member_id.clone(),
            Member {
                id: config.member_id.clone(),
                email: config.member_name.clone(),
                public_key: B64.encode(public_key),
                wrapped_dek: String::new(), // Pending — existing member must grant access
            },
        );
        reconcile(&mut doc, &state)?;
        store.persist(&mut doc).await?;

        println!(
            "Joined workspace '{}'. An existing member needs to sync and grant you access.",
            payload.workspace.name
        );
    } else {
        // Create new workspace
        let workspace_name: String = Input::new()
            .with_prompt("Workspace name")
            .interact_text()
            .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

        let storage_config = collect_storage_config()?;
        let workspace_id = Uuid::now_v7().to_string();

        let store = Store::new(&workspace_id, &config.member_id, &storage_config)?;

        config.workspaces.push(WorkspaceConfig {
            id: workspace_id.clone(),
            name: workspace_name.clone(),
            storage: storage_config,
        });
        write_config(&config).await?;

        let mut doc = store.pull().await?;

        let private_key = derive_private_key(&config.passphrase, &workspace_id)?;
        let public_key = get_public_key(&private_key);
        let dek = generate_dek();
        let wrapped_dek = wrap_dek(&dek, &public_key)?;

        let mut state: EnviDocument = hydrate(&doc)?;
        state.id = workspace_id;
        state.name = workspace_name.clone();
        state.members.insert(
            config.member_id.clone(),
            Member {
                id: config.member_id.clone(),
                email: config.member_name.clone(),
                public_key: B64.encode(public_key),
                wrapped_dek,
            },
        );
        reconcile(&mut doc, &state)?;
        store.persist(&mut doc).await?;

        println!(
            "Workspace '{}' created. Run `envi ui` to manage secrets.",
            workspace_name
        );
    }

    Ok(())
}

fn collect_storage_config() -> Result<StorageConfig> {
    let backends = [
        "Local filesystem",
        "S3-compatible (AWS, MinIO, B2…)",
        "Cloudflare R2",
        "WebDAV",
    ];
    let choice = Select::new()
        .with_prompt("Storage backend")
        .items(&backends)
        .default(0)
        .interact()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

    match choice {
        0 => {
            let root: String = Input::new()
                .with_prompt("Storage path")
                .default("./envi-storage".to_string())
                .interact_text()
                .map_err(|e| envilib::error::Error::Other(e.to_string()))?;
            Ok(StorageConfig::Fs(envilib::storage::FsConfig { root }))
        }
        1 => {
            let bucket = prompt("Bucket name")?;
            let region = prompt_default("Region", "us-east-1")?;
            let endpoint_str = prompt_optional("Endpoint URL (blank for AWS)")?;
            let endpoint = if endpoint_str.is_empty() {
                None
            } else {
                Some(endpoint_str)
            };
            let access_key_id = prompt("Access Key ID")?;
            let secret_access_key = prompt_password("Secret Access Key")?;
            Ok(StorageConfig::S3(envilib::storage::S3Config {
                bucket,
                region,
                endpoint,
                access_key_id,
                secret_access_key,
            }))
        }
        2 => {
            let account_id = prompt("Cloudflare Account ID")?;
            let bucket = prompt("Bucket name")?;
            let access_key_id = prompt("R2 Access Key ID")?;
            let secret_access_key = prompt_password("R2 Secret Access Key")?;
            Ok(StorageConfig::R2(envilib::storage::R2Config {
                account_id,
                bucket,
                access_key_id,
                secret_access_key,
            }))
        }
        _ => {
            let endpoint = prompt("WebDAV endpoint URL")?;
            let username = prompt_optional("Username (blank if none)")?;
            let password = prompt_optional_password("Password (blank if none)")?;
            Ok(StorageConfig::Webdav(envilib::storage::WebdavConfig {
                endpoint,
                username,
                password,
            }))
        }
    }
}

fn prompt(msg: &str) -> Result<String> {
    Input::new()
        .with_prompt(msg)
        .interact_text()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))
}

fn prompt_default(msg: &str, default: &str) -> Result<String> {
    Input::new()
        .with_prompt(msg)
        .default(default.to_string())
        .interact_text()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))
}

fn prompt_optional(msg: &str) -> Result<String> {
    Input::new()
        .with_prompt(msg)
        .allow_empty(true)
        .interact_text()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))
}

fn prompt_password(msg: &str) -> Result<String> {
    Password::new()
        .with_prompt(msg)
        .interact()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))
}

fn prompt_optional_password(msg: &str) -> Result<String> {
    Password::new()
        .with_prompt(msg)
        .allow_empty_password(true)
        .interact()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))
}
