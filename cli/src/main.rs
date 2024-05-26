#![feature(iter_intersperse)]

#[macro_use]
pub mod log;

mod web;
mod config;
mod command;
mod table;
mod complete;
pub mod pacman;

use clap::{Parser};
use clap_complete::{Shell};
use colored::Colorize;
use crate::command::{Args, Action, InfoCommand};
use crate::complete::generate_completions;
use crate::config::Config;
use crate::web::requests;
use crate::web::requests::add;

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

        Action::Add { what, pkgbuild, custom, devel, replace, install, quiet } => {
            add(&config, &what, replace, custom, pkgbuild, devel, install, quiet);
        }
        Action::Remove { name } => {
            requests::delete(&config, &name);
        }
        Action::Build { name, clean, install, quiet } => {
            requests::build(&config, &name, clean, install, quiet);
        }
        Action::List => {
            requests::list(&config);
        }
        Action::Info { name, what, all } => {
            match what {
                None => { requests::info(&config, &name, all); }
                Some(InfoCommand::Pkgbuild) => { requests::pkgbuild(&config, &name); }
                Some(InfoCommand::Build { id }) => { requests::build_info(&config, &name, &id); }
                Some(InfoCommand::Logs { id, subscribe, linger }) => { 
                    if id.is_some() {
                        requests::build_logs(&config, &name, &id); 
                    } else {
                        requests::subscribe_build_logs(&config, linger, subscribe, &name)
                    }
                }
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



