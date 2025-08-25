use crate::database::{self, Database};
use crate::package::srcinfo::SrcinfoGeneratorInstance;
use crate::package::Package;
use crate::repository::PackageRepositoryInstance;
use crate::runner::{ContainerId, RunStatus, RunnerInstance};
use crate::web::broadcast::BroadcastInstance;
use chrono::{DateTime, Utc};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serene_data::build::BuildProgress::{Build, Clean, Publish, Update};
use serene_data::build::BuildState::{Failure, Fatal, Running, Success};
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use std::sync::Arc;

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
    pub details: Option<RunStatus>,
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
            details: None,
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

/// cleans up builds which are pending or working, but serene exited in the
/// meantime, or some beyond fatal error happened
pub async fn cleanup_unfinished(db: &Database) -> anyhow::Result<()> {
    info!("checking for unfinished builds");

    let active = BuildSummary::find_active(db).await?;

    for mut summary in active {
        warn!("cleaning build for {}, as it is still active", summary.package);

        summary.end(Fatal(
            "build was not finished or failed beyond fatally, then serene was restarted - check your logs!".to_owned(),
            if let Running(state) = &summary.state { *state } else { BuildProgress::Resolve }
        ));

        // we set the time to zero so we don't have stupidly long time durations
        summary.ended = Some(summary.started);

        summary.change(db).await?;
    }

    Ok(())
}

pub type BuilderInstance = Arc<Builder>;

pub struct Builder {
    db: Database,
    runner: RunnerInstance,
    broadcast: BroadcastInstance,
    repository: PackageRepositoryInstance,
    srcinfo_generator: SrcinfoGeneratorInstance,
}

impl Builder {
    /// creates a new builder
    pub fn new(
        db: Database,
        runner: RunnerInstance,
        repository: PackageRepositoryInstance,
        broadcast: BroadcastInstance,
        srcinfo_generator: SrcinfoGeneratorInstance,
    ) -> Self {
        Self { db, runner, repository, broadcast, srcinfo_generator }
    }

    /// Removes a package from the system, by removing the container, from the
    /// repo, and the database
    pub async fn run_remove(&self, package: &Package) -> anyhow::Result<()> {
        // remove container if exists
        self.runner.clean_build_container(package).await?;

        if let Err(e) = self.repository.lock().await.remove(package).await {
            warn!("removing package: {e:#}");
        }

        // remove logs from filesystem
        database::log::clean(package).await?;

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

            // BUILD
            summary.state = Running(Build);
            summary.change(&self.db).await?;
            self.broadcast.change(&package.base, summary.state.clone()).await;

            let clean = package.clean || force_clean; // also clean here if force clean
            let (container, success) = match self.build(&mut package, clean).await {
                Ok((status, logs, container)) => {
                    let next = status.success;
                    summary.details = Some(status);

                    // write logs to disk
                    database::log::write(&summary, logs).await?;

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
        package.update(&self.srcinfo_generator).await
    }

    /// builds a given package
    async fn build(
        &self,
        package: &mut Package,
        clean: bool,
    ) -> anyhow::Result<(RunStatus, String, ContainerId)> {
        let container = self.runner.prepare_build_container(package, clean).await?;

        self.runner.upload_inputs(&container, package.build_files().await?).await?;

        let (status, logs) = self.runner.run(&container, Some(package.base.clone())).await?;

        Ok((status, logs, container))
    }

    /// publishes a given package to the repository
    async fn publish(&self, package: &mut Package, container: &ContainerId) -> anyhow::Result<()> {
        let mut output = self.runner.download_outputs(&container).await?;

        let srcinfo = output.srcinfo().await?;
        package.upgrade(srcinfo).await?;

        self.repository.lock().await.publish(package, output).await
    }

    /// cleans a given container
    async fn clean(&self, container: &ContainerId) -> anyhow::Result<()> {
        self.runner.clean(container).await
    }
}
