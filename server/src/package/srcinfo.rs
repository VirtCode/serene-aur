use crate::runner::archive::InputArchive;
use crate::runner::RunnerInstance;
use anyhow::anyhow;
use log::debug;
use srcinfo::Srcinfo;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// wraps a srcinfo together with its source so we can convert to and from the
/// src
#[derive(Clone)]
pub struct SrcinfoWrapper {
    source: String,
    inner: Srcinfo,
}

impl FromStr for SrcinfoWrapper {
    type Err = srcinfo::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { source: s.to_owned(), inner: s.parse()? })
    }
}

impl ToString for SrcinfoWrapper {
    fn to_string(&self) -> String {
        self.source.clone()
    }
}

impl Deref for SrcinfoWrapper {
    type Target = Srcinfo;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Into<Srcinfo> for SrcinfoWrapper {
    fn into(self) -> Srcinfo {
        self.inner
    }
}

// we wrap this in a mutex so we don't get any race conditions of different
// packages trying to generate their srcinfo at the same time
pub type SrcinfoGeneratorInstance = Arc<Mutex<SrcinfoGenerator>>;

pub struct SrcinfoGenerator {
    runner: RunnerInstance,
}

impl SrcinfoGenerator {
    /// create a srcinfo generator
    pub fn new(runner: RunnerInstance) -> Self {
        Self { runner }
    }

    /// generates the .SRCINFO for a given PKGBUILD
    pub async fn generate_srcinfo(&self, pkgbuild: &str) -> anyhow::Result<SrcinfoWrapper> {
        debug!("starting srcinfo generation for pkgbuild");

        let mut input = InputArchive::new();
        input.write_file(pkgbuild, Path::new("PKGBUILD"), true).await?;

        let container = self.runner.prepare_srcinfo_container(true).await?;

        self.runner.upload_inputs(&container, input).await?;
        let status = self.runner.run(&container, None).await?;

        debug!("srcinfo generation finished with status {}", status.success);

        if status.success {
            let mut output = self.runner.download_outputs(&container).await?;
            output.srcinfo().await
        } else {
            Err(anyhow!("srcinfo generation container failed: {}", status.logs))
        }
    }
}
