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
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => {
            let cwd = std::env::current_dir().expect("failed to get current directory");
            match hit_with_gpt::init_repo(&cwd) {
                Ok(()) => println!("Initialized empty hit repository in {}/.hit", cwd.display()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    eprintln!("Repository already initialized");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error initializing repository: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
