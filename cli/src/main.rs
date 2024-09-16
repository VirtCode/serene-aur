#![feature(iter_intersperse)]

mod action;
mod command;
mod complete;
mod config;
pub mod log;
mod table;
mod web;

use crate::command::Args;
use crate::config::Config;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    // load config
    let mut config = Config::create()?;

    // parse command
    let args = Args::parse();

    if let Some(host) = args.server {
        config.url = host;
    }

    // run subcommands
    action::run(&config, args.command);

    Ok(())
}
