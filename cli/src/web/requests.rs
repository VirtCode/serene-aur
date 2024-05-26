use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use chrono::{Local, Utc};
use colored::{ColoredString, Colorize};
use reqwest_eventsource::Event;
use serene_data::build::{BuildInfo, BuildState};
use serene_data::package::{BroadcastEvent, MakepkgFlag, PackageAddRequest, PackageAddSource, PackageBuildRequest, PackageInfo, PackagePeek, PackageSettingsRequest};
use crate::command::SettingsSubcommand;
use crate::config::Config;
use crate::log::Loading;
use crate::pacman;
use crate::table::{ago, Column, table};
use crate::web::{delete_empty, eventsource, get, post, post_empty, post_simple, Result};
use crate::web::data::{BuildProgressFormatter, BuildStateFormatter, describe_cron_timezone_hack, get_build_id};

pub fn add_package(c: &Config, request: PackageAddRequest) -> Result<PackagePeek> {
    post::<PackageAddRequest, PackagePeek>(c, "package/add", request)
}

pub fn build_package(c: &Config, package: &str, request: PackageBuildRequest) -> Result<()> {
    post_simple(c, &format!("package/{package}/build"), request)
}

pub fn get_build(c: &Config, package: &str, id: &str) -> Result<BuildInfo> {
    get::<BuildInfo>(c, &format!("package/{package}/build/{id}"))
}

pub fn get_package(c: &Config, package: &str) -> Result<PackageInfo> {
    get::<PackageInfo>(c, &format!("package/{package}"))
}

pub fn subscribe_events<F>(c: &Config, package: &str, mut callback: F) -> Result<()> where F: FnMut(BroadcastEvent, String) -> bool {
    eventsource(c, &format!("package/{package}/build/logs/subscribe"), |event| {
        if let Event::Message(event) = event {

            // ignore unknown events
            if let Ok(brd) = BroadcastEvent::from_str(&event.event) {
                return callback(brd, event.data);
            }
        }
        
        false
    })
}

pub fn add(c: &Config, what: &str, replace: bool, custom: bool, pkgbuild: bool, devel: bool, install: bool, quiet: bool) {
    let mut log = Loading::start("initializing package adding");
    
    let source = if pkgbuild {
        log.next(&format!("adding package {} from the AUR", what.italic()));
        PackageAddSource::Single { pkgbuild: what.to_owned(), devel }
    } else if custom {
        log.next(&format!("adding package from repository at {}", what.italic()));
        PackageAddSource::Custom { url: what.to_owned(), devel }
    } else {
        log.next("adding package from custom pkgbuild");
        PackageAddSource::Aur { name: what.to_owned() }
    };
    
    let info = match add_package(c, PackageAddRequest { replace, source }) {
        Ok(info) => { info }
        Err(e) => { log.fail(&e.msg()); return }
    };
    
    log.succeed(&format!("successfully added package {}", info.base.bold()));
    
    // install if requested
    if install { wait_and_install(c, &info.base, quiet); }
}

/// waits for a package to build and then installs it
fn wait_and_install(c: &Config, base: &str, quiet: bool) {
    let log = RefCell::new(Some(Loading::start("subscribing to logs")));
    let mut started = false;

    // waiting for build to finish
    let mut log = match subscribe_events(c, base, |e, data| {

        match e {
            BroadcastEvent::BuildStart | BroadcastEvent::Log => {
                if !started {
                    if !quiet { if let Some(log) = log.replace(None) { log.succeed("subscribed to logs successfully") }}
                    else if let Some(log) = log.borrow_mut().as_mut() { log.next("waiting for build to finish") }
                }

                if !quiet {
                    print!("{data}");
                }

                started = true;
            }
            BroadcastEvent::BuildEnd => { return true; }
            BroadcastEvent::Ping => {}
        }

        false
    }) {
        Ok(()) => {
            if let Some(log) = log.replace(None) { log }
            else { Loading::start("finishing up build") }
        }
        Err(err) => {
            if let Some(log) = log.replace(None) { log.fail(&format!("log subscription failed: {}", &err.msg())) }
            else { Loading::failure(&format!("log subscription failed: {}", &err.msg())) }

            return;
        }
    };

    // fetch information
    log.next("fetching package information");
    let package = match get_package(c, base) {
        Ok(info) => { info }
        Err(e) => {
            log.fail(&format!("failed to fetch package: {}", &e.msg()));
            return;
        }
    };

    log.next("fetching build information");
    let build = match get_build(c, base, "latest") {
        Ok(build) => { build }
        Err(e) => {
            log.fail(&format!("failed to fetch build: {}", &e.msg()));
            return;
        }
    };

    // build must be successful
    match build.state {
        BuildState::Running(progress) => {
            log.fail(&format!("build somehow not finished, but at {progress}"));
            return;
        }
        BuildState::Failure => {
            log.fail("build failed, see logs");
            return;
        }
        BuildState::Fatal(message, progress) => {
            log.fail(&format!("fatal failure occurred at {progress}: {message}"));
            return;
        }

        // successful
        BuildState::Success => {
            log.succeed("build finished successfully");
        }
    }

    // install via pacman
    if pacman::install(c, package.members) {
        Loading::success("successfully installed packages");
    } else {
        Loading::failure("failed to install packages");
    }
}

