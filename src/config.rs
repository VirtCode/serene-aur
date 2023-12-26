use lazy_static::lazy_static;
use std::env;

lazy_static! {
    pub static ref CONFIG: Config = Config::env();
}

pub struct Config {
    /// the architecture of the build container
    pub architecture: String,
    /// the name of the exposed repository
    pub repository_name: String,
    /// default scheduling of packages
    pub schedule_default: String,
    /// scheduling of development packages
    pub schedule_devel: String,
    /// container name prefix xxxxx-my-package
    pub container_prefix: String
}

impl Config {
    fn env() -> Self {
        let schedule = env::var("SCHEDULE").unwrap_or("0 0 0 * * *".to_string());

        Self {
            architecture: env::var("ARCH").unwrap_or(env::consts::ARCH.to_owned()),
            repository_name: env::var("NAME").unwrap_or("serene".to_string()),
            schedule_devel: env::var("SCHEDULE_DEVEL").unwrap_or(schedule.clone()),
            schedule_default: schedule,
            container_prefix: env::var("RUNNER_PREFIX").unwrap_or("serene-aur-runner-".to_string())
        }
    }
}