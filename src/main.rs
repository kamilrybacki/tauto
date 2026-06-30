use clap::Parser;

#[derive(Parser)]
#[command(name = "tauto", about = "Lean-backed business contract verifier")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Verify contracts in a directory
    Verify {
        #[arg(help = "Path to contracts")]
        path: std::path::PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Verify { path } => {
            println!("Verifying contracts in {}", path.display());
        }
    }
}
