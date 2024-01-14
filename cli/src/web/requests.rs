use std::fmt::format;
use chrono::{Local};
use colored::{ColoredString, Colorize};
use cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron;
use serde::{Deserialize, Serialize};
use serene_data::build::{BuildInfo, BuildState};
use serene_data::package::{PackageInfo, PackagePeek};
use crate::config::Config;
use crate::web::{delete_empty, get, post, post_empty};
use crate::web::data::{BuildProgressFormatter, BuildStateFormatter, get_build_id};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PackageAddRequest {
    Aur { name: String },
    Custom { url: String, devel: bool }
}

pub fn add_aur(c: &Config, name: &str) {
    info!("Adding package {} from the AUR...", name.italic());

    match post::<PackageAddRequest, PackagePeek>(c, "package/add", PackageAddRequest::Aur { name: name.to_owned() }) {
        Ok(info) => {
            info!("Successfully added package {}", info.base.bold());
        }
        Err(e) => { e.print() }
    }
}

pub fn add_git(c: &Config, url: &str, devel: bool) {
    info!("Adding custom package at {}...", url.italic());

    match post::<PackageAddRequest, PackagePeek>(c, "package/add", PackageAddRequest::Custom{ url: url.to_owned(), devel }) {
        Ok(info) => {
            info!("Successfully added package {}", info.base.bold());
        }
        Err(e) => { e.print() }
    }
}

pub fn delete(c: &Config, package: &str) {
    info!("Requesting deletion of package {}...", package.italic());

    match delete_empty(c, format!("package/{}", package).as_str()) {
        Ok(()) => { info!("Successfully deleted package") }
        Err(e) => { e.print() }
    }
}

pub fn build(c: &Config, package: &str) {
    info!("Requesting build for package {}...", package.italic());

    match post_empty(c, format!("package/{}/build", package).as_str()) {
        Ok(()) => { info!("Successfully dispatched build") }
        Err(e) => { e.print() }
    }
}

pub fn list(c: &Config) {
    info!("Querying server...");

    match get::<Vec<PackagePeek>>(c, "package/list") {
        Ok(mut list) => {
            println!();
            println!("{:<20} {:<15} {:<5} {:<5} {:<7}", "name".italic(), "version".italic(), "devel".italic(), "enabl".italic(), "build".italic());

            list.sort_by_key(|p| p.base.clone());

            for peek in list {
                println!("{:<20.20} {:<15.15} {:^5} {:^5} {:<7}",
                    peek.base.bold(),
                    peek.version,
                    if peek.devel { "X".dimmed() } else { "".dimmed() },
                    if peek.enabled { "X".yellow() } else { "".dimmed() },
                    peek.build.map(|p| p.state.colored_passive()).unwrap_or_else(|| "none".dimmed())
                );
            }
        }
        Err(e) => { e.print() }
    }
}

pub fn info(c: &Config, package: &str) {
    info!("Querying server...");

    match get::<PackageInfo>(c, format!("package/{}", package).as_str()) {
        Ok(mut info) => {
            println!();
            println!("{}", info.base.bold());
            println!("{:<9} {}", "members:", info.members.join(" "));
            println!("{:<9} {}", "added:", info.added.with_timezone(&Local).format("%x %X"));

            let mut tags = vec![];
            if info.enabled { tags.push("enabled".yellow()) } else { tags.push("disabled".dimmed()) }
            if info.clean { tags.push("clean".blue()) }
            if info.devel { tags.push("devel".dimmed()) }

            println!("{:<9} {}", "status:",
                     tags.iter().map(|s| s.to_string()).intersperse(" ".to_string()).collect::<String>()
            );

            println!("{:<9} {} (UTC)", "schedule:",
                get_description_cron(&info.schedule).unwrap_or_else(|_| "could not parse cron".to_owned())
            );

            println!();
            println!("builds:");
            println!("{:<4}  {:<15}  {:<7}  {:<17}  {:>5}", "id".italic(), "version".italic(), "state".italic(), "date".italic(), "time".italic());

            info.builds.sort_by_key(|b| b.started);
            info.builds.reverse();

            for peek in info.builds {
                println!("{:<4}  {:<15.15}  {:<7}  {:17}  {:>5}",
                    get_build_id(&peek).dimmed(),
                    peek.version.map(ColoredString::from).unwrap_or_else(|| "unknown".dimmed()),
                    peek.state.colored_substantive(),
                    peek.started.with_timezone(&Local).format("%x %X"),
                    peek.ended.map(|ended| {
                       format!("{}s", (ended - peek.started).num_seconds())
                    }).map(ColoredString::from).unwrap_or_else(|| "??".blue())
                );
            }
        }
        Err(e) => { e.print() }
    }
}

pub fn build_info(c: &Config, package: &str, build: &str) {
    println!("Querying server for package builds...\n");
    match get::<PackageInfo>(c, format!("package/{}", package).as_str()) {
        Ok(info) => {

            let Some(b) = info.builds.iter().find(|b| get_build_id(b).as_str() == build.to_lowercase()) else {
                error!("not build for {} with the id {} found", info.base, build);
                return;
            };

            println!("{} {}", "build".bold(), build.to_lowercase().bold());
            println!("{:<8} {}", "started:",
                     b.started.with_timezone(&Local).format("%x %X"));
            println!("{:<8} {}", "ended:",
                     b.ended.map(|s| s.with_timezone(&Local).format("%x %X").to_string())
                         .unwrap_or_else(|| "not yet".to_string()));
            println!("{:<8} {}", "version:",
                     b.version.as_ref()
                         .map(|b| ColoredString::from(b.as_str()))
                         .unwrap_or_else(|| "unknown".dimmed()));

            let additive = match &b.state {
                BuildState::Running(state) | BuildState::Fatal(_, state) => {
                    format!("on {}", state.printable_string())
                }
                _ => "".to_string()
            };

            println!("\n{:<8} {} {}", "status:", b.state.colored_substantive(), additive);

            match &b.state {
                BuildState::Failure => { println!("{:<8} {}", "message:", "see logs for error messages".italic()) }
                BuildState::Fatal(msg, _) => { println!("{:<8} {}", "message:", msg) }
                _ => {}
            }
        }
        Err(e) => { e.print() }
    }
}


pub fn build_logs(c: &Config, package: &str, build: &str) {
    println!("Querying server for package builds...");
    match get::<PackageInfo>(c, format!("package/{}", package).as_str()) {
        Ok(info) => {

            let Some(b) = info.builds.iter().find(|b| get_build_id(b).as_str() == build.to_lowercase()) else {
                error!("not build for {} with the id {} found", info.base, build);
                return;
            };

            println!("Retrieving build logs...\n");
            match get::<String>(c, format!("package/{}/build/{}/logs", package, b.started).as_str()) {
                Ok(l) => { println!("{l}") }
                Err(e) => { e.print() }
            }

        }
        Err(e) => { e.print() }
    }
}
