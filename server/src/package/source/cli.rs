use crate::config::{self, CONFIG, INFO};
use crate::package::git;
use crate::package::source::{Source, SourceImpl, SrcinfoWrapper, PKGBUILD};
use crate::runner::archive::InputArchive;
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CLI_PKGBUILD: &str = include_str!("../../../../cli/PKGBUILD");
const CLI_PKGBUILD_REPLACE: &str = "#-serene-cli-source-do-not-remove";

/// this is a custom source for the serene cli
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliSource {
    /// depending on the mode, this is either the tag or latest commit
    /// we track the latest commit here, because the source should not be devel
    #[serde(default)]
    state: String,
}

impl CliSource {
    pub fn new() -> Self {
        Self { state: "".to_owned() }
    }

    pub fn migrated(last_commit: String) -> Self {
        Self { state: last_commit }
    }
}

#[typetag::serde]
#[async_trait]
impl SourceImpl for CliSource {
    async fn initialize(&mut self, _folder: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        Some(config::SOURCE_REPOSITORY.to_string())
    }

    fn get_type(&self) -> String {
        "internal cli".to_string()
    }

    fn get_state(&self) -> String {
        self.state.clone()
    }

    async fn update(&mut self, _folder: &Path) -> anyhow::Result<()> {
        if CONFIG.edge_cli {
            debug!("updating edge cli source");

            // check upstream for changes
            self.state = git::find_remote_commit(config::SOURCE_REPOSITORY).await?;
        } else {
            debug!("ensuring normal cli source");

            // use version that is currently running
            self.state = INFO.version.clone();
        }

        Ok(())
    }

    async fn get_pkgbuild(&self, _folder: &Path) -> anyhow::Result<String> {
        if CONFIG.edge_cli {
            Ok(CLI_PKGBUILD.to_string())
        } else {
            let mut pkgbuild = CLI_PKGBUILD.to_string();

            let tag = INFO.version.as_str();
            let version = tag.strip_prefix("v").unwrap_or(tag);

            // override source and version
            pkgbuild = pkgbuild.replace(
                CLI_PKGBUILD_REPLACE,
                &format!(
                    r#"
                    source=("git+https://github.com/VirtCode/serene-aur.git#tag={tag}")
                    pkgver={version}
                    pkgdesc="$pkgdesc (server tagged version)"
                    "#
                ),
            );

            // remove pkgver function because it doesn't change
            pkgbuild = pkgbuild.replace("pkgver()", "_pkgver()");

            Ok(pkgbuild)
        }
    }

    async fn get_srcinfo(&self, _folder: &Path) -> anyhow::Result<Option<SrcinfoWrapper>> {
        Ok(None)
    }

    async fn load_build_files(
        &self,
        archive: &mut InputArchive,
        folder: &Path,
    ) -> anyhow::Result<()> {
        archive.write_file(&self.get_pkgbuild(folder).await?, Path::new(PKGBUILD), true).await
    }
}

/// create a new cli souce
pub fn new() -> Source {
    Source::new(Box::new(CliSource::new()), false)
}
