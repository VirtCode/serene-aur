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
use crate::package::{Package, PackageManager};
use crate::package::store::PackageStore;
use crate::repository::PackageRepository;
use crate::runner::{archive, ContainerId, Runner, RunStatus};
use crate::runner::archive::{begin_read, read_version};

pub mod schedule;

#[derive(Clone, Serialize, Deserialize)]
pub enum BuildProgress {
    Update,
    Build,
    Publish,
    Clean
}

#[derive(Clone, Serialize, Deserialize)]
pub enum BuildState {
    Running(BuildProgress),
    Success,
    Failure,
    Fatal(String, BuildProgress)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BuildSummary {
    pub state: BuildState,

    pub logs: Option<RunStatus>,
    pub version: Option<String>,

    pub started: DateTime<Utc>,
    pub ended: Option<DateTime<Utc>>
}

pub struct Builder {
    store: Arc<RwLock<PackageStore>>,
    runner: Runner,
    repository: PackageRepository,
}

impl Builder {

    pub fn new(store: Arc<RwLock<PackageStore>>, runner: Runner, repository: PackageRepository) -> Self {
        Self { store, runner, repository }
    }

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
            match self.clean(&container).await {
                Ok(()) => {}
                Err(e) => {
                    summary.state = Fatal(format!("{e:#}"), Clean);
                    break 'run;
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

    // updates the sources of a given package
    async fn update(&mut self, package: &mut Package) -> anyhow::Result<()> {
        package.upgrade_sources().await
    }

    // builds a given package
    async fn build(&mut self, package: &mut Package) -> anyhow::Result<(RunStatus, ContainerId)> {
        let container = self.runner.prepare(package).await?;

        self.runner.upload_sources(&container, package).await?;

        let status = self.runner.build(&container).await?;

        Ok((status, container))
    }

    // publishes a given package to the repository
    async fn publish(&mut self, package: &mut Package, container: &ContainerId) -> anyhow::Result<()> {
        let stream = self.runner.download_packages(&container).await?;
        let mut archive = begin_read(stream)?;

        let version = read_version(&mut archive).await?;
        package.upgrade_version(&version).await?;

        self.repository.publish(package, archive).await
    }

    // cleans the container for a given package
    async fn clean(&mut self, container: &ContainerId) -> anyhow::Result<()> {
        self.runner.clean(container).await
    }

}