mod glitch;
mod info;
mod master;
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
    /// Slice interesting moments from inputs into a rhythmic, glitchy mashup
    Glitch(glitch::Args),
    /// Print WAV file metadata
    Info(info::Args),
    /// Apply a mastering chain: high-pass filter, compression, limiting, and normalization
    Master(master::Args),
    /// Normalize audio to peak amplitude
    Normalize(normalize::Args),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Glitch(args) => glitch::run(args),
        Commands::Info(args) => info::run(args),
        Commands::Master(args) => master::run(args),
        Commands::Normalize(args) => normalize::run(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
