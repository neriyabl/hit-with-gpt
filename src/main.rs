use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hit")]
#[command(about = "A Git alternative with AI features", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Watch,
    Serve,
    Sync,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => {
            if let Err(e) = hit_with_gpt::repo::init() {
                eprintln!("Error initializing repository: {}", e);
            }
        }
        Commands::Watch => {
            if let Err(e) = hit_with_gpt::watcher::watch_and_store_changes() {
                eprintln!("Watcher error: {}", e);
            }
        }
        Commands::Serve => {
            hit_with_gpt::server::start_server().await;
        }
        Commands::Sync => {
            hit_with_gpt::sync::sync_from_server().await;
        }
    }
}
