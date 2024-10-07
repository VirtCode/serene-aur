use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use log::warn;
use std::env;
use std::str::FromStr;

pub const SOURCE_REPOSITORY: &str = "https://github.com/VirtCode/serene-aur";
pub const RUNNER_CONTAINER_NAME: &str = "ghcr.io/virtcode/serene-aur-runner:edge-{version}";
pub const CLI_PACKAGE_NAME: &str = "serene-cli";

lazy_static! {
    pub static ref INFO: Info = Info::start();
}

pub struct Info {
    pub start_time: DateTime<Utc>,
    pub version: String,
}

impl Info {
    fn start() -> Self {
        Self { start_time: Utc::now(), version: env!("TAG").to_string() }
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config::env();
}

pub struct Config {
    /// allow reading information for non-authenticated clients
    pub allow_reads: bool,
    /// the architecture of the build container
    pub architecture: String,
    /// the name of the exposed repository
    pub repository_name: String,
    /// password for private key used for signatures
    pub sign_key_password: Option<String>,
    /// default scheduling of packages
    pub schedule_default: String,
    /// scheduling of development packages
    pub schedule_devel: String,
    /// schedule for pulling the runner image
    pub schedule_image: String,
    /// container name prefix xxxxx-my-package
    pub container_prefix: String,
    /// runner docker image
    pub runner_image: String,
    /// prune old images on server
    pub prune_images: bool,
    /// custom url for docker instance to use
    pub docker_url: Option<String>,
    /// port to bind to
    pub port: u16,
    /// build the cli by default
    pub build_cli: bool,
    /// url for runners to reach the server to pull dependencies from its repo
    pub own_repository_url: Option<String>,
    /// secret used to sign webhook tokens
    pub webhook_secret: Option<String>,
    /// mirror used to sychronize package dbs
    pub sync_mirror: String,
    /// build the packages in the sequence they depend on each other
    pub resolve_build_sequence: bool,
    /// still build depending packages even if dependency failed
    pub resolve_ignore_failed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allow_reads: false,

            architecture: env::consts::ARCH.to_string(),
            repository_name: "serene".to_string(),
            sign_key_password: None,

            schedule_default: "0 0 0 * * *".to_string(), // 00:00 UTC every day
            schedule_devel: "0 0 0 * * *".to_string(),
            schedule_image: "0 0 0 * * *".to_string(),

            container_prefix: "serene-aur-runner-".to_string(),
            runner_image: RUNNER_CONTAINER_NAME.to_string(),
            prune_images: true,

            docker_url: None,

            port: 80,
            build_cli: true,
            own_repository_url: None,

            webhook_secret: None,

            resolve_build_sequence: true,
            resolve_ignore_failed: false,

            sync_mirror: "https://mirror.init7.net/archlinux/{repo}/os/{arch}".to_string(),
        }
    }
}

impl Config {
    fn env_string_option(name: &str, default: Option<String>) -> Option<String> {
        env::var(name).ok().or(default)
    }

    fn env_string(name: &str, default: String) -> String {
        env::var(name).unwrap_or(default)
    }

    fn env_u16(name: &str, default: u16) -> u16 {
        env::var(name)
            .ok()
            .and_then(|s| {
                u16::from_str(&s).map_err(|_| warn!("failed to parse {name}, using default")).ok()
            })
            .unwrap_or(default)
    }

    fn env_bool(name: &str, default: bool) -> bool {
        env::var(name)
            .ok()
            .and_then(|s| {
                bool::from_str(&s).map_err(|_| warn!("failed to parse {name}, using default")).ok()
            })
            .unwrap_or(default)
    }

    #[rustfmt::skip]
    fn env() -> Self {
        let default = Self::default();

        Self {
            allow_reads: Self::env_bool("ALLOW_READS", default.allow_reads),

            architecture: Self::env_string("ARCH", default.architecture),
            repository_name: Self::env_string("NAME", default.repository_name),
            sign_key_password: Self::env_string_option("SIGN_KEY_PASSWORD", default.sign_key_password),

            schedule_image: Self::env_string("SCHEDULE_IMAGE", default.schedule_image),
            schedule_devel: Self::env_string( "SCHEDULE_DEVEL", Self::env_string("SCHEDULE", default.schedule_devel)),
            schedule_default: Self::env_string("SCHEDULE", default.schedule_default),

            container_prefix: Self::env_string("RUNNER_PREFIX", default.container_prefix),
            runner_image: Self::env_string("RUNNER_IMAGE", default.runner_image),
            prune_images: Self::env_bool("PRUNE_IMAGES", default.prune_images),

            docker_url: Self::env_string_option("DOCKER_URL", default.docker_url),

            port: Self::env_u16("PORT", default.port),
            build_cli: Self::env_bool("BUILD_CLI", default.build_cli),
            own_repository_url: Self::env_string_option("OWN_REPOSITORY_URL", default.own_repository_url),

            resolve_build_sequence: Self::env_bool("RESOLVE_BUILD_SEQUENCE", default.resolve_build_sequence),
            resolve_ignore_failed: Self::env_bool("RESOLVE_IGNORE_FAILED", default.resolve_ignore_failed),

            webhook_secret: Self::env_string_option("WEBHOOK_SECRET", default.webhook_secret),

            sync_mirror: Self::env_string("SYNC_MIRROR", default.sync_mirror),
        }
    }
}
