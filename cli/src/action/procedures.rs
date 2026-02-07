use crate::action::pacman;
use crate::action::util::{bytes_str, duration_str};
use crate::command::SettingsSubcommand;
use crate::complete::save_completions;
use crate::config::Config;
use crate::log::Log;
use crate::table::{Column, ago, table};
use crate::web::data::{
    BuildProgressFormatter, BuildReasonFormatter, BuildStateFormatter, describe_cron_timezone_hack,
};
use crate::web::requests::{
    add_package, build_all_packages, build_package, get_build, get_build_logs, get_builds,
    get_info, get_key, get_package, get_package_pkgbuild, get_packages, get_webhook_secret,
    remove_package, set_package_setting, subscribe_events,
};
use chrono::{Local, Utc};
use colored::{ColoredString, Colorize};
use semver::Version;
use serene_data::build::BuildState;
use serene_data::package::{
    BroadcastEvent, MakepkgFlag, PackageAddRequest, PackageAddSource, PackageBuildRequest,
    PackageSettingsRequest,
};
use std::cell::RefCell;
use std::env::consts::ARCH;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::time::Duration;

/// waits for a package to build and then installs it
fn wait_and_install(c: &Config, base: &str, quiet: bool, just_listen: bool) {
    let log = RefCell::new(Some(Log::start("subscribing to package build events")));
    let mut started = false;

    // waiting for build to finish
    let mut log = match subscribe_events(c, base, |_package, event| {
        match event {
            BroadcastEvent::Log(msg) => {
                if !started
                    && !quiet
                    && let Some(log) = log.replace(None)
                {
                    log.succeed("package build started successfully")
                }

                if !quiet {
                    print!("{msg}");
                }

                started = true;
            }
            BroadcastEvent::Change(event) => match event {
                BuildState::Pending => {
                    if let Some(log) = log.borrow_mut().as_mut() {
                        log.next("waiting for resolving and dependencies to build")
                    }
                }
                BuildState::Running(_) => {
                    if let Some(log) = log.borrow_mut().as_mut() {
                        log.next("waiting for package to build")
                    }
                }
                BuildState::Cancelled(_)
                | BuildState::Success
                | BuildState::Failure
                | BuildState::Fatal(_, _) => {
                    return true;
                }
            },
            BroadcastEvent::Ping => {}
        }

        false
    }) {
        Ok(()) => {
            if let Some(log) = log.replace(None) {
                log
            } else {
                Log::start("finishing up build")
            }
        }
        Err(err) => {
            if let Some(log) = log.replace(None) {
                log.fail(&format!("log subscription failed: {}", &err.msg()))
            } else {
                Log::failure(&format!("log subscription failed: {}", &err.msg()))
            }

            return;
        }
    };

    // fetch information
    log.next("fetching package information");
    let package = match get_package(c, base) {
        Ok(info) => info,
        Err(e) => {
            log.fail(&format!("failed to fetch package: {}", &e.msg()));
            return;
        }
    };

    log.next("fetching build information");
    let build = match get_build(c, base, "latest") {
        Ok(build) => build,
        Err(e) => {
            log.fail(&format!("failed to fetch build: {}", &e.msg()));
            return;
        }
    };

    // build must be successful
    match build.state {
        BuildState::Running(progress) => {
            log.fail(&format!("build somehow not finished, but at {progress}? bug this"));
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
        BuildState::Pending => {
            log.fail("build didn't actually start, is still pending? bug this");
            return;
        }
        BuildState::Cancelled(message) => {
            log.fail(&format!("build was cancelled due to: {message}"));
            return;
        }

        // successful
        BuildState::Success => {
            log.succeed("build finished successfully");
        }
    }

    // install via pacman
    if !just_listen {
        if pacman::install(c, package.members) {
            Log::success("successfully installed packages");
        } else {
            Log::failure("failed to install packages");
        }
    }
}

/// add a package to the repository
pub fn add(
    c: &Config,
    what: &str,
    replace: bool,
    noresolve: bool,
    skipbuild: bool,
    file: bool,
    custom: bool,
    pkgbuild: bool,
    devel: bool,
    install: bool,
    quiet: bool,
    listen: bool,
) {
    let mut log = Log::start("initializing package adding");

    // read file if requested
    let what = if file {
        log.next("loading content from file");

        let mut file = match File::open(what) {
            Ok(f) => f,
            Err(e) => {
                log.fail(&format!("failed to open file: {e:#}"));
                return;
            }
        };

        let mut buf = String::new();
        if let Err(e) = file.read_to_string(&mut buf) {
            log.fail(&format!("failed to read file: {e:#}"));
            return;
        }

        buf
    } else {
        what.to_owned()
    };

    // parse source
    let source = if pkgbuild {
        log.next("adding package from custom pkgbuild");
        PackageAddSource::Raw { pkgbuild: what.to_owned(), devel }
    } else if custom {
        log.next(&format!("adding package from repository at {}", what.italic()));
        PackageAddSource::Git { url: what.to_owned(), devel }
    } else {
        log.next(&format!("adding package {} from the AUR", what.italic()));
        PackageAddSource::Aur { name: what.to_owned() }
    };

    // add package on server
    let info = match add_package(
        c,
        PackageAddRequest { replace, source, build: !skipbuild, resolve: !noresolve },
    ) {
        Ok(info) => info,
        Err(e) => {
            log.fail(&e.msg());
            return;
        }
    };

    log.succeed(&format!(
        "successfully added packages {}",
        info.iter().map(|i| i.base.as_str()).collect::<Vec<_>>().join(", ")
    ));

    // install if requested
    if install {
        wait_and_install(c, &info.first().expect("added no package?").base, quiet, listen);
    }
}

/// removes a package from the repository
pub fn remove(c: &Config, package: &str) {
    let log = Log::start(&format!("removing package {} from the repository", package.italic()));

    match remove_package(c, package) {
        Ok(()) => log.succeed("successfully deleted package"),
        Err(e) => log.fail(&e.msg()),
    }
}

pub fn build_all(c: &Config, force: bool, resolve: bool, clean: bool) {
    let log = Log::start("requesting build for all packages");

    if let Err(e) = build_all_packages(c, PackageBuildRequest::all(clean, resolve, force)) {
        log.fail(&e.msg());
    } else {
        log.succeed("queued build for every package successfully")
    }
}

/// builds packages right now
pub fn build(
    c: &Config,
    packages: Vec<String>,
    clean: bool,
    resolve: bool,
    install: bool,
    quiet: bool,
    force: bool,
    listen: bool,
) {
    let log = Log::start(&format!(
        "requesting immediate build for package{} {}",
        if packages.len() > 1 { "s" } else { "" },
        packages.join(", ").italic()
    ));

    if let Err(e) =
        build_package(c, PackageBuildRequest::specific(packages.clone(), clean, resolve, force))
    {
        log.fail(&e.msg());
        return;
    }

    log.succeed("queued build successfully");

    // install if requested
    if install {
        if packages.len() > 1 {
            Log::warning("waiting for multiple packages to build is not yet supported");
        } else {
            wait_and_install(c, packages.first().expect("no first argument?"), quiet, listen);
        }
    }
}

/// list all packages in a table
pub fn list(c: &Config) {
    check_version_mismatch(c);

    let log = Log::start("querying all packages");

    match get_packages(c) {
        Ok(mut list) => {
            log.succeed("retrieved package info successfully");

            save_completions(&list);

            println!();
            list.sort_by_key(|p| p.base.clone());

            let columns = [
                Column::new("name").ellipse(),
                Column::new("version"),
                Column::new("devel").force().centered(),
                Column::new("enabl").force().centered(),
                Column::new("build").force().centered(),
                Column::new("time ago").force(),
            ];

            let rows: Vec<[ColoredString; 6]> = list
                .iter()
                .map(|peek| {
                    [
                        peek.base.bold(),
                        peek.version
                            .as_ref()
                            .map(|s| s.normal())
                            .unwrap_or_else(|| "never built".dimmed()),
                        if peek.devel { "X".dimmed() } else { "".dimmed() },
                        if peek.enabled { "X".yellow() } else { "".dimmed() },
                        peek.build
                            .as_ref()
                            .map(|p| p.state.colored_passive())
                            .unwrap_or_else(|| "none".dimmed()),
                        peek.build
                            .as_ref()
                            .map(|p| {
                                let duration = Utc::now() - p.ended.unwrap_or(p.started);
                                let string = ago::coarse(duration);

                                if duration.num_weeks() > 0 {
                                    string.dimmed()
                                } else {
                                    string.normal()
                                }
                            })
                            .unwrap_or("never".to_string().bold()),
                    ]
                })
                .collect();

            if rows.is_empty() {
                println!("{}\n", "no packages added yet".dimmed())
            } else {
                table(columns, rows, "  ");
            }
        }
        Err(e) => log.fail(&e.msg()),
    }
}

/// get information about package and its builds
pub fn info(c: &Config, package: &str, all: bool) {
    check_version_mismatch(c);

    let mut log = Log::start("loading package information and builds");

    // fetch information
    log.next("fetching package information");
    let info = match get_package(c, package) {
        Ok(info) => info,
        Err(e) => {
            log.fail(&format!("failed to get package info: {}", &e.msg()));
            return;
        }
    };

    log.next("fetching latest package builds");
    let builds = match get_builds(c, package, if all { None } else { Some(8) }) {
        Ok(build) => build,
        Err(e) => {
            log.fail(&format!("failed to fetch builds: {}", &e.msg()));
            return;
        }
    };

    log.succeed("successfully loaded package information");

    // output stuff
    println!();
    println!("{}", info.base.bold());
    if let Some(desc) = info.description {
        println!("{}", desc.italic());
    }
    if let Some(url) = info.upstream_url {
        println!("{}", url.dimmed());
    }

    println!();

    println!(
        "{:<9} {}",
        "members:",
        if info.members.is_empty() { "none yet".dimmed() } else { info.members.join(" ").normal() }
    );
    println!("{:<9} {}", "added:", info.added.with_timezone(&Local).format("%x %X"));
    println!(
        "{:<9} {} {}",
        "source:",
        info.source,
        info.source_url.map(|s| format!("({s})")).unwrap_or_default().dimmed()
    );

    let mut tags = vec![];
    if info.enabled {
        tags.push("enabled".yellow())
    } else {
        tags.push("disabled".dimmed())
    }
    if info.clean {
        tags.push("clean".blue())
    }
    if info.devel {
        tags.push("devel".dimmed())
    }
    if info.srcinfo_override {
        tags.push("srcinfo-override".red())
    }
    if info.dependency {
        tags.push("dependency".purple())
    }
    if info.private {
        tags.push("private".bright_black())
    }

    println!(
        "{:<9} {}",
        "status:",
        tags.iter().map(|s| s.to_string()).intersperse(" ".to_string()).collect::<String>()
    );

    println!(
        "{:<9} {}{}",
        "schedule:",
        describe_cron_timezone_hack(&info.schedule)
            .unwrap_or_else(|_| "could not parse cron".to_owned()),
        if info.schedule_changed { " *".red() } else { "".normal() }
    );

    println!(
        "{:<9} {}",
        "flags:",
        if info.makepkg_flags.is_empty() {
            "none".italic().dimmed()
        } else {
            info.makepkg_flags.iter().map(|f| format!("--{f} ")).collect::<String>().normal()
        }
    );

    println!("{:<9} {}", "builds:", info.builds);

    if info.private {
        println!();
        println!("prepare commands: {}", "redacted".dimmed().italic());
    } else if let Some(prepare) = &info.prepare_commands {
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
        Column::new("reason").force(),
        Column::new("date").force(),
        Column::new("time").force(),
    ];

    let rows = builds
        .iter()
        .enumerate()
        .map(|(i, peek)| {
            [
                format!("{:0>4}", info.builds - i as u32 - 1).dimmed(),
                peek.version.as_ref().map(|s| s.normal()).unwrap_or_else(|| "unknown".dimmed()),
                peek.state.colored_substantive(),
                peek.reason.colored(),
                peek.started.with_timezone(&Local).format("%x %X").to_string().normal(),
                peek.ended
                    .map(|ended| ago::fine(ended - peek.started))
                    .map(ColoredString::from)
                    .unwrap_or_else(|| "??".blue()),
            ]
        })
        .collect();

    table(columns, rows, "  ");
}

/// get build information
pub fn build_info(c: &Config, package: &str, build: &Option<String>) {
    let log = Log::start("querying server for the build");

    let id = build.clone().unwrap_or("latest".to_string());

    match get_build(c, package, &id) {
        Ok(b) => {
            log.succeed("found build successfully");

            println!("build for {}", package.bold());
            println!("{:<8} {}", "started:", b.started.with_timezone(&Local).format("%x %X"));
            println!(
                "{:<8} {}",
                "ended:",
                b.ended
                    .map(|s| s.with_timezone(&Local).format("%x %X").to_string())
                    .unwrap_or_else(|| "not yet".to_string())
            );
            println!(
                "{:<8} {}",
                "version:",
                b.version
                    .as_ref()
                    .map(|b| ColoredString::from(b.as_str()))
                    .unwrap_or_else(|| "unknown".dimmed())
            );

            let additive = match &b.state {
                BuildState::Running(state) | BuildState::Fatal(_, state) => {
                    format!("on {}", state.printable_string())
                }
                _ => "".to_string(),
            };

            println!("{:<8} {}", "reason:", b.reason.colored());
            println!("\n{:<8} {} {}", "status:", b.state.colored_substantive(), additive);

            match &b.state {
                BuildState::Failure => {
                    println!("{:<8} {}", "message:", "see logs for error messages".italic())
                }
                BuildState::Fatal(msg, _) => {
                    println!("{:<8} {}", "message:", msg)
                }
                BuildState::Cancelled(msg) => {
                    println!("{:<8} {}", "message:", msg)
                }
                _ => {}
            }


            if b.mem_peak.is_some() || b.cpu_system.is_some() || b.cpu_user.is_some() || b.io_tbr.is_some() || b.io_tbw.is_some() {
                println!("\nbuild stats:")
            }
            if let Some(mem_peak) = b.mem_peak {
                println!("  {:<12} {}", "peak memory:", bytes_str(mem_peak))
            }
            if b.cpu_user.is_some() || b.cpu_system.is_some() {
                let mut line = String::new();
                let both = b.cpu_user.is_some() && b.cpu_system.is_some();
                let mut total = Duration::from_micros(0);

                if let Some(cpu_user) = b.cpu_user {
                    let duration = Duration::from_micros(cpu_user as u64);
                    line += format!("{} user", duration_str(duration)).as_str();
                    total += duration;
                }
                if both {
                    line += ", ";
                }
                if let Some(cpu_system) = b.cpu_system {
                    let duration = Duration::from_micros(cpu_system as u64);
                    line += format!("{} system", duration_str(duration)).as_str();
                    total += duration;
                }

                if both {
                    println!("  {:<12} {} ({})", "cpu time:", duration_str(total), line)
                } else {
                    println!("  {:<12} {}", "cpu time:", line)
                }
            }

            if b.io_tbr.is_some() || b.io_tbw.is_some() {
                let total_read = b.io_tbr.map(bytes_str).unwrap_or("???".bright_black().to_string());
                let total_write = b.io_tbw.map(bytes_str).unwrap_or("???".bright_black().to_string());

                println!("  {:<12} {} read, {} written", "io:", total_read, total_write)
            }
        }
        Err(e) => log.fail(&e.msg()),
    }
}

/// get logs for a build
pub fn build_logs(c: &Config, package: &str, build: &Option<String>) {
    let log = Log::start("fetching last complete build logs");

    let id = build.clone().unwrap_or("latest".to_string());

    match get_build_logs(c, package, &id) {
        Ok(logs) => {
            log.succeed("fetched build logs successfully");
            println!("{logs}")
        }
        Err(e) => log.fail(&e.msg()),
    }
}

/// print the personalized webhook secret for a package
pub fn webhook_secret(c: &Config, package: &str, machine: bool) {
    let log = Log::start("requesting webhook secret");

    match get_webhook_secret(c, package) {
        Ok(secret) => {
            log.succeed("received webhook secret successfully");
            if machine {
                println!("{secret}")
            } else {
                println!(
                    "Your personalized webhook secret for the package {} is:\n{secret}\n",
                    package.italic()
                );
                println!(
                    "To trigger the webhook you have to send a HTTP-{} request to:",
                    "POST".bold()
                );
                println!("{}/webhook/package/{package}/build?secret={secret}", c.url)
            }
        }
        Err(e) => log.fail(&e.msg()),
    }
}

/// print the signing key from the server
pub fn signing_key(c: &Config, machine: bool) {
    let log = Log::start("getting server public key");

    match get_key(c) {
        Ok(key) => {
            log.succeed("successfully got server public key");
            if machine {
                println!("{key}")
            } else {
                println!(
                    "Here's the public key that is used to sign the packages on that server.\n\
                     You can import it into `pacman` to make use of the signatures. Refer to\n{}\n",
                    "https://wiki.archlinux.org/title/Pacman/Package_signing#Adding_unofficial_keys".italic()
                );

                println!(
                    "You can also let this CLI import it semi-automatically for you with:\n\
                     `serene host signatures`\n"
                );

                println!(
                    "You can hide this message with the `-m` flag to pipe the key somewhere,\n\
                     i.e. `serene manage key -m > serene-key.pub`\n"
                );

                println!("{key}");
            }
        }
        Err(e) => log.fail(&e.msg()),
    }
}

/// subscribe to current build logs
pub fn subscribe_build_logs(c: &Config, package: &str, explicit: bool, linger: bool) {
    // we have to use a rc ref cell here because of the closure later down
    let log = RefCell::new(Some(Log::start("looking for existing builds")));

    // skip if explicit subscription
    if !explicit {
        // we ignore failure here, as we just want to check
        if let (Ok(latest), Ok(info)) =
            (get_build_logs(c, package, "latest"), get_build(c, package, "latest"))
        {
            if let Some(s) = log.replace(None) {
                s.succeed("found existing build successfully")
            }

            if linger {
                println!(
                    "{} {}\n{latest}{} {}",
                    "### package build is".italic().dimmed(),
                    BuildState::Pending.colored_passive(),
                    "### package build is".italic().dimmed(),
                    info.state.colored_passive(),
                );
            } else {
                print!("{latest}"); // already has newline at end

                return;
            }
        }
    }

    if let Some(s) = log.borrow_mut().as_mut() {
        s.next("subscribing to live logs and waiting")
    }

    if let Err(err) = subscribe_events(c, package, |_package, event| {
        if let Some(s) = log.replace(None) {
            s.succeed("subscription was successful")
        }

        // ignore unknown events
        match event {
            BroadcastEvent::Ping => {}
            BroadcastEvent::Change(state) => {
                println!("{} {}", "### package build is".italic().dimmed(), state.colored_passive())
            }
            BroadcastEvent::Log(data) => print!("{data}"),
        }

        false // stay attached
    }) {
        if let Some(s) = log.replace(None) {
            s.fail(&err.msg())
        } else {
            Log::failure(&err.msg());
        }
    }
}

/// change a setting on a package
pub fn set_setting(c: &Config, package: &str, setting: SettingsSubcommand) {
    let mut log = Log::start("changing package settings");

    let request = match setting {
        SettingsSubcommand::Clean { enabled } => {
            log.next(&format!(
                "{} clean build for package {package}",
                if enabled { "enabling" } else { "disabling" }
            ));
            PackageSettingsRequest::Clean(enabled)
        }
        SettingsSubcommand::Private { mark } => {
            log.next(&format!(
                "marking package {package} as {}",
                if mark { "private" } else { "not private anymore" }
            ));
            PackageSettingsRequest::Private(mark)
        }
        SettingsSubcommand::Enable { enabled } => {
            log.next(&format!(
                "{} automatic building for package {package}",
                if enabled { "enabling" } else { "disabling" }
            ));
            PackageSettingsRequest::Enabled(enabled)
        }
        SettingsSubcommand::Dependency { set } => {
            log.next(&format!(
                "{} dependency flag for {package}",
                if set { "setting" } else { "removing" }
            ));
            PackageSettingsRequest::Dependency(set)
        }
        SettingsSubcommand::Schedule { cron } => {
            let sched = if cron.trim().is_empty() {
                log.next(&format!("reverting to default schedule for package {package}"));
                None
            } else if let Ok(description) = describe_cron_timezone_hack(&cron) {
                log.next(
                    &format!("setting custom schedule '{description}' for package {package}",),
                );
                Some(cron)
            } else {
                log.fail("invalid cron string provided");
                return;
            };

            PackageSettingsRequest::Schedule(sched)
        }
        SettingsSubcommand::Prepare { command } => {
            let cmd = if command.trim().is_empty() {
                log.next(&format!("removing prepare command for package {package}"));
                None
            } else {
                log.next(&format!("setting prepare command for package {package}"));
                Some(command)
            };

            PackageSettingsRequest::Prepare(cmd)
        }
        SettingsSubcommand::Flags { flags } => {
            let flags = flags
                .iter()
                .map(|s| {
                    MakepkgFlag::from_str(s)
                        .map_err(|_| format!("makepkg flag --{s} not supported"))
                })
                .collect::<Result<Vec<MakepkgFlag>, String>>();

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
        SettingsSubcommand::Devel { devel } => {
            log.next(&format!(
                "making {package} a {} package",
                if devel { "devel" } else { "non-devel" }
            ));
            PackageSettingsRequest::Devel(devel)
        }
        SettingsSubcommand::SrcinfoOverride { force } => {
            log.next(&format!(
                "{} srcinfo override for {package}",
                if force { "enabling" } else { "disabling" }
            ));
            PackageSettingsRequest::SrcinfoOverride(force)
        }
    };

    match set_package_setting(c, package, request) {
        Ok(()) => log.succeed(&format!("updated property for package {package} successfully")),
        Err(e) => log.fail(&e.msg()),
    }
}

/// get the pkgbuild for a specific package
pub fn pkgbuild(c: &Config, package: &str) {
    let log = Log::start("fetching last used pkgbuild");

    match get_package_pkgbuild(c, package) {
        Ok(pkgbuild) => {
            log.succeed("successfully fetched last used pkgbuild");
            println!("{pkgbuild}");
        }
        Err(e) => log.fail(&e.msg()),
    }
}

/// checks for the server version and prints a warning if a mismatch is found
pub fn check_version_mismatch(c: &Config) {
    if let Ok(info) = get_info(c) {
        // strip v- prefix from tags
        let server = info.version.strip_prefix("v").unwrap_or(&info.version);
        let client = env!("TAG").strip_prefix("v").unwrap_or(env!("TAG"));

        if let (Ok(server), Ok(client)) = (Version::parse(server), Version::parse(client)) {
            match server.cmp(&client) {
                std::cmp::Ordering::Less => Log::warning(&format!(
                    "server ({server}) is behind your cli ({client}), update your server"
                )),
                std::cmp::Ordering::Greater => Log::warning(&format!(
                    "cli ({client}) is behind your server ({server}), update your cli"
                )),

                std::cmp::Ordering::Equal => {} // everything is good
            }

            // upgrade from 0.3.x to 0.4.x (the CLI usually updates automatically)
            if server.minor == 3 && client.minor == 4 {
                Log::warning("please read #15 on GitHub before blindly updating your server");
            }
        } else {
            Log::warning("invalid cli or server version, please check for updates")
        }
    } else {
        Log::warning("server version check failed, please check for updates")
    }
}

pub fn server_info(c: &Config) {
    let mut log = Log::start("fetching server information");

    let info = match get_info(c) {
        Ok(info) => info,
        Err(e) => {
            log.fail(&e.msg());
            return;
        }
    };

    log.next("fetching package information");

    let packages = match get_packages(c) {
        Ok(packages) => packages,
        Err(e) => {
            log.fail(&e.msg());
            return;
        }
    };

    let total = packages.len();
    let mut members = 0;
    let mut devel = 0;
    let mut enabled = 0;

    let mut pending = 0;
    let mut working = 0;
    let mut passing = 0;
    let mut failing = 0;
    let mut cancelled = 0;
    let mut fatal = 0;

    for package in packages {
        if package.devel {
            devel += 1;
        }
        if package.enabled {
            enabled += 1;
        }

        if let Some(b) = package.build {
            match b.state {
                BuildState::Pending => pending += 1,
                BuildState::Running(_) => working += 1,
                BuildState::Success => passing += 1,
                BuildState::Failure => failing += 1,
                BuildState::Cancelled(_) => cancelled += 1,
                BuildState::Fatal(_, _) => fatal += 1,
            }
        }

        members += package.members.len();
    }

    log.succeed("successfully fetched server information");

    println!();
    println!("{} {}", "serene".bold(), info.version);
    println!("{:<10} {}/{}", "location:", c.url.italic(), info.architecture);

    // this might have a prefixed space for the tables
    let uptime = ago::coarse(Utc::now() - info.started);
    println!("{:<10} {}", "uptime:", uptime.strip_prefix(" ").unwrap_or(&uptime));

    println!("{:<10} {}", "repo name:", info.name.bold());

    let mut tags = vec![];
    if info.readable {
        tags.push("readable".yellow())
    } else {
        tags.push("unreadable".dimmed())
    }
    if info.signed {
        tags.push("signed".green())
    }

    println!(
        "{:<10} {}",
        "features:",
        tags.iter().map(|s| s.to_string()).intersperse(" ".to_string()).collect::<String>()
    );

    println!();
    println!("package overview:");

    println!("  {:<8} {total} ({members} members available)", "amount:");
    println!(
        "  {:<8} {}/{}/{}/{}/{}/{}",
        "status:",
        passing.to_string().green(),
        working.to_string().blue(),
        failing.to_string().red(),
        cancelled.to_string().yellow(),
        pending.to_string().dimmed(),
        fatal.to_string().bright_red()
    );
    println!("  {:<8} {} of {total}", "enabled:", enabled.to_string().yellow());
    println!("  {:<8} {} of {total}", "devel:", devel.to_string().dimmed());

    println!();
    println!("this host:");

    // strip v- prefix from tags
    let server = info.version.strip_prefix("v").unwrap_or(&info.version);
    let client = env!("TAG").strip_prefix("v").unwrap_or(env!("TAG"));

    let message = if let (Ok(server), Ok(client)) = (Version::parse(server), Version::parse(client))
    {
        match server.cmp(&client) {
            std::cmp::Ordering::Less => Some("update your server"),
            std::cmp::Ordering::Greater => Some("update your cli"),
            std::cmp::Ordering::Equal => None,
        }
    } else {
        Some("something went wrong")
    };

    println!(
        "  {:<12} {} ({})",
        "cli version:",
        if message.is_some() { env!("TAG").red() } else { env!("TAG").normal() },
        message.unwrap_or("up-to-date")
    );
    println!(
        "  {:<12} {}",
        "achitecture:",
        if ARCH == info.architecture { "compatible".normal() } else { "incompatible".red() }
    )
}
