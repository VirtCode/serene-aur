#![feature(iter_intersperse)]

#[macro_use]
pub mod log;

mod web;
mod config;
mod command;
mod table;
mod complete;

use std::io;
use std::process::exit;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use colored::Colorize;
use crate::command::{Args, Action, InfoCommand};
use crate::complete::generate_completions;
use crate::config::Config;
use crate::web::requests;

fn main() -> anyhow::Result<()> {
    let mut config = Config::create()?;
    let args = Args::parse();

    if let Some(host) = args.server {
        config.url = host;
    }

    match args.command {
        Action::Secret { machine } => {
            if machine { config.print_secret(false) } else {
                println!("Add this line to the {} of your target server to trust this machine:", "authorized_secrets".italic());
                config.print_secret(true);
            }
        }

        Action::Add { what, pkgbuild, custom, devel, replace } => {
            if pkgbuild && custom {
                error!("can either be a pkgbuild or a custom repository, not both");
                return Ok(());
            }

            if custom {
                requests::add_git(&config, &what, devel, replace);
            } else if pkgbuild {
                requests::add_pkgbuild(&config, &what, devel, replace);
            } else {
                if devel { info!("{} devel flag is ignored for aur packages", "warn:".bright_yellow().bold())}
                requests::add_aur(&config, &what, replace);
            }
        }
        Action::Remove { name } => {
            requests::delete(&config, &name);
        }
        Action::Build { name, clean } => {
            requests::build(&config, &name, clean);
        }
        Action::List => {
            requests::list(&config);
        }
        Action::Info { name, what, all } => {
            match what {
                None => { requests::info(&config, &name, all); }
                Some(InfoCommand::Pkgbuild) => { requests::pkgbuild(&config, &name); }
                Some(InfoCommand::Build { id }) => { requests::build_info(&config, &name, &id); }
                Some(InfoCommand::Logs { id }) => { requests::build_logs(&config, &name, &id); }
                Some(InfoCommand::Set { property }) => { requests::set_setting(&config, &name, property) }
            }
        }
        Action::Completions => {
            let Some(shell) = Shell::from_env() else {
                error!("failed to determine current shell"); 
                return Ok(());
            };
            
            println!("{}", generate_completions(shell));
        }
    }

    Ok(())
}



