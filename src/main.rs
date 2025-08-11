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

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .pretty()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Init => {
            if let Err(e) = hit_with_gpt::repo::init() {
                tracing::error!(%e, "Error initializing repository");
            }
        }
        Commands::Watch => {
            if let Err(e) = hit_with_gpt::watcher::watch_and_store_changes() {
                tracing::error!(%e, "Watcher error");
            }
        }
        Commands::Serve => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build runtime");
            if let Err(e) = rt.block_on(hit_with_gpt::server::start_server()) {
                tracing::error!(%e, "Server error");
            }
        }
        Commands::Sync => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build runtime");
            rt.block_on(hit_with_gpt::sync::sync_from_server());
        }
    }
}