pub fn delete(c: &Config, package: &str) {
    let log = Loading::start(&format!("removing package {} from the repository", package.italic()));

    match delete_empty(c, format!("package/{package}").as_str()) {
        Ok(()) => { log.succeed("successfully deleted package") }
        Err(e) => { log.fail(&e.msg()) }
    }
}

pub fn build(c: &Config, package: &str, clean: bool, install: bool, quiet: bool) {
    let log = Loading::start(&format!("requesting immediate build for package {}", package.italic()));

    if let Err(e) = build_package(c, package, PackageBuildRequest { clean }) {
        log.fail(&e.msg());
        return;
    }

    log.succeed("queued build successfully");

    // install if requested
    if install { wait_and_install(c, package, quiet); }
}

pub fn list(c: &Config) {
    let log = Loading::start("querying all packages");

    match get::<Vec<PackagePeek>>(c, "package/list") {
        Ok(mut list) => {
            log.succeed("retrieved package info successfully");
            
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
        Err(e) => { log.fail(&e.msg()) }
    }
}

pub fn info(c: &Config, package: &str, all: bool) {
    let log = Loading::start("loading package information and builds");

    let query = if all { "" } else { "?count=8" };

    match (
        get::<PackageInfo>(c, format!("package/{}", package).as_str()),
        get::<Vec<BuildInfo>>(c, format!("package/{}/build{}", package, query).as_str())
    ) {
        (Ok(mut info), Ok(mut builds)) => {
            log.succeed("successfully loaded all information");
            
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

            println!("{:<9} {}", "schedule:",
                describe_cron_timezone_hack(&info.schedule).unwrap_or_else(|_| "could not parse cron".to_owned())
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
        (Err(e), _) => { log.fail(&e.msg()) }
        (_, Err(e)) => { log.fail(&e.msg()) }
    }
}

pub fn build_info(c: &Config, package: &str, build: &Option<String>) {
    let log = Loading::start("querying server for the build");
    
    match get::<BuildInfo>(c, format!("package/{}/build/{}", package, build.as_ref().unwrap_or(&"latest".to_string())).as_str()) {
        Ok(b) => {
            log.succeed("found build successfully");

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
        Err(e) => { log.fail(&e.msg()) }
    }
}


pub fn build_logs(c: &Config, package: &str, build: &Option<String>) {
    let log = Loading::start("fetching last complete build logs");
    
    match get::<String>(c, format!("package/{}/build/{}/logs", package, build.as_ref().unwrap_or(&"latest".to_string())).as_str()) {
        Ok(logs) => { 
            log.succeed("fetched build logs successfully");
            println!("{logs}") 
        }
        Err(e) => { log.fail(&e.msg()) }
    }
}

fn latest_build_logs_quiet(c: &Config, package: &str) -> Option<String> {
    get::<String>(c, format!("package/{}/build/latest/logs", package).as_str()).ok()
}

pub fn subscribe_build_logs(c: &Config, linger: bool, subscribe: bool, package: &str) {
    // we have to use a rc ref cell here because of the closure later down
    let log = Rc::new(RefCell::new(Some(Loading::start("looking for existing builds"))));
    
    let mut first_build_finished = false;
    
    // skip if explicit subscription
    if !subscribe {
        if let Some(latest) = latest_build_logs_quiet(c, package) {

            if let Some(s) = log.replace(None) { s.succeed("found existing build successfully") }
            
            if linger {
                println!("{}\n\n{latest}\n{}", "### package build started".italic().dimmed(), "### package build finished".italic().dimmed());
                first_build_finished = true;
                
            } else {
                print!("{latest}"); // already has newline at end
                
                return;
            }
        }
    }
    
    if let Some(s) = log.borrow_mut().as_mut() { s.next("subscribing to live logs and waiting for ping") }

    let copy = log.clone();
    if let Err(err) = eventsource(c, format!("package/{}/build/logs/subscribe", package).as_str(), |event| {
        
        if let Event::Message(event) = event {
            if let Some(s) = copy.replace(None) { s.succeed("subscription was successful") }
            
            // ignore unknown events
            if let Ok(broadcast_event) = BroadcastEvent::from_str(&event.event) {
                match broadcast_event {
                    BroadcastEvent::Ping => {}
                    BroadcastEvent::BuildStart => {
                        if linger && first_build_finished {
                            println!("{}\n", "### package build started".italic().dimmed())
                        }
                    },
                    BroadcastEvent::BuildEnd => {
                        first_build_finished = true;
                        
                        if linger {
                            println!("\n{}", "### package build finished".italic().dimmed())
                        } else {
                            return true // exit
                        }
                    },
                    BroadcastEvent::Log => print!("{}", event.data),
                }
            }
        }
        
        false // stay attached
    }) {
        if let Some(s) = log.replace(None) { 
            s.fail(&err.msg()) 
        } else {
            Loading::failure(&err.msg());
        }
    }
}

pub fn set_setting(c: &Config, package: &str, setting: SettingsSubcommand) {
    let mut log = Loading::start("changing package settings");
    
    let request = match setting {
        SettingsSubcommand::Clean { enabled } => {
            log.next(&format!("{} clean build for package {package}", if enabled { "enabling" } else { "disabling" }));
            PackageSettingsRequest::Clean(enabled)
        }
        SettingsSubcommand::Enable { enabled } => {
            log.next(&format!("{} automatic building for package {package}", if enabled { "enabling" } else { "disabling" }));
            PackageSettingsRequest::Enabled(enabled)
        }
        SettingsSubcommand::Schedule { cron } => {
            let Ok(description) = describe_cron_timezone_hack(&cron) else {
                log.fail("invalid cron string provided");
                return;
            };

            log.next(&format!("setting custom schedule '{}' for package {package}", description));
            PackageSettingsRequest::Schedule(cron)
        }
        SettingsSubcommand::Prepare { command } => {
            log.next(&format!("setting prepare command for package {package}"));
            PackageSettingsRequest::Prepare(command)
        }
        SettingsSubcommand::Flags { flags } => {
            let flags = flags.iter()
                .map(|s| MakepkgFlag::from_str(s).map_err(|e| format!("makepkg flag --{s} not supported")))
                .collect::<std::result::Result<Vec<MakepkgFlag>, String>>();

            match flags {
                Ok(f) => {
                    log.next(&format!("changing makepkg flags package {package}"));
                    PackageSettingsRequest::Flags(f) 
                }
                Err(e) => {
                    log.fail(&e);
                    return;
                }
            }
        }
    };

    match post_simple(c, &format!("package/{package}/set"), request) {
        Ok(()) => { log.succeed(&format!("updated property for package {package} successfully")) }
        Err(e) => { log.fail(&e.msg()) }
    }
}

pub fn pkgbuild(c: &Config, package: &str) {
    let log = Loading::start("fetching last used pkgbuild");

    match get::<String>(c, format!("package/{package}/pkgbuild").as_str()) {
        Ok(pkgbuild) => { 
            log.succeed("successfully fetched last used pkgbuild");
            println!("{pkgbuild}"); 
        }
        Err(e) => { log.fail(&e.msg()) }
    }
}