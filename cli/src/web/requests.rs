use std::str::FromStr;
use anyhow::Context;
use chrono::{Local, Utc};
use colored::{ColoredString, Colorize};
use cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron;
use serene_data::build::{BuildInfo, BuildState};
use serene_data::package::{MakepkgFlag, PackageAddRequest, PackageAddSource, PackageInfo, PackagePeek, PackageSettingsRequest};
use crate::command::SettingsSubcommand;
use crate::config::Config;
use crate::table::{ago, Column, table};
use crate::web::{delete_empty, get, post, post_empty, post_simple};
use crate::web::data::{BuildProgressFormatter, BuildStateFormatter, get_build_id};

pub fn add_aur(c: &Config, name: &str, replace: bool) {
    info!("Adding package {} from the AUR...", name.italic());

    match post::<PackageAddRequest, PackagePeek>(c, "package/add", PackageAddRequest {
        replace,
        source: PackageAddSource::Aur { name: name.to_owned() }
    }) {
        Ok(info) => {
            info!("Successfully added package {}", info.base.bold());
        }
        Err(e) => { e.print() }
    }
}

pub fn add_git(c: &Config, url: &str, devel: bool, replace: bool) {
    info!("Adding custom package at {}...", url.italic());

    match post::<PackageAddRequest, PackagePeek>(c, "package/add", PackageAddRequest {
        replace,
        source: PackageAddSource::Custom { url: url.to_owned(), devel }
    }) {
        Ok(info) => {
            info!("Successfully added package {}", info.base.bold());
        }
        Err(e) => { e.print() }
    }
}

pub fn add_pkgbuild(c: &Config, pkgbuild: &str, devel: bool, replace: bool) {
    info!("Adding custom pkgbuild package...");

    match post::<PackageAddRequest, PackagePeek>(c, "package/add", PackageAddRequest {
        replace,
        source: PackageAddSource::Single { pkgbuild: pkgbuild.to_owned(), devel }
    }) {
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
            list.sort_by_key(|p| p.base.clone());
            
            let columns = [
                Column::new("name").ellipse(),
                Column::new("version"),
                Column::new("devel").force().centered(),
                Column::new("enabl").force().centered(),
                Column::new("build").force().centered(),
                Column::new("time ago").force()
            ];
            
            let rows = list.iter().map(|peek| {
                [
                    peek.base.bold(), 
                    peek.version.as_ref().map(|s| s.normal()).unwrap_or_else(|| "unknown".dimmed()),
                    if peek.devel { "X".dimmed() } else { "".dimmed() },
                    if peek.enabled { "X".yellow() } else { "".dimmed() },
                    peek.build.as_ref().map(|p| p.state.colored_passive()).unwrap_or_else(|| "none".dimmed()),
                    peek.build.as_ref().map(|p| {
                        let duration = Utc::now() - p.ended.unwrap_or(p.started);
                        let string = ago::difference(duration);
                        
                        if duration.num_weeks() > 0 { string.dimmed() }
                        else { string.normal() }
                    }).unwrap_or("never".to_string().bold())
                ]
            }).collect();
            
            table(columns, rows, "  ");
        }
        Err(e) => { e.print() }
    }
}

pub fn info(c: &Config, package: &str, all: bool) {
    info!("Querying server...");

    let query = if all { "" } else { "?count=8" };

    match (
        get::<PackageInfo>(c, format!("package/{}", package).as_str()),
        get::<Vec<BuildInfo>>(c, format!("package/{}/build{}", package, query).as_str())
    ) {
        (Ok(mut info), Ok(mut builds)) => {
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

            println!("{:<9} {}", "flags:",
                if info.makepkg_flags.is_empty() { "none".italic().dimmed() }
                else { info.makepkg_flags.iter().map(|f| format!("--{f} ")).collect::<String>().normal() }
            );

            if let Some(prepare) = &info.prepare_commands {
                println!();
                println!("prepare commands:");
                println!("{}", prepare.trim());
            }

            println!();
            println!("builds:");
            
            let columns = [
                Column::new("id").force(),
                Column::new("version"),
                Column::new("state").force(),
                Column::new("date").force(),
                Column::new("time").force()
            ];

            let rows = builds.iter().map(|peek| {
                [
                    get_build_id(peek).dimmed(),
                    peek.version.as_ref().map(|s| s.normal()).unwrap_or_else(|| "unknown".dimmed()),
                    peek.state.colored_substantive(),
                    peek.started.with_timezone(&Local).format("%x %X").to_string().normal(),
                    peek.ended.map(|ended| {
                        format!("{}s", (ended - peek.started).num_seconds())
                    }).map(ColoredString::from).unwrap_or_else(|| "??".blue())
                ]
            }).collect();
            
            table(columns, rows, "  ");
        }
        (Err(e), _) => { e.print() }
        (_, Err(e)) => { e.print() }
    }
}

pub fn build_info(c: &Config, package: &str, build: &Option<String>) {
    println!("Querying server for package builds...\n");
    match get::<BuildInfo>(c, format!("package/{}/build/{}", package, build.as_ref().unwrap_or(&"latest".to_string())).as_str()) {
        Ok(b) => {

            println!("{} {}", "build".bold(), get_build_id(&b).bold());
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


pub fn build_logs(c: &Config, package: &str, build: &Option<String>) {
    println!("Querying server for package builds...");
    match get::<String>(c, format!("package/{}/build/{}/logs", package, build.as_ref().unwrap_or(&"latest".to_string())).as_str()) {
        Ok(logs) => { println!("{logs}") }
        Err(e) => { e.print() }
    }
}

pub fn set_setting(c: &Config, package: &str, setting: SettingsSubcommand) {
    let request = match setting {
        SettingsSubcommand::Clean { enabled } => {
            info!("{} clean build for package {}...", if enabled { "Enabling" } else { "Disabling" }, package);
            PackageSettingsRequest::Clean(enabled)
        }
        SettingsSubcommand::Enable { enabled } => {
            info!("{} building for package {}...", if enabled { "Enabling" } else { "Disabling" }, package);
            PackageSettingsRequest::Enabled(enabled)
        }
        SettingsSubcommand::Schedule { cron } => {
            let Ok(description) = get_description_cron(&cron) else {
                error!("invalid cron string provided");
                return;
            };

            info!("Setting custom schedule '{}' for package {}...", description, package);
            PackageSettingsRequest::Schedule(cron)
        }
        SettingsSubcommand::Prepare { command } => {
            info!("Setting prepare command for package {}...", package);
            PackageSettingsRequest::Prepare(command)
        }
        SettingsSubcommand::Flags { flags } => {
            let flags = flags.split_whitespace()
                .map(|s| MakepkgFlag::from_str(s).map_err(|e| format!("makepkg flag --{s} not supported")))
                .collect::<Result<Vec<MakepkgFlag>, String>>();

            match flags {
                Ok(f) => { PackageSettingsRequest::Flags(f) }
                Err(e) => {
                    error!("{e}");
                    return;
                }
            }
        }
    };

    match post_simple(c, &format!("package/{}/set", package), request) {
        Ok(()) => {
            info!("Successfully changed property")
        }
        Err(e) => { e.print() }
    }
}

pub fn pkgbuild(c: &Config, package: &str) {
    // we do print nothing, as this may be used to store in file

    match get::<String>(c, format!("package/{}/pkgbuild", package).as_str()) {
        Ok(l) => { println!("{l}") }
        Err(e) => { e.print() }
    }
}