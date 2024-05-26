use std::{env, fs};
use std::io::stdin;
use std::path::{Path, PathBuf};
use std::process::exit;
use anyhow::Context;
use colored::Colorize;
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use serene_data::secret;

const CONFIG_FILE: &str = "serene.yml";

/// gets the file the configuration is stored at
fn file() -> PathBuf {
    Path::new(&env::var("XDG_CONFIG_HOME").unwrap_or_else(|_|
        format!("{}/.config", env::var("HOME").expect("$HOME not set?"))
    )).join(CONFIG_FILE)
}

/// generates a fresh secret
fn generate_secret() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 64)
}

/// get default root elevator
fn default_elevator() -> String {
    "sudo".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub secret: String,
    pub url: String,
    #[serde(default = "default_elevator")]
    pub elevator: String
}

impl Config {

    /// reads or creates a config
    pub fn create() -> anyhow::Result<Self> {
        let file = file();

        if file.exists() {
            let string = fs::read_to_string(&file)
                .context("failed to read configuration file")?;

            Ok(serde_yaml::from_str(&string)
                .context("failed to deserialize configuration file")?)
        } else {
            let config = Config::intro()?;

            let string = serde_yaml::to_string(&config)
                .context("failed to serialize configuration file")?;

            fs::write(&file, string)
                .context("failed to save new configuration file")?;

            exit(0);
        }
    }

    /// prints the intro sequence which walks the user through adding the secret
    fn intro() -> anyhow::Result<Self> {
        println!("Welcome to {}!", "serene".bold());

        println!();
        println!("1. In order to use this cli, you need to host the corresponding build server.");
        println!("Please enter the url to that server:");
        let mut url = String::new();
        stdin().read_line(&mut url)
            .context("couldn't read line from stdin")?;
        url = url.trim().to_owned();

        println!();
        println!("2. Great, now add the following line to its {} file:", "authorized_secrets".italic());

        let secret = generate_secret();
        let config = Self::new(secret, url);
        config.print_secret(true);

        println!();
        println!("3. To now use the repository with your pacman, add the following to your pacman configuration:");
        println!("[serene]                        # or something else if you've changed that");
        println!("SigLevel = Optional TrustAll    # signatures are not yet supported");
        println!("Server = {}/{}", &config.url, env::consts::ARCH);

        println!();
        println!("After that, you're all set and ready to go!");

        Ok(config)
    }
    
    /// creates a new config with default values
    fn new(secret: String, url: String) -> Self {
        Self {
            secret, url,
            elevator: default_elevator()
        }
    }

    /// prints the hashed secret to stdout together with host and username
    pub fn print_secret(&self, nice: bool) {
        let hash = secret::hash(&self.secret);

        if nice { println!("{hash} {}@{}", whoami::username(), whoami::hostname()) }
        else { println!("{hash}") }
    }
}