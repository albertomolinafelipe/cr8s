use clap::{Parser, Subcommand};

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
    Get(commands::get::GetArgs),
    /// Create or update resources from a configuration file
    Apply(commands::apply::ApplyArgs),
    /// Delete specified resources
    Delete,
    /// Show detailed information about a resource
    Describe,
}



fn main() {
    let cli = R8sCtl::parse();
    let config = config::load_config();

    match cli.command {
        Commands::Get(args) => commands::get::handle(&config, &args),
        Commands::Apply(args) => commands::apply::handle(&config, &args),
        _ => {
            println!("not implemented...");
        }
    }
}
