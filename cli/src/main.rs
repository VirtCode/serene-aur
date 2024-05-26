#![feature(iter_intersperse)]

pub mod log;
mod web;
mod config;
mod command;
mod table;
mod complete;
mod action;

use clap::Parser;
use crate::command::Args;
use crate::config::Config;

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



