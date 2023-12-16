use std::collections::HashMap;
use std::path::Path;
use async_trait::async_trait;
use log::debug;
use srcinfo::Srcinfo;
use crate::package::git;

const SRCINFO: &str = ".SRCINFO";

#[async_trait]
pub trait PackageSource {
    // creates an empty package source for a repository, create should be called afterwards
    fn empty(repository: &str) -> Self;

    // pulls the package sources for the first time
    async fn create(&mut self, folder: &Path) -> anyhow::Result<()>;

    // checks whether an update would be available
    async fn update_available(&self) -> anyhow::Result<bool>;

    // upgrades the sources
    async fn upgrade(&mut self, folder: &Path) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct NormalSource {
    repository: String,
    last_commit: String
}

#[async_trait]
impl PackageSource for NormalSource {
    fn empty(repository: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            last_commit: "".to_owned()
        }
    }

    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("creating {}", self.repository);
        git::clone(&self.repository, folder).await?;

        self.upgrade(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        debug!("updating {}", &self.repository);

        let current_commit = git::latest_commit(&self.repository).await?;
        Ok(current_commit != self.last_commit)
    }

    async fn upgrade(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("upgrading {}", &self.repository);

        // pull repo
        git::pull(folder).await?;
        // set commit to newest (could also be done by looking at the local repository...)
        self.last_commit = git::latest_commit(&self.repository).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct DevelSource {
    repository: String,
    last_commit: String,
    last_source_commits: HashMap<String, String>
}

#[async_trait]
impl PackageSource for DevelSource {

    fn empty(repository: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            last_commit: "".to_owned(),
            last_source_commits: HashMap::new()
        }
    }

    async fn create(&mut self, folder: &Path) -> anyhow::Result<()> {
        debug!("creating {}", self.repository);
        git::clone(&self.repository, folder).await?;

        self.upgrade(folder).await
    }

    async fn update_available(&self) -> anyhow::Result<bool> {
        // check pkgbuild repository
        debug!("updating {}", &self.repository);
        let current_commit = git::latest_commit(&self.repository).await?;
        if current_commit != self.last_commit { return Ok(true) }

        // check sources
        for (repo, commit) in &self.last_source_commits {
            debug!("updating source {}", repo);
            let latest = git::latest_commit(repo).await?;
            if &latest != commit { return Ok(true) }
        }

        Ok(false)
    }

    async fn upgrade(&mut self, folder: &Path) -> anyhow::Result<()> {
        // pull pkg repo
        debug!("upgrading {}", &self.repository);
        git::pull(folder).await?;
        self.last_commit = git::latest_commit(&self.repository).await?;

        // refresh sources
        self.last_source_commits = HashMap::new();
        let srcinfo: Srcinfo = tokio::fs::read_to_string(folder.join(SRCINFO)).await?.parse()?;

        for src in srcinfo.base.source.iter().flat_map(|s| &s.vec) { // TODO: only use required arch
            let mut split = src.split('+');

            if split.next() != Some("git") { continue } // skip non-git sources

            if let Some(repo) = split.next() {
                debug!("upgrading source {}", repo);
                let commit = git::latest_commit(repo).await?;
                self.last_source_commits.insert(repo.to_owned(), commit);
            }
        }

        Ok(())
    }
}