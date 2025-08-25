pub mod pacman;
mod procedures;

use crate::action::procedures::{
    add, build, build_all, build_info, build_logs, info, list, pkgbuild, remove, set_setting,
    signing_key, subscribe_build_logs, webhook_secret,
};
use crate::command::{Action, HostSubcommand, InfoCommand, ServerSubcommand};
use crate::complete::generate_completions;
use crate::config::Config;
use crate::intro;
use crate::log::Log;
use clap_complete::Shell;
use colored::Colorize;
use procedures::server_info;

pub fn run(config: &Config, action: Action) {
    match action {
        Action::Host { manage } => match manage {
            HostSubcommand::Secret { machine } => {
                if machine {
                    config.print_secret(false)
                } else {
                    println!(
                        "Add this line to the {} of your target server to trust this machine:",
                        "authorized_secrets".italic()
                    );
                    config.print_secret(true);
                }
            }
            HostSubcommand::Signatures => {
                if let Err(e) = intro::import_pacman_key(config, false) {
                    eprintln!();
                    eprintln!("Failed to complete key setup: {e:#}");
                }
            }
        },

        Action::Add {
            what,
            pkgbuild,
            custom,
            noresolve,
            devel,
            replace,
            install,
            listen,
            quiet,
            file,
        } => {
            add(
                config,
                &what,
                replace,
                noresolve,
                file,
                custom,
                pkgbuild,
                devel,
                install || listen,
                quiet,
                listen,
            );
        }
        Action::Remove { name } => {
            remove(config, &name);
        }
        Action::Build { names, clean, noresolve, gentle, install, listen, quiet, all, force } => {
            if all {
                build_all(config, force, !noresolve, clean);
            } else {
                build(config, names, clean, !noresolve, install || listen, quiet, !gentle, listen);
            }
        }
        Action::List => {
            list(config);
        }
        Action::Info { name, what, all } => match what {
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
                    subscribe_build_logs(config, &name, subscribe, linger);
                }
            }
            Some(InfoCommand::Set { property }) => set_setting(config, &name, property),
        },
        Action::Server { manage } => match manage {
            ServerSubcommand::Webhook { name, machine } => {
                webhook_secret(config, &name, machine);
            }
            ServerSubcommand::Info => server_info(config),
            ServerSubcommand::Key { machine } => signing_key(config, machine),
        },
        Action::Completions => {
            let Some(shell) = Shell::from_env() else {
                Log::failure("failed to determine current shell");
                return;
            };

            println!("{}", generate_completions(shell));
        }
    }
}
