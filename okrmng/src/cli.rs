use std::fs;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::{config, defs::CONFIG_PATH};

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

pub fn run() -> Result<()> {
    let args = Args::parse();
    let mut config = config::Config::new()?;

    match args.command {
        Commands::PrivApp { command } => {
            println!("setting priv-app");

            match command {
                PrivApp::Add { package } => {
                    config.app.priv_app.insert(package);
                    println!("added new package");
                }
                PrivApp::Rm { package } => {
                    config.app.priv_app.remove(&package);
                    println!("removed package");
                }
            }
        }
        Commands::SystemApp { command } => {
            println!("setting system-app");
            
            
            match command {
                SystemApp::Add { package } => {
                    config.app.system_app.insert(package);
                    println!("added new package");
                }
                SystemApp::Rm { package } => {
                    config.app.system_app.remove(&package);
                    println!("removed package");
                }
            }
        }
    }

    fs::write(
        CONFIG_PATH,
        toml::to_string(&config).context("Failed to change config")?,
    )?;

    Ok(())
}
