use clap::{Parser, Subcommand};

use crate::commands::{
    create::{CreateArgs, handle_create},
    delete::{DeleteArgs, handle_delete},
    get::{GetArgs, handle_get},
    logs::{LogArgs, handle_logs},
};

mod commands;
mod config;

/// CLI tool to interact with the r8s cluster: deploy, inspect, and manage workloads and resources.
#[derive(Parser, Debug)]
#[command(name = "r8sctl", version, about, long_about = None)]
struct R8sCtl {
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

fn main() {
    let cli = R8sCtl::parse();
    let config = config::load_config();

    match cli.command {
        Commands::Get(args) => handle_get(&config, &args),
        Commands::Create(args) => handle_create(&config, &args),
        Commands::Delete(args) => handle_delete(&config, &args),
        Commands::Logs(args) => handle_logs(&config, &args),
    };
}
