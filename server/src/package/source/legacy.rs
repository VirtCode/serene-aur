use crate::package::source::aur::AurSource;
use crate::package::source::cli::CliSource;
use crate::package::source::git::GitSource;
use crate::package::source::raw::RawSource;
use crate::package::source::{Source, SourceImpl, SRCINFO};
use crate::package::srcinfo::SrcinfoWrapper;
use anyhow::Context;
use log::debug;
use serde::{Deserialize, Serialize};
use serene_data::secret;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use tokio::fs;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LegacySource {
    SereneCliSource {
        last_commit: String,
    },
    DevelGitSource {
        repository: String,
        last_commit: String,
        last_source_commits: HashMap<String, String>,
    },
    NormalSource {
        repository: String,
        last_commit: String,
    },
    SingleSource {
        pkgbuild: String,
        srcinfo: String,

        devel: bool,
        last_source_commits: HashMap<String, String>,
    },
}

impl LegacySource {
    pub fn get_state(&self) -> String {
        match self {
            LegacySource::SereneCliSource { last_commit } => last_commit.clone(),
            LegacySource::DevelGitSource { last_commit, last_source_commits, .. } => {
                let mut string = last_commit.clone();

                for commit in last_source_commits.values() {
                    string.push_str(commit);
                }

                string
            }
            LegacySource::NormalSource { last_commit, .. } => last_commit.clone(),
            LegacySource::SingleSource { pkgbuild, devel, last_source_commits, .. } => {
                // yes, this is technically for secrets
                let mut string = secret::hash(pkgbuild);

                if *devel {
                    for commit in last_source_commits.values() {
                        string.push_str(commit);
                    }
                }

                string
            }
        }
    }

    async fn migrate_git_source(
        folder: &Path,
        repository: String,
        last_commit: String,
        last_source_commits: Option<HashMap<String, String>>,
    ) -> anyhow::Result<Source> {
        // is it an aur package
        let inner: Box<dyn SourceImpl + Send + Sync> = if let Some(base) = repository
            .strip_prefix("https://aur.archlinux.org/")
            .and_then(|a| a.strip_suffix(".git"))
        {
            debug!("migrating an aur source, with base being {}", base);

            let srcinfo = fs::read_to_string(folder.join(SRCINFO))
                .await
                .context("failed to read .SRCINFO")
                .and_then(|s| SrcinfoWrapper::from_str(&s).context("failed to parse .SRCINFO"))?;

            Box::new(AurSource::migrated(base.to_owned(), srcinfo.version()))
        } else {
            debug!("migrating a git source");
            Box::new(GitSource::migrated(repository, last_commit))
        };

        Ok(Source::migrated(
            inner,
            last_source_commits.is_some(),
            last_source_commits.unwrap_or_default(),
        ))
    }

    pub async fn migrate(self, folder: &Path) -> anyhow::Result<Source> {
        match self {
            LegacySource::SereneCliSource { last_commit } => {
                debug!("migrating a cli source");
                Ok(Source::new(Box::new(CliSource::migrated(last_commit)), true))
            }
            LegacySource::DevelGitSource { repository, last_commit, last_source_commits } => {
                Self::migrate_git_source(folder, repository, last_commit, Some(last_source_commits))
                    .await
            }
            LegacySource::NormalSource { repository, last_commit } => {
                Self::migrate_git_source(folder, repository, last_commit, None).await
            }
            LegacySource::SingleSource { pkgbuild, devel, last_source_commits, .. } => {
                debug!("migrating a raw source");
                Ok(Source::migrated(
                    Box::new(RawSource::new(&pkgbuild)),
                    devel,
                    last_source_commits,
                ))
            }
        }
    }
}
