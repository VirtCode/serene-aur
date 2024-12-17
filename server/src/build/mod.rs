use crate::database::Database;
use crate::package::Package;
use crate::repository::PackageRepository;
use crate::runner::archive::{begin_read, read_srcinfo};
use crate::runner::{ContainerId, RunStatus, Runner};
use crate::web::broadcast::Broadcast;
use chrono::{DateTime, Utc};
use log::warn;
use serde::{Deserialize, Serialize};
use serene_data::build::BuildProgress::{Build, Clean, Publish, Update};
use serene_data::build::BuildState::{Failure, Fatal, Running, Success};
use serene_data::build::{BuildReason, BuildState};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod schedule;
pub mod session;

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

impl BuildSummary {
    pub fn start(package: &Package, reason: BuildReason) -> Self {
        Self {
            package: package.base.clone(),
            state: BuildState::Pending,
            logs: None,
            version: None,
            started: Utc::now(),
            ended: None,
            reason,
        }
    }

    pub fn end(&mut self, state: BuildState) {
        self.state = state;
        self.ended = Some(Utc::now());
    }
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
    /// if this function returns an error, the issue is with the database
    pub async fn run_build(
        &self,
        mut package: Package,
        update: bool,
        force_clean: bool,
        mut summary: BuildSummary,
    ) -> anyhow::Result<BuildSummary> {
        let state = 'run: {
            // UPDATE
            if update {
                summary.state = Running(Update);
                summary.change(&self.db).await?;
                self.broadcast.change(&package.base, summary.state.clone()).await;

                match self.update(&mut package).await {
                    Ok(_) => {}
                    Err(e) => {
                        break 'run Fatal(format!("{e:#}"), Update);
                    }
                };
            }

            // CLEAN (when changed to clean or force clean)
            if package.clean || force_clean {
                summary.state = Running(Clean);
                summary.change(&self.db).await?;
                self.broadcast.change(&package.base, summary.state.clone()).await;

                match self.try_clean(&package).await {
                    Ok(()) => {}
                    Err(e) => {
                        break 'run Fatal(format!("{e:#}"), Clean);
                    }
                }
            }

            // BUILD
            summary.state = Running(Build);
            summary.change(&self.db).await?;
            self.broadcast.change(&package.base, summary.state.clone()).await;

            let (container, success) = match self.build(&mut package).await {
                Ok((status, container)) => {
                    let next = status.success;
                    summary.logs = Some(status);
                    (container, next)
                }
                Err(e) => {
                    break 'run Fatal(format!("{e:#}"), Build);
                }
            };

            // PUBLISH
            if success {
                summary.state = Running(Publish);
                summary.change(&self.db).await?;
                self.broadcast.change(&package.base, summary.state.clone()).await;

                match self.publish(&mut package, &container).await {
                    Ok(()) => {}
                    Err(e) => {
                        break 'run Fatal(format!("{e:#}"), Publish);
                    }
                }

                summary.version = package.get_version();
                summary.state = Running(Clean);

                summary.change(&self.db).await?;
                self.broadcast.change(&package.base, summary.state.clone()).await;

                // change sources here as the new package was successfully published
                package.change_sources(&self.db).await?;
            }

            // CLEAN
            if package.clean {
                summary.state = Running(Publish);
                summary.change(&self.db).await?;
                self.broadcast.change(&package.base, summary.state.clone()).await;

                match self.clean(&container).await {
                    Ok(()) => {}
                    Err(e) => {
                        break 'run Fatal(format!("{e:#}"), Clean);
                    }
                }
            }

            if success {
                Success
            } else {
                Failure
            }
        };

        summary.end(state);
        summary.change(&self.db).await?;
        self.broadcast.change(&package.base, summary.state.clone()).await;

        Ok(summary)
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
