mod procedures;
pub mod pacman;

use clap_complete::Shell;
use colored::Colorize;
use crate::action::procedures::{add, build, build_info, build_logs, info, list, pkgbuild, remove, set_setting, subscribe_build_logs};
use crate::command::{Action, InfoCommand};
use crate::complete::generate_completions;
use crate::config::Config;
use crate::log::Log;

pub fn run(config: &Config, action: Action) {
    match action {
        Action::Secret { machine } => {
            if machine {
                config.print_secret(false)
            } else {
                println!("Add this line to the {} of your target server to trust this machine:", "authorized_secrets".italic());
                config.print_secret(true);
            }
        }

        Action::Add { what, pkgbuild, custom, devel, replace, install, quiet, file } => {
            add(config, &what, replace, file, custom, pkgbuild, devel, install, quiet);
        }
        Action::Remove { name } => {
            remove(config, &name);
        }
        Action::Build { name, clean, install, quiet } => {
            build(config, &name, clean, install, quiet);
        }
        Action::List => {
            list(config);
        }
        Action::Info { name, what, all } => {
            match what {
                None => { 
                    info(config, &name, all); 
                }
                Some(InfoCommand::Pkgbuild) => { 
                    pkgbuild(config, &name); 
                }
                Some(InfoCommand::Build { id }) => { 
                    build_info(config, &name, &id); 
                }
                Some(InfoCommand::Logs { id, subscribe, linger }) => {
                    if id.is_some() {
                        build_logs(config, &name, &id);
                    } else {
                        subscribe_build_logs(config, &name, linger, subscribe);
                    }
                }
                Some(InfoCommand::Set { property }) => { 
                    set_setting(config, &name, property) 
                }
            }
        }
        Action::Completions => {
            let Some(shell) = Shell::from_env() else {
                Log::failure("failed to determine current shell");
                return;
            };

            println!("{}", generate_completions(shell));
        }
    }
}
