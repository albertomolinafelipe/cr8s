use clap::{Parser, Subcommand};

use crate::{
    commands::{
        create::{CreateArgs, handle_create},
        delete::{DeleteArgs, handle_delete},
        get::{GetArgs, handle_get},
        logs::{LogArgs, handle_logs},
    },
    config::Config,
};

mod commands;
mod config;

/// CLI tool to interact with the cr8s cluster: deploy, inspect, and manage workloads and resources.
#[derive(Parser, Debug)]
#[command(name = "cr8sctl", version, about, long_about = None)]
struct Cr8sCtl {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Retrieve cluster resources
    Get(GetArgs),
    /// Create or update resources from a configuration file
    Create(CreateArgs),
    /// Delete deployed resources
    Delete(DeleteArgs),
    /// Display the logs for a resource
    Logs(LogArgs),
}

#[tokio::main]
async fn main() {
    let cli = Cr8sCtl::parse();
    let config = Config::from_env();
    match cli.command {
        Commands::Get(args) => handle_get(&config, &args).await,
        Commands::Create(args) => handle_create(&config, &args).await,
        Commands::Delete(args) => handle_delete(&config, &args).await,
        Commands::Logs(args) => handle_logs(&config, &args).await,
    };
}
