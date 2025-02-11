use crate::package::source::SourceImpl;
use crate::package::{aur, git};
use async_trait::async_trait;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// this is a source which pulls from the AUR, checking via RPC for updates
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AurSource {
    base: String,
    version: String,
}

impl AurSource {
    pub fn new(base: &str) -> Self {
        Self { base: base.to_owned(), version: "".to_owned() }
    }

    pub async fn get_version(&self) -> anyhow::Result<String> {
        let package = aur::info(&self.base).await?;

        if let Some(package) = package {
            Ok(package.version)
        } else {
            warn!(
                "could not get version for aur package {}, it seems to be no longer on the AUR",
                self.base
            );

            Ok("nonexistent".to_owned())
        }
    }
}

#[async_trait]
impl SourceImpl for AurSource {
    async fn initialize(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("initializing aur source for {}", self.base);

        git::clone(&aur::get_repository(&self.base), folder).await?;
        self.version = self.get_version().await?;

        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        Some(aur::get_listing(&self.base))
    }

    fn get_type(&self) -> String {
        "aur package".to_string()
    }

    fn get_state(&self) -> String {
        self.version.clone()
    }

    async fn update(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("updating aur source for {}", self.base);

        let version = self.get_version().await?;

        // only update if version has changed
        if version != self.version {
            git::pull(folder).await?;
            self.version = version;
        }

        Ok(())
    }
}
