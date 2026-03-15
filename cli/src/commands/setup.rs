use autosurgeon::{hydrate, reconcile};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use console::{style, Term};
use dialoguer::{Input, Password, Select};
use lib::{
    config::{read_config, write_config, EnviConfig, VaultConfig},
    crypto::{
        compute_key_mac, derive_private_key, derive_signing_key, generate_dek, get_public_key,
        wrap_dek,
    },
    error::Result,
    invite::parse_invite,
    storage::StorageConfig,
    store::Store,
    types::{EnviDocument, Member},
};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use uuid::Uuid;

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

fn done(pb: ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {}", style("✓").green().bold(), msg));
}

pub async fn run(invite_token_arg: Option<String>) -> Result<()> {
    let term = Term::stdout();
    let _ = term.clear_screen();

    println!();
    for line in [
        " ███████╗███╗   ██╗██╗   ██╗██╗",
        " ██╔════╝████╗  ██║██║   ██║██║",
        " █████╗  ██╔██╗ ██║██║   ██║██║",
        " ██╔══╝  ██║╚██╗██║╚██╗ ██╔╝██║",
        " ███████╗██║ ╚████║ ╚████╔╝ ██║",
        " ╚══════╝╚═╝  ╚═══╝  ╚═══╝  ╚═╝",
    ] {
        println!("  {}", style(line).cyan().bold());
    }
    println!();
    println!("  {}", style("serverless secret manager for teams").dim());
    println!("  {}", style("─".repeat(38)).dim());
    println!();

    let mut config = read_config().await?;

    // First-time setup: collect member name
    if config.is_none() {
        println!(
            "  {} {}",
            style("→").cyan(),
            style("Choose a device name.").bold()
        );
        println!("  {}", style("A label to identify this device.").dim());
        println!();
        let member_name: String = Input::new()
            .with_prompt(format!("  {}", style("Device name").bold()))
            .interact_text()
            .map_err(|e| lib::error::Error::Other(e.to_string()))?;

        let cfg = EnviConfig {
            version: "v1".to_string(),
            member_name: member_name.clone(),
            member_id: Uuid::new_v4().to_string(),
            vaults: vec![],
        };
        write_config(&cfg).await?;
        config = Some(cfg);
        println!();
    }

    let mut config = config.unwrap();

    println!(
        "  {} {}",
        style("→").cyan(),
        style("Set a passphrase to protect your keys.").bold()
    );
    println!("  {}", style("This never leaves your device.").dim());
    println!();
    println!(
        "  {} {}",
        style("⚠").yellow(),
        style("Losing your passphrase may result in permanently losing access to your vault content.").yellow()
    );
    println!(
        "  {}",
        style("Memorise it or store it in a secure password manager. Never save it unencrypted on any device.").dim()
    );
    println!();

    let passphrase = crate::passphrase::prompt_new_passphrase()?;
    println!();

    // Choose: create new vault or join via invite
    let action = if invite_token_arg.is_some() {
        1 // import
    } else {
        println!(
            "  {} {}",
            style("→").cyan(),
            style("What would you like to do?").bold()
        );
        println!();
        Select::new()
            .with_prompt(format!("  {}", style("Action").bold()))
            .items(&[
                "Create a new vault",
                "Join an existing vault (invite token)",
            ])
            .default(0)
            .interact()
            .map_err(|e| lib::error::Error::Other(e.to_string()))?
    };

    println!();

    if action == 1 {
        // Join via invite link
        let invite_token = if let Some(link) = invite_token_arg {
            link
        } else {
            println!(
                "  {} {}",
                style("→").cyan(),
                style("Paste your invite token below.").bold()
            );
            println!();
            Input::new()
                .with_prompt(format!("  {}", style("Invite link").bold()))
                .interact_text()
                .map_err(|e| lib::error::Error::Other(e.to_string()))?
        };

        let payload = parse_invite(&invite_token)?;

        let pb = spinner(&format!(
            "Connecting to vault '{}'…",
            payload.vault.name
        ));
        let store = Store::new(&payload.vault.id, &config.member_id, &payload.storage)?;
        let mut doc = store.pull().await?;
        done(pb, "Connected");

        let private_key =
            derive_private_key(&passphrase, &payload.vault.id, &config.member_id)?;
        let public_key = get_public_key(&private_key);
        let signing_key = derive_signing_key(&private_key);
        let signing_public_key = B64.encode(signing_key.verifying_key().to_bytes());

        config.vaults.push(VaultConfig {
            id: payload.vault.id.clone(),
            name: payload.vault.name.clone(),
            storage: payload.storage,
        });
        write_config(&config).await?;

        // Add ourselves as a pending member (key_mac set by granter who knows the DEK)
        let mut state: EnviDocument = hydrate(&doc)?;
        state.members.insert(
            config.member_id.clone(),
            Member {
                id: config.member_id.clone(),
                email: config.member_name.clone(),
                public_key: B64.encode(public_key),
                signing_key: signing_public_key,
                key_mac: String::new(),     // set by granter when wrapping DEK
                wrapped_dek: String::new(), // Pending — existing member must grant access
            },
        );
        reconcile(&mut doc, &state)?;

        let pb = spinner("Registering your keys…");
        store.persist(&mut doc, &signing_key).await?;
        done(pb, "Registered");

        println!();
        println!(
            "  {} Joined {}",
            style("✓").green().bold(),
            style(&payload.vault.name).cyan().bold(),
        );
        println!(
            "  {} An existing member needs to sync and grant you access.",
            style("i").dim(),
        );
    } else {
        // Create new vault
        println!(
            "  {} {}",
            style("→").cyan(),
            style("Name your vault.").bold()
        );
        println!();

        let vault_name: String = Input::new()
            .with_prompt(format!("  {}", style("Vault name").bold()))
            .interact_text()
            .map_err(|e| lib::error::Error::Other(e.to_string()))?;

        println!();
        println!(
            "  {} {}",
            style("→").cyan(),
            style("Choose where to store encrypted data.").bold()
        );
        println!();

        let storage_config = collect_storage_config()?;
        let vault_id = Uuid::now_v7().to_string();

        let store = Store::new(&vault_id, &config.member_id, &storage_config)?;

        config.vaults.push(VaultConfig {
            id: vault_id.clone(),
            name: vault_name.clone(),
            storage: storage_config,
        });
        write_config(&config).await?;

        let pb = spinner("Initializing vault…");
        let mut doc = store.pull().await?;
        done(pb, "Storage ready");

        let private_key = derive_private_key(&passphrase, &vault_id, &config.member_id)?;
        let public_key = get_public_key(&private_key);
        let signing_key = derive_signing_key(&private_key);
        let signing_public_key = B64.encode(signing_key.verifying_key().to_bytes());
        let dek = generate_dek();
        let wrapped_dek = wrap_dek(&dek, &public_key)?;
        let public_key_b64 = B64.encode(public_key);
        let key_mac = compute_key_mac(
            &dek,
            &config.member_id,
            &public_key_b64,
            &signing_public_key,
        );

        let mut state: EnviDocument = hydrate(&doc)?;
        state.id = vault_id;
        state.name = vault_name.clone();
        state.members.insert(
            config.member_id.clone(),
            Member {
                id: config.member_id.clone(),
                email: config.member_name.clone(),
                public_key: public_key_b64,
                signing_key: signing_public_key,
                key_mac,
                wrapped_dek,
            },
        );
        reconcile(&mut doc, &state)?;

        let pb = spinner("Encrypting and saving…");
        store.persist(&mut doc, &signing_key).await?;
        done(pb, "Saved");

        println!();
        println!(
            "  {} Vault {} created!",
            style("✓").green().bold(),
            style(&vault_name).cyan().bold(),
        );
        println!(
            "  {} Run {} to manage your secrets.",
            style("i").dim(),
            style("envi ui").cyan(),
        );
    }

    println!();
    Ok(())
}

fn collect_storage_config() -> Result<StorageConfig> {
    let backends = [
        "Local filesystem",
        "S3-compatible  (AWS, MinIO, B2…)",
        "Cloudflare R2",
        "WebDAV",
        "GitHub",
    ];
    let choice = Select::new()
        .with_prompt(format!("  {}", style("Storage backend").bold()))
        .items(&backends)
        .default(0)
        .interact()
        .map_err(|e| lib::error::Error::Other(e.to_string()))?;

    println!();

    match choice {
        0 => {
            let root: String = Input::new()
                .with_prompt(format!("  {}", style("Storage path").bold()))
                .default("./envi-storage".to_string())
                .interact_text()
                .map_err(|e| lib::error::Error::Other(e.to_string()))?;
            Ok(StorageConfig::Fs(lib::storage::FsConfig { root }))
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
            Ok(StorageConfig::S3(lib::storage::S3Config {
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
            Ok(StorageConfig::R2(lib::storage::R2Config {
                account_id,
                bucket,
                access_key_id,
                secret_access_key,
            }))
        }
        3 => {
            let endpoint = prompt("WebDAV endpoint URL")?;
            let username = prompt_optional("Username (blank if none)")?;
            let password = prompt_optional_password("Password (blank if none)")?;
            Ok(StorageConfig::Webdav(lib::storage::WebdavConfig {
                endpoint,
                username,
                password,
            }))
        }
        _ => {
            let token = prompt_password("GitHub personal access token")?;
            let owner = prompt("Repository owner (user or org)")?;
            let repo = prompt("Repository name")?;
            let root_str = prompt_optional("Root path in repo (blank for repo root)")?;
            let root = if root_str.is_empty() {
                None
            } else {
                Some(root_str)
            };
            Ok(StorageConfig::Github(lib::storage::GithubConfig {
                token,
                owner,
                repo,
                root,
            }))
        }
    }
}

fn prompt(msg: &str) -> Result<String> {
    Input::new()
        .with_prompt(format!("  {}", style(msg).bold()))
        .interact_text()
        .map_err(|e| lib::error::Error::Other(e.to_string()))
}

fn prompt_default(msg: &str, default: &str) -> Result<String> {
    Input::new()
        .with_prompt(format!("  {}", style(msg).bold()))
        .default(default.to_string())
        .interact_text()
        .map_err(|e| lib::error::Error::Other(e.to_string()))
}

fn prompt_optional(msg: &str) -> Result<String> {
    Input::new()
        .with_prompt(format!("  {}", style(msg).bold()))
        .allow_empty(true)
        .interact_text()
        .map_err(|e| lib::error::Error::Other(e.to_string()))
}

fn prompt_password(msg: &str) -> Result<String> {
    Password::new()
        .with_prompt(format!("  {}", style(msg).bold()))
        .interact()
        .map_err(|e| lib::error::Error::Other(e.to_string()))
}

fn prompt_optional_password(msg: &str) -> Result<String> {
    Password::new()
        .with_prompt(format!("  {}", style(msg).bold()))
        .allow_empty_password(true)
        .interact()
        .map_err(|e| lib::error::Error::Other(e.to_string()))
}
