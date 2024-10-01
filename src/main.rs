use clap::{Parser, Subcommand};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check if the environment is ready
    Check {
        /// The environment to check
        #[arg(short, long, default_value = "development")]
        environment: String,
    },
    /// Build the Dockerfile
    Dockerfile,
    /// Build builds
    Build,
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Check { environment } => println!("hello {}", environment),
        Commands::Dockerfile => {}
        Commands::Build => {}
    }
}
