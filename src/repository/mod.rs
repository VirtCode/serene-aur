use std::path::PathBuf;
use actix_files::Files;
use anyhow::{anyhow, Context};
use async_tar::Entries;
use futures_util::AsyncRead;
use log::__private_api::enabled;
use tokio::fs;
use crate::build::archive;
use crate::package::Package;

mod manage;

const REPO_DIR: &str = "./app/repository";
const ARCH: &str = "x86_64";

/// returns the webservice which exposes the repository
pub fn webservice() -> Files {
    Files::new(ARCH, REPO_DIR)
        .show_files_listing()
}

pub struct PackageRepository {
    folder: PathBuf,
    name: String,
    packages: Vec<RepositoryEntry>
}

pub struct RepositoryEntry {
    base: String,
    files: Vec<String>
}

impl PackageRepository {

    pub fn new(folder: PathBuf, name: String) -> Self {
        Self {
            folder, name,
            packages: vec![]
        }
    }

    pub async fn publish(&mut self, package: &Package, mut output: Entries<impl AsyncRead + Unpin + Sized>) -> anyhow::Result<()> {
        fs::create_dir_all(&self.folder).await
            .context("failed to create folder for repository")?;

        let files = package.expected_files().await
            .context("failed to construct expected files from package")?;

        // get or create entry
        let entry =
            if let Some(p) = self.packages.iter_mut().find(|e| e.base == package.base) { p } else {
                self.packages.push(RepositoryEntry { base: package.base.clone(), files: vec![]});
                self.packages.last_mut().expect("item was just added")
            };

        // remove old files from repository
        manage::remove(&self.name, &entry.files, &self.folder).await
            .context("failed to remove files from repository")?;

        // delete package files
        for x in &entry.files {
            fs::remove_file(self.folder.join(x)).await
                .context(format!("failed to delete file from repository: {x}"))?
        }

        entry.files = vec![];

        // extract package files
        archive::extract_files(&mut output, &files, &self.folder).await
            .context("failed to extract all packages from build container")?;

        // add package files
        manage::add(&self.name, &files, &self.folder).await
            .context("failed to add files to repository")?;

        entry.files = files;

        Ok(())
    }

    async fn remove(&mut self, package: &Package) -> anyhow::Result<()> {
        let pos = self.packages.iter().position(|p| p.base == package.base)
            .context(anyhow!("could not find package {} in repository", &package.base))?;

        let entry = self.packages.remove(pos);

        // remove files from repository
        manage::remove(&self.name, &entry.files, &self.folder).await
            .context("failed to remove files from repository")?;

        // delete package files
        for x in &entry.files {
            fs::remove_file(self.folder.join(x)).await
                .context(format!("failed to delete file from repository: {x}"))?
        }

        Ok(())
    }
}