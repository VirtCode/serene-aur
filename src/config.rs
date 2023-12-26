use lazy_static::lazy_static;
use std::env;

lazy_static! {
    pub static ref CONFIG: Config = Config::env();
}

pub struct Config {
    /// the architecture of the build container
    pub architecture: String,
    /// the name of the exposed repository
    pub repository_name: String
}

impl Config {
    fn env() -> Self {
        Self {
            architecture: env::var("ARCH").unwrap_or(env::consts::ARCH.to_owned()),
            repository_name: env::var("NAME").unwrap_or("serene".to_string())
        }
    }
}