use clap::{Parser, Subcommand, ArgAction};

#[derive(Parser)]
#[clap(version, about)]
#[command(disable_help_subcommand = true)]
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

        /// name is custom repository
        #[clap(short, long)]
        pkgbuild: bool,

        /// name is custom repository
        #[clap(short, long)]
        custom: bool,

        /// is development package, ignored for aur
        #[clap(short, long)]
        devel: bool,

        /// replace source with new
        #[clap(short, long)]
        replace: bool,
    },

    /// removes a package
    Remove {
        /// base name of the package
        name: String
    },

    /// schedules an immediate build for a package
    Build {
        /// base name of the package
        name: String,

        /// force clean before the next build
        #[clap(short, long)]
        clean: bool
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
        what: Option<InfoCommand>
    },

    /// prints the current secret
    Secret {
        /// print the secret in a machine readable way
        #[clap(short, long)]
        machine: bool
    },
    
    #[command(hide = true)]
    Completions
}

#[derive(Subcommand)]
pub enum InfoCommand {
    /// get information about a build
    Build {
        /// id of the build, latest if empty
        id: Option<String>
    },

    /// get logs from a build
    Logs {
        /// id of the build, latest if empty
        id: Option<String>,

        /// subscribe and attach to live logs
        #[clap(short, long)]
        subscribe: Option<bool>
    },

    /// get the pkgbuild used to build the current package
    Pkgbuild,

    /// set property of the package
    Set {
        /// property to set
        #[clap(subcommand)]
        property: SettingsSubcommand
    }
}

#[derive(Subcommand)]
pub enum SettingsSubcommand {
    /// enable or disable clean build
    Clean {
        /// remove container after build
        #[arg(action = ArgAction::Set)]
        enabled: bool
    },

    /// enable or disable automatic package building
    Enable {
        /// enable automatic building
        #[arg(action = ArgAction::Set)]
        enabled: bool
    },

    /// set custom schedule
    Schedule {
        /// cron string of schedule
        cron: String
    },

    /// set prepare command
    Prepare {
        /// commands to be run before build
        command: String
    },

    /// set additional makepkg flags
    Flags {
        /// flags to add, space separated, full without dashes
        flags: String
    },
}