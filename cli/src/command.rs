use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[clap(version = option_env!("TAG").unwrap_or("unknown"), about)]
#[command(disable_help_subcommand = true, bin_name = "serene")]
pub struct Args {
    #[clap(subcommand)]
    pub command: Action,

    /// override the host url that is used
    #[clap(short, long)]
    pub server: Option<String>,
}

#[derive(Subcommand)]
pub enum Action {
    /// list all packages which are added
    List,

    /// adds a package
    Add {
        /// what to add, by default aur package name
        what: String,

        /// <WHAT> is a custom repository
        #[clap(short, long, group = "nonaur", help_heading = "Custom Sources")]
        custom: bool,

        /// <WHAT> is a custom pkgbuild
        #[clap(short, long, group = "nonaur", help_heading = "Custom Sources")]
        pkgbuild: bool,

        /// add as a development package
        #[clap(short, long, requires = "nonaur", help_heading = "Custom Sources")]
        devel: bool,

        /// replace existing package with same base
        #[clap(short, long)]
        replace: bool,

        /// do not resolve dependencies for the package
        #[clap(short, long)]
        noresolve: bool,

        /// read the contents for <WHAT> from a file
        #[clap(short, long)]
        file: bool,

        /// install package with `pacman` after successful build
        #[clap(short, long, group = "logs", help_heading = "Installing")]
        install: bool,

        /// just listen, don't install after the build is finished
        #[clap(short, long, group = "logs", help_heading = "Installing")]
        listen: bool,

        /// do not print logs during the build
        #[clap(short, long, requires = "logs", help_heading = "Installing")]
        quiet: bool,
    },

    /// removes a package
    Remove {
        /// base name of the package
        name: String,
    },

    /// schedule immediate builds for packages
    Build {
        /// names of the package bases to build
        #[clap(required_unless_present = "all")]
        names: Vec<String>,

        /// force clean before the next build
        #[clap(short, long)]
        clean: bool,

        /// do not resolve dependency order to build in
        #[clap(short, long)]
        noresolve: bool,

        /// do not build if the package is up-to-date
        #[clap(short, long)]
        gentle: bool,

        /// install package with `pacman` after successful build
        #[clap(short, long, group = "logs", help_heading = "Installing")]
        install: bool,

        /// just listen, don't install after the build is finished
        #[clap(short, long, group = "logs", help_heading = "Installing")]
        listen: bool,

        /// do not print logs during the build
        #[clap(short, long, requires = "logs", help_heading = "Installing")]
        quiet: bool,

        /// build all packages of the repository instead, implies `--gentle`
        #[clap(short, long, conflicts_with_all = ["names", "logs", "gentle"], help_heading = "All")]
        all: bool,

        /// force the build of all packages, including up-to-date
        #[clap(short, long, requires = "all", help_heading = "All")]
        force: bool,
    },

    /// get and set info about a package
    Info {
        /// base name of the package
        name: String,

        /// show all builds
        #[clap(short, long)]
        all: bool,

        /// what type of info to get
        #[clap(subcommand)]
        what: Option<InfoCommand>,
    },

    /// prints the current secret
    Secret {
        /// print the secret in a machine-readable way
        #[clap(short, long)]
        machine: bool,
    },

    /// manage things related to the server
    Manage {
        #[clap(subcommand)]
        manage: ManageSubcommand,
    },

    #[command(hide = true)]
    Completions,
}

#[derive(Subcommand)]
pub enum InfoCommand {
    /// get information about a build
    Build {
        /// id of the build, latest if empty
        id: Option<String>,
    },

    /// get logs from a build
    Logs {
        /// id of the build, latest if empty
        id: Option<String>,

        /// explicitly subscribe to live logs
        #[clap(short, long)]
        subscribe: bool,

        /// stay attached indefinitely
        #[clap(short, long)]
        linger: bool,
    },

    /// get the pkgbuild used to build the current package
    Pkgbuild,

    /// set property of the package
    Set {
        /// property to set
        #[clap(subcommand)]
        property: SettingsSubcommand,
    },
}

#[derive(Subcommand)]
pub enum ManageSubcommand {
    /// get info about the given server
    Info,

    /// get a personalized webhook secret for a package
    Webhook {
        /// name of the package
        name: String,

        /// print the secret in a machine-readable way
        #[clap(short, long)]
        machine: bool,
    },
}

#[derive(Subcommand)]
pub enum SettingsSubcommand {
    /// enable or disable clean build
    Clean {
        /// remove container after build
        #[arg(action = ArgAction::Set)]
        enabled: bool,
    },

    /// enable or disable automatic package building
    Enable {
        /// enable automatic building
        #[arg(action = ArgAction::Set)]
        enabled: bool,
    },

    /// set the dependency mark of the package
    Dependency {
        /// the package was added as a dependency
        #[arg(action = ArgAction::Set)]
        set: bool,
    },

    /// set custom schedule
    Schedule {
        /// cron string of schedule
        cron: String,
    },

    /// set prepare command
    Prepare {
        /// commands to be run before build
        command: String,
    },

    /// set additional makepkg flags
    Flags {
        /// flags to add, without the dashes
        flags: Vec<String>,
    },
}
