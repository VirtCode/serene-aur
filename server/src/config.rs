use lazy_static::lazy_static;
use std::env;
use std::str::FromStr;
use anyhow::Context;
use log::warn;

pub const SOURCE_REPOSITORY: &str = "https://github.com/VirtCode/serene-aur";
pub const CLI_PACKAGE_NAME: &str = "serene-cli";

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
    /// custom url for docker instance to use
    pub docker_url: Option<String>,
    /// port to bind to
    pub port: u16,
    /// build the cli by default
    pub build_cli: bool,
    /// url for runners to reach the server to pull dependencies from its repo
    pub own_repository_url: Option<String>,
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
            runner_image: "ghcr.io/virtcode/serene-aur-runner:main".to_string(),
            
            docker_url: None,

            port: 80,
            build_cli: true,
            own_repository_url: None
        }
    }
}

impl Config {
    fn env() -> Self {
        let default = Self::default();

        Self {
            allow_reads: env::var("ALLOW_READS").ok()
                .and_then(|s| bool::from_str(&s).map_err(|_| warn!("failed to parse ALLOW_READS, using default")).ok())
                .unwrap_or(default.allow_reads),

            architecture: env::var("ARCH").unwrap_or(default.architecture),
            repository_name: env::var("NAME").unwrap_or(default.repository_name),
            sign_key_password: env::var("SIGN_KEY_PASSWORD").ok(),
            own_repository_url: env::var("OWN_REPOSITORY_URL").ok().or(default.own_repository_url),

            schedule_image: env::var("SCHEUDLE_IMAGE").unwrap_or(default.schedule_image),
            schedule_devel: env::var("SCHEDULE_DEVEL").or(env::var("SCHEUDLE")).unwrap_or(default.schedule_devel.clone()),
            schedule_default: env::var("SCHEUDLE").unwrap_or(default.schedule_default),

            container_prefix: env::var("RUNNER_PREFIX").unwrap_or(default.container_prefix),
            runner_image: env::var("RUNNER_IMAGE").unwrap_or(default.runner_image),
            
            docker_url: env::var("DOCKER_URL").ok().or(default.docker_url),

            port: env::var("PORT").ok()
                .and_then(|s| u16::from_str(&s).map_err(|_| warn!("failed to parse PORT, using default")).ok())
                .unwrap_or(default.port),
            build_cli: env::var("BUILD_CLI").ok()
                .and_then(|s| bool::from_str(&s).map_err(|_| warn!("failed to parse BUILD_CLI, using default")).ok())
                .unwrap_or(default.build_cli)
        }
    }
}