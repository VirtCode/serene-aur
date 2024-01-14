use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(version, about)]
#[command(disable_help_subcommand = true)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,

    /// override the host url that is used
    #[clap(short, long)]
    pub server: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// list all packages which are added
    List,

    /// adds a package
    Add {
        /// base name for the aur package or custom url
        name: String,

        /// name is custom repository
        #[clap(short, long)]
        custom: bool,

        /// is development package, only works on custom urls
        #[clap(short, long)]
        devel: bool,
    },

    /// removes a package
    Remove {
        /// base name of the package
        name: String
    },

    /// schedules an immediate build for a package
    Build {
        /// base name of the package
        name: String
    },

    /// get info about a package
    Info {
        /// base name of the package
        name: String,

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
}

#[derive(Subcommand)]
pub enum InfoCommand {
    /// get information about a build
    Build {
        /// id of the build
        id: String
    },

    /// get logs from a build
    Logs {
        /// id of the build
        id: String
    },
}