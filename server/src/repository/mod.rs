use crate::config::CONFIG;
use crate::package::Package;
use crate::runner::archive;
use actix_files::Files;
use anyhow::{anyhow, Context};
use async_tar::Entries;
use futures_util::AsyncRead;
use hyper::body::HttpBody;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

pub mod crypto;
mod manage;

const REPO_DIR: &str = "repository";
const REPO_SERENE: &str = "bases.json";
const PRIV_KEY_FILE: &str = "sign_key.asc";

/// returns the webservice which exposes the repository
pub fn webservice() -> Files {
    Files::new(&CONFIG.architecture, REPO_DIR).show_files_listing()
}

pub struct PackageRepository {
    name: String,
    bases: HashMap<String, Vec<PackageEntry>>,
}

#[derive(Serialize, Deserialize)]
struct PackageEntry {
    name: String,
    file: String,
}

impl PackageRepository {
    /// creates a new package repository
    pub async fn new() -> anyhow::Result<Self> {
        let mut s = Self { name: CONFIG.repository_name.to_owned(), bases: HashMap::new() };

        s.load().await?;

        Ok(s)
    }

    /// loads the current bases file from disk
    async fn load(&mut self) -> anyhow::Result<()> {
        let path = Path::new(REPO_DIR).join(REPO_SERENE);
        if !path.is_file() {
            return Ok(());
        }

        let string =
            fs::read_to_string(path).await.context("failed to read database summary from file")?;

        self.bases =
            serde_json::from_str(&string).context("failed to deserialize database summary")?;

        Ok(())
    }

    /// saves the current bases file to disk
    async fn save(&self) -> anyhow::Result<()> {
        let path = Path::new(REPO_DIR).join(REPO_SERENE);

        let string =
            serde_json::to_string(&self.bases).context("failed to serialize serene database")?;

        fs::write(path, string).await.context("failed to write serene database to file")?;

        Ok(())
    }

    /// publishes the files for a package on the repository
    pub async fn publish(
        &mut self,
        package: &Package,
        mut output: Entries<impl AsyncRead + Unpin + Sized>,
    ) -> anyhow::Result<()> {
        fs::create_dir_all(REPO_DIR).await.context("failed to create folder for repository")?;

        let files = package
            .expected_files()
            .await
            .context("failed to construct expected files from package")?;

        // remove old things if present
        if let Some(entries) = self.bases.get(&package.base) {
            // remove old files from repository
            if let Err(e) = manage::remove(
                &self.name,
                &entries.iter().map(|e| e.name.clone()).collect(),
                Path::new(REPO_DIR),
            )
            .await
            {
                warn!("failed to remove files from repository: {e:#}");
            }

            // delete package files
            for entry in entries {
                if let Err(e) = fs::remove_file(Path::new(REPO_DIR).join(&entry.file)).await {
                    warn!("failed to delete file from repository ({e}): {}", entry.file);
                }
            }
        }

        // extract package files
        archive::extract_files(&mut output, &files, Path::new(REPO_DIR))
            .await
            .context("failed to extract all packages from build container")?;

        // sign packages if enabled
        if crypto::should_sign_packages() {
            manage::sign(&files, Path::new(REPO_DIR)).await.context("failed to sign packages")?;
        }

        // add package files
        manage::add(&self.name, &files, Path::new(REPO_DIR))
            .await
            .context("failed to add files to repository")?;

        // create entries, assuming they have the right order
        let entries = package
            .get_packages()
            .into_iter()
            .zip(files)
            .map(|(name, file)| PackageEntry { name, file })
            .collect();

        self.bases.insert(package.base.clone(), entries);
        self.save().await?;

        Ok(())
    }

    /// removes a package from the repository
    pub async fn remove(&mut self, package: &Package) -> anyhow::Result<()> {
        if let Some(entries) = self.bases.remove(&package.base) {
            // remove old files from repository
            manage::remove(
                &self.name,
                &entries.iter().map(|e| e.name.clone()).collect(),
                Path::new(REPO_DIR),
            )
            .await
            .context("failed to remove files from repository")?;

            // delete package (and signature) files
            for entry in entries {
                fs::remove_file(Path::new(REPO_DIR).join(&entry.file))
                    .await
                    .context(format!("failed to delete file from repository: {}", entry.file))?;

                let sign_path = Path::new(REPO_DIR).join(format!("{}.sig", entry.file));
                if sign_path.exists() {
                    fs::remove_file(sign_path).await.context(format!(
                        "failed to delete signature file from repository: {}.sig",
                        entry.file
                    ))?;
                }
            }
        } else {
            return Err(anyhow!("could not find package {} in repository", &package.base));
        }

        self.save().await?;

        Ok(())
    }
}
