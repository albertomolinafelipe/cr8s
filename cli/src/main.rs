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
    Create(commands::create::CreateArgs),
}

fn main() {
    let cli = R8sCtl::parse();
    let config = config::load_config();

    match cli.command {
        Commands::Get(args) => commands::get::handle(&config, &args),
        Commands::Create(args) => commands::create::handle(&config, &args),
    }
}
