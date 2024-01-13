use chrono::{Local};
use colored::{ColoredString, Colorize};
use cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron;
use serde::{Deserialize, Serialize};
use serene_data::package::{PackageInfo, PackagePeek};
use crate::config::Config;
use crate::web::{get, post, post_empty};
use crate::web::data::{BuildStateFormatter, get_build_hash};

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
            println!("{:<3} {:<15} {:<7} {:<17} {:>5}", "id".italic(), "version".italic(), "success".italic(), "date".italic(), "time".italic());

            info.builds.sort_by_key(|b| b.started);
            info.builds.reverse();

            for peek in info.builds {
                println!("{:<3} {:<15.15} {:<7} {:17} {:>5}",
                    get_build_hash(&peek),
                    peek.version.map(ColoredString::from).unwrap_or_else(|| "unknown".dimmed()),
                    peek.state.colored_substantive(),
                    peek.started.with_timezone(&Local).format("%x %X"),
                    peek.ended.map(|ended| {
                       format!("{}s", (ended - peek.started).num_seconds())
                    }).map(ColoredString::from).unwrap_or_else(|| "running".blue())
                );
            }
        }
        Err(e) => { e.print() }
    }
}
