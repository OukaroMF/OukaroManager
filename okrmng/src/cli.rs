use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version = "0.1", about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum SystemApp {
    Add {
        #[arg(long, short)]
        package: String,
    },
    Rm {
        #[arg(long, short)]
        package: String,
    },
}

#[derive(Subcommand)]
enum PrivApp {
    Add {
        #[arg(long, short)]
        package: String,
    },
    Rm {
        #[arg(long, short)]
        package: String,
    },
}

#[derive(Subcommand)]
enum Commands {
    SystemApp {
        #[command(subcommand)]
        command: SystemApp,
    },
    PrivApp {
        #[command(subcommand)]
        command: PrivApp,
    },
}

pub fn run() {
    let args = Args::parse();
}
