#![feature(iter_intersperse)]

#[macro_use]
pub mod log;

mod web;
mod config;

use clap::{Parser, Subcommand};
use colored::Colorize;
use crate::config::Config;
use crate::web::requests;

fn main() -> anyhow::Result<()>{
    let config = Config::create()?;
    let args = Args::parse();

    match args.command {
        Command::Secret { machine } => {
            if machine { config.print_secret(false) }
            else {
                println!("Add this line to the {} of your target server to trust this machine:", "authorized_secrets".italic());
                config.print_secret(true);
            }
        }

        Command::Add { name } => {
            requests::add_aur(&config, &name);
        }
        Command::Custom {devel, url} => {
            requests::add_git(&config, &url, devel);
        }
        Command::Build { name } => {
            requests::build(&config, &name);
        }
        Command::List => {
            requests::list(&config);
        }
        Command::Info { name } => {
            requests::info(&config, &name);
        }

        _ => unimplemented!()
    }

    Ok(())
}


#[derive(Parser)]
#[clap(version, about)]
#[command(disable_help_subcommand = true)]
pub struct Args {
    #[clap(subcommand)]
    command: Command
}

#[derive(Subcommand)]
pub enum Command {
    /// adds a package from the official aur
    Add {
        /// name of that package
        name: String
    },

    /// adds a package given a git repository
    Custom {
        /// url of that repository
        url: String,
        /// set the package to be a development package
        #[clap(short, long)]
        devel: bool
    },

    /*
    /// removes a package
    Remove {
        /// name of the package
        name: String
    },
     */

    /// get info about a package
    Info {
        /// name of the package
        name: String
    },

    /// schedules an immediate build for a package
    Build {
        /// name of the package
        name: String
    },

    /// List all packages which are added
    List,

    /// prints the current secret
    Secret {
        /// print the secret in a machine readable way
        #[clap(short, long)]
        machine: bool
    }
}