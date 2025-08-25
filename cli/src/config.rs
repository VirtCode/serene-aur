use anyhow::{Context, Result};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use serene_data::secret;
use std::path::{Path, PathBuf};
use std::{env, fs};

const CONFIG_FILE: &str = "serene.yml";
const SECRET_FILE: &str = "serene/secret.txt";

/// gets the file the configuration is stored at
fn config_file() -> PathBuf {
    Path::new(
        &env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| format!("{}/.config", env::var("HOME").expect("$HOME not set?"))),
    )
    .join(CONFIG_FILE)
}

/// gets the file the secret is stored in
fn secret_file() -> PathBuf {
    Path::new(
        &env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            format!("{}/.local/share", env::var("HOME").expect("$HOME not set?"))
        }),
    )
    .join(SECRET_FILE)
}

/// generates a fresh secret
fn generate_secret() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 64)
}

/// get default root elevator
fn default_elevator() -> String {
    "sudo".to_string()
}

/// get default root elevator
fn secret_placeholder() -> String {
    "empty".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    #[serde(default = "secret_placeholder", skip_serializing)]
    pub secret: String,

    #[serde(default = "default_elevator", skip_serializing)]
    pub elevator: String,
}

impl Config {
    /// checks whether the config exists
    pub fn exists() -> bool {
        config_file().is_file()
    }

    /// creates a empty config with only a url
    pub fn empty(url: String) -> Self {
        Self { secret: secret_placeholder(), url, elevator: secret_placeholder() }
    }

    /// reads or creates a config
    pub fn read() -> anyhow::Result<Self> {
        let string =
            fs::read_to_string(config_file()).context("failed to read configuration file")?;

        let mut config: Self =
            serde_yaml::from_str(&string).context("failed to deserialize configuration file")?;

        // read secret from file
        // yes, this means you can't use "empty" as your secret
        // and this is indeed a bit sketchy but has the most ergonomic result
        if config.secret == secret_placeholder() {
            config.secret = Self::read_or_generate_secret()?;
        }

        Ok(config)
    }

    pub fn write(self) -> anyhow::Result<Self> {
        let file = config_file();

        let string =
            serde_yaml::to_string(&self).context("failed to serialize configuration file")?;

        // create .config if doesn't exist
        if let Some(parent) = file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).context("failed to create config directory")?;
            }
        }

        fs::write(&file, string).context("failed to save new configuration file")?;

        // now read saved config (and generate secret if required)
        Self::read()
    }

    fn read_or_generate_secret() -> Result<String> {
        let file = secret_file();

        if file.exists() {
            fs::read_to_string(&file)
                .map(|s| s.trim().to_string())
                .context("failed to read secret file")
        } else {
            if let Some(parent) = file.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).context("failed to create secret directory")?;
                }
            }

            let secret = generate_secret();
            fs::write(&file, &secret).context("failed to store new secret")?;
            Ok(secret)
        }
    }

    /// prints the hashed secret to stdout together with host and username
    pub fn print_secret(&self, nice: bool) {
        let hash = secret::hash(&self.secret);

        if nice {
            println!("{hash} {}@{}", whoami::username(), whoami::hostname())
        } else {
            println!("{hash}")
        }
    }
}
