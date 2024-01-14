#![feature(iter_intersperse)]

#[macro_use]
pub mod log;

mod web;
mod config;
mod command;

use clap::{Parser, Subcommand};
use colored::Colorize;
use crate::command::{Args, Command, InfoCommand};
use crate::config::Config;
use crate::web::requests;

fn main() -> anyhow::Result<()> {
    let mut config = Config::create()?;
    let args = Args::parse();

    if let Some(host) = args.server {
        config.url = host;
    }

    match args.command {
        Command::Secret { machine } => {
            if machine { config.print_secret(false) } else {
                println!("Add this line to the {} of your target server to trust this machine:", "authorized_secrets".italic());
                config.print_secret(true);
            }
        }

        Command::Add { name, custom, devel } => {
            if custom {
                requests::add_git(&config, &name, devel);
            } else {
                requests::add_aur(&config, &name);
            }
        }
        Command::Remove { name } => {
            requests::delete(&config, &name);
        }
        Command::Build { name } => {
            requests::build(&config, &name);
        }
        Command::List => {
            requests::list(&config);
        }
        Command::Info { name, what } => {
            match what {
                None => { requests::info(&config, &name); }
                Some(InfoCommand::Build { id }) => { requests::build_info(&config, &name, &id); }
                Some(InfoCommand::Logs { id }) => { requests::build_logs(&config, &name, &id); }
            }
        }
    }

    Ok(())
}



