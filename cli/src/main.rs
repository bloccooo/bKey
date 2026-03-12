mod agent;
mod commands;
mod passphrase;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "envi", about = "Encrypted team secret manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Set up a new workspace or join an existing one
    Setup {
        /// Accept an invite link directly
        invite: Option<String>,
    },
    /// Open the TUI dashboard
    Ui,
    /// Inject secrets into a subprocess
    Run {
        /// Override project (default: from .envi file)
        #[arg(short, long)]
        project: Option<String>,
        /// Print env vars that would be injected without running
        #[arg(long)]
        dry_run: bool,
        /// Command to run
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    /// Sync the workspace manually
    Sync,
    /// Clear cached credentials and stop the key agent
    Logout,
    /// Remove all local data (cache, config, agent)
    Clear,
    #[command(hide = true)]
    Agent {
        /// Start the agent server (internal, do not use directly)
        #[arg(long, hide = true)]
        serve: bool,
        /// Stop the running agent
        #[arg(long)]
        kill: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command.unwrap_or(Command::Ui) {
        Command::Setup { invite } => commands::setup::run(invite).await,
        Command::Ui => commands::ui::run().await,
        Command::Run { project, dry_run, cmd } => {
            commands::run::run(project, dry_run, cmd).await
        }
        Command::Sync => commands::sync::run().await,
        Command::Logout => agent::run(false, true).await,
        Command::Clear => commands::clear::run().await,
        Command::Agent { serve, kill } => agent::run(serve, kill).await,
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
