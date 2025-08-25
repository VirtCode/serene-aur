#![allow(dead_code)]
#![feature(iter_intersperse)]

mod action;
mod command;
mod complete;
mod config;
mod intro;
pub mod log;
mod table;
mod web;

use crate::command::Args;
use crate::config::Config;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    // do intro on first run
    if !Config::exists() {
        intro::intro()?;
        return Ok(());
    }

    let args = Args::parse();
    let mut config = Config::read()?;

    if let Some(host) = args.server {
        config.url = host;
    }

    // run subcommands
    action::run(&config, args.command);

    Ok(())
}
