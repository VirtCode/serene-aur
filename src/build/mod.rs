use std::error::Error;
use std::sync::Arc;
use anyhow::Context;
use bollard::Docker;
use chrono::{DateTime, Utc};
use log::{error, info, LevelFilter};
use serde::{Deserialize, Serialize};
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use tokio::sync::RwLock;
use crate::build::BuildProgress::{Build, Clean, Publish, Update};
use crate::build::BuildState::{Failure, Fatal, Running, Success};
use crate::package::{Package};
use crate::package::store::PackageStore;
use crate::repository::PackageRepository;
use crate::runner::{archive, ContainerId, Runner, RunStatus};
use crate::runner::archive::{begin_read, read_version};

pub mod schedule;

#[derive(Clone, Serialize, Deserialize)]
pub enum BuildProgress {
    /// the build is updating the sources
    Update,
    /// the build is building the package in the container
    Build,
    /// the build is publishing the built packages in the repository
    Publish,
    /// the build is cleaning the environment
    Clean
}

#[derive(Clone, Serialize, Deserialize)]
pub enum BuildState {
    /// the build is running
    Running(BuildProgress),
    /// the build succeeded
    Success,
    /// the build failed when building the package
    Failure,
    /// a fatal error occurred in a given step of the build
    Fatal(String, BuildProgress)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BuildSummary {
    /// state of the build
    pub state: BuildState,

    /// logs / status obtained from the build container
    pub logs: Option<RunStatus>,
    /// version that was built
    pub version: Option<String>,

    /// start time of the build
    pub started: DateTime<Utc>,
    /// end time of the build
    pub ended: Option<DateTime<Utc>>
}

pub struct Builder {
    store: Arc<RwLock<PackageStore>>,
    runner: Runner,
    repository: PackageRepository,
}

impl Builder {

    /// creates a new builder
    pub fn new(store: Arc<RwLock<PackageStore>>, runner: Runner, repository: PackageRepository) -> Self {
        Self { store, runner, repository }
    }

    /// starts a build for a package, if there is no update, the build will be skipped (except when forced)
    pub async fn start(&mut self, package: &str, force: bool) {
        info!("starting build for package {package} now");

        let package =
            if let Some(p) = self.store.read().await.get(package) { p }
            else {
                error!("package scheduled for build is no longer in package store");
                return
            };

        let updatable = match package.updatable().await
            .context("failed to check for package updates on scheduled build") {
            Ok(u) => { u }
            Err(e) => { error!("{e:#}"); return }
        };

        if updatable || force {
            match self.run(package, updatable).await
                .context("build run for package failed extremely fatally"){
                Ok(_) => {}
                Err(e) => { error!("{e:#}") }
            };
        }
    }

    /// this runs a complete build of a package
    async fn run(&mut self, mut package: Package, update: bool) -> anyhow::Result<()> {
        let start = Utc::now();

        let mut summary = BuildSummary {
            state: Running(if update { Update } else { Build }),
            started: start.clone(),
            logs: None, version: None, ended: None,
        };

        package.update_build(summary.clone());
        self.store.write().await.update(package.clone()).await?;

        'run: {
            // UPDATE
            if update {
                match self.update(&mut package).await {
                    Ok(_) => {}
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Update);
                        break 'run;
                    }
                };

                summary.state = Running(Build);

                package.update_build(summary.clone());
                self.store.write().await.update(package.clone()).await?;
            }

            // BUILD
            let (container, success) = match self.build(&mut package).await {
                Ok((status, container)) => {
                    let next = status.success;
                    summary.logs = Some(status);
                    (container, next)
                }
                Err(e) => {
                    summary.state = Fatal(format!("{e:#}"), Build);
                    break 'run;
                }
            };
            summary.state = Running(if success { Publish } else { Clean });

            package.update_build(summary.clone());
            self.store.write().await.update(package.clone()).await?;

            // PUBLISH
            if success {
                match self.publish(&mut package, &container).await {
                    Ok(()) => { }
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Publish);
                        break 'run;
                    }
                }

                summary.version = Some(package.version.clone());
                summary.state = Running(Clean);

                package.update_build(summary.clone());
                self.store.write().await.update(package.clone()).await?;
            }

            // CLEAN
            if package.clean {
                match self.clean(&container).await {
                    Ok(()) => {}
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Clean);
                        break 'run;
                    }
                }
            }

            summary.state = if success {
                Success
            } else {
                Failure
            };
        };

        summary.ended = Some(Utc::now());

        package.update_build(summary);
        self.store.write().await.update(package).await?;

        Ok(())
    }

    /// updates the sources of a given package
    async fn update(&mut self, package: &mut Package) -> anyhow::Result<()> {
        package.upgrade_sources().await
    }

    /// builds a given package
    async fn build(&mut self, package: &mut Package) -> anyhow::Result<(RunStatus, ContainerId)> {
        let container = self.runner.prepare(package).await?;

        self.runner.upload_sources(&container, package).await?;

        let status = self.runner.build(&container).await?;

        Ok((status, container))
    }

    /// publishes a given package to the repository
    async fn publish(&mut self, package: &mut Package, container: &ContainerId) -> anyhow::Result<()> {
        let stream = self.runner.download_packages(&container).await?;
        let mut archive = begin_read(stream)?;

        let version = read_version(&mut archive).await?;
        package.upgrade_version(&version).await?;

        self.repository.publish(package, archive).await
    }

    /// cleans the container for a given package
    async fn clean(&mut self, container: &ContainerId) -> anyhow::Result<()> {
        self.runner.clean(container).await
    }
}