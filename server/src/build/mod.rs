use crate::database::Database;
use crate::package::Package;
use crate::repository::PackageRepository;
use crate::runner::archive::{begin_read, read_srcinfo};
use crate::runner::{ContainerId, RunStatus, Runner};
use crate::web::broadcast::{Broadcast, Event};
use anyhow::Context;
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serene_data::build::BuildProgress::{Build, Clean, Publish, Update};
use serene_data::build::BuildState::{Failure, Fatal, Running, Success};
use serene_data::build::{BuildReason, BuildState};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod schedule;

#[derive(Clone, Serialize, Deserialize)]
pub struct BuildSummary {
    /// package the summary belongs to
    pub package: String,
    /// state of the build
    pub state: BuildState,
    /// reason why the build ran
    pub reason: BuildReason,

    /// logs / status obtained from the build container
    pub logs: Option<RunStatus>,
    /// version that was built
    pub version: Option<String>,

    /// start time of the build
    pub started: DateTime<Utc>,
    /// end time of the build
    pub ended: Option<DateTime<Utc>>,
}

pub struct Builder {
    db: Database,
    runner: Arc<RwLock<Runner>>,
    broadcast: Arc<Broadcast>,
    repository: Arc<RwLock<PackageRepository>>,
}

impl Builder {
    /// creates a new builder
    pub fn new(
        db: Database,
        runner: Arc<RwLock<Runner>>,
        repository: Arc<RwLock<PackageRepository>>,
        broadcast: Arc<Broadcast>,
    ) -> Self {
        Self { db, runner, repository, broadcast }
    }

    /// starts a build for a package, if there is no update, the build will be
    /// skipped (except when forced)
    pub async fn run_scheduled(&self, package: &str, force: bool, clean: bool, reason: BuildReason) {
        info!("starting build for package {package} now");

        let package = match Package::find(package, &self.db).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                warn!("package scheduled for build is no longer in package store");
                return;
            }
            Err(e) => {
                error!("failed to read package from database: {e:#}");
                return;
            }
        };

        let updatable = match package
            .updatable()
            .await
            .context("failed to check for package updates on scheduled build")
        {
            Ok(u) => u,
            Err(e) => {
                error!("{e:#}");
                return;
            }
        };

        if updatable || force {
            match self
                .run_build(package, updatable, clean, reason)
                .await
                .context("build run for package failed extremely fatally")
            {
                Ok(_) => {}
                Err(e) => {
                    error!("{e:#}")
                }
            };
        }
    }

    /// Removes a package from the system, by removing the container, from the
    /// repo, and the database
    pub async fn run_remove(&self, package: &Package) -> anyhow::Result<()> {
        // remove container if exists
        if let Some(container) = self.runner.read().await.find_container(package).await? {
            self.clean(&container).await?;
        }

        if let Err(e) = self.repository.write().await.remove(package).await {
            warn!("removing package: {e:#}");
        }

        package.self_destruct().await?;
        package.delete(&self.db).await?;

        Ok(())
    }

    /// this runs a complete build of a package
    pub async fn run_build(
        &self,
        mut package: Package,
        update: bool,
        force_clean: bool,
        reason: BuildReason,
    ) -> anyhow::Result<()> {
        let start = Utc::now();

        let mut summary = BuildSummary {
            package: package.base.clone(),
            reason,
            state: Running(Build),
            started: start.clone(),
            logs: None,
            version: None,
            ended: None,
        };
        summary.save(&self.db).await?;

        self.broadcast.notify(&package.base, Event::BuildStart).await;

        'run: {
            // UPDATE
            if update {
                // state is already correct

                match self.update(&mut package).await {
                    Ok(_) => {}
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Update);
                        break 'run;
                    }
                };
            }

            // CLEAN (when changed to clean or force clean)
            if package.clean || force_clean {
                summary.state = Running(Clean);
                summary.change(&self.db).await?;

                match self.try_clean(&package).await {
                    Ok(()) => {}
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Clean);
                        break 'run;
                    }
                }
            }

            // BUILD
            summary.state = Running(Build);
            summary.change(&self.db).await?;

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

            // PUBLISH
            if success {
                summary.state = Running(Publish);
                summary.change(&self.db).await?;

                match self.publish(&mut package, &container).await {
                    Ok(()) => {}
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Publish);
                        break 'run;
                    }
                }

                summary.version = package.version.clone();
                summary.state = Running(Clean);

                summary.change(&self.db).await?;

                // change sources here as the new package was successfully published
                package.change_sources(&self.db).await?;
            }

            // CLEAN
            if package.clean {
                summary.state = Running(Publish);
                summary.change(&self.db).await?;

                match self.clean(&container).await {
                    Ok(()) => {}
                    Err(e) => {
                        summary.state = Fatal(format!("{e:#}"), Clean);
                        break 'run;
                    }
                }
            }

            summary.state = if success { Success } else { Failure };
        };

        summary.ended = Some(Utc::now());
        summary.change(&self.db).await?;

        self.broadcast.notify(&package.base, Event::BuildFinish).await;

        Ok(())
    }

    /// updates the sources of a given package
    async fn update(&self, package: &mut Package) -> anyhow::Result<()> {
        package.update().await
    }

    /// builds a given package
    async fn build(&self, package: &mut Package) -> anyhow::Result<(RunStatus, ContainerId)> {
        let container = self.runner.read().await.prepare(package).await?;

        self.runner.read().await.upload_sources(&container, package).await?;

        let status = self.runner.read().await.build(&container, package).await?;

        Ok((status, container))
    }

    /// publishes a given package to the repository
    async fn publish(&self, package: &mut Package, container: &ContainerId) -> anyhow::Result<()> {
        let stream = self.runner.read().await.download_packages(&container).await?;
        let mut archive = begin_read(stream)?;

        let version = read_srcinfo(&mut archive).await?;
        package.upgrade(version).await?;

        self.repository.write().await.publish(package, archive).await
    }

    /// cleans a given container
    async fn clean(&self, container: &ContainerId) -> anyhow::Result<()> {
        self.runner.read().await.clean(container).await
    }

    /// cleans the container for a given package if available
    async fn try_clean(&self, package: &Package) -> anyhow::Result<()> {
        if let Some(container) = self.runner.read().await.find_container(package).await? {
            self.clean(&container).await?;
        }

        Ok(())
    }
}
