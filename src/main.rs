mod info;
mod normalize;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "klang", about = "WAV file utilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print WAV file metadata
    Info(info::Args),
    /// Normalize audio to peak amplitude
    Normalize(normalize::Args),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Info(args) => info::run(args),
        Commands::Normalize(args) => normalize::run(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
