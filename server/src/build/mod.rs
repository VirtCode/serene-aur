use crate::config::CONFIG;
use crate::database::{self, Database};
use crate::package::Package;
use crate::package::srcinfo::SrcinfoGeneratorInstance;
use crate::repository::PackageRepositoryInstance;
use crate::resolve::AurResolver;
use crate::runner::stats::CgroupStats;
use crate::runner::{ContainerId, RunStatus, RunnerInstance};
use crate::web::broadcast::BroadcastInstance;
use anyhow::{Context, anyhow};
use base64::Engine;
use chrono::{DateTime, Utc};
use futures::AsyncReadExt;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serene_data::build::BuildProgress::{Build, Clean, Publish, Update};
use serene_data::build::BuildState::{Failure, Fatal, Running, Success};
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;
use tokio_util::compat::TokioAsyncReadCompatExt;

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

    /// container cgroup stats of the build
    #[serde(flatten)]
    pub stats: Option<CgroupStats>,
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
            stats: None,
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
    ///
    /// returns a tuple containing the [`BuildSummary`] for the build of this
    /// package and an optional list of packages which will need to be rebuilt
    /// because of this build
    pub async fn run_build(
        &self,
        mut package: Package,
        update: bool,
        force_clean: bool,
        mut summary: BuildSummary,
    ) -> anyhow::Result<(BuildSummary, Option<Vec<Package>>)> {
        // list of packages which are required to rebuild because
        // this package was built
        let mut required_rebuilds = None;
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

            // force_clean is only used here as it is intended to remove the _old_ container
            let clean = package.clean || CONFIG.force_clean || force_clean;

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
                    Ok((build_stats, package_files)) => {
                        summary.stats = Some(build_stats);

                        let hash = compute_shared_objects_hash(&package_files);
                        if hash != package.shared_objects_hash {
                            // some versioned shared objects provided by the package changed;
                            // we need to rebuild all packages which depend on this package
                            package.change_shared_objects_hash(&self.db).await?;

                            let db = self.db.clone();
                            let (tx, rx) = oneshot::channel();

                            // we need to do the resolving in a separate OS thread because
                            // the underlying `Alpm` instance is not `Send`:
                            // See <https://github.com/archlinux/alpm.rs/issues/42>
                            std::thread::spawn(move || {
                                tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .expect("no build tokio runtime?")
                                    .block_on(async move {
                                        log::debug!("resolving packages on seperate thread");

                                        let result =
                                            match AurResolver::with_all(&db, &[], false).await {
                                                Ok(mut resolver) => resolver.resolve_all(&db).await,
                                                Err(err) => Err(err),
                                            };

                                        log::debug!("resolving on seperate thread finished");

                                        tx.send(result).unwrap_or_else(|_| {
                                            log::error!("failed to send resolving error")
                                        })
                                    })
                            });

                            let mut resolved = rx
                                .await
                                .context("failed to receive resolving info from thread")??;

                            resolved.retain(|(_, info)| info.depend.contains(&package.base));
                            required_rebuilds =
                                Some(resolved.into_iter().map(|(pkg, _)| pkg).collect::<Vec<_>>());
                        }
                    }
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
            if package.clean || CONFIG.force_clean {
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

            if success { Success } else { Failure }
        };

        summary.end(state);
        summary.change(&self.db).await?;
        self.broadcast.change(&package.base, summary.state.clone()).await;

        Ok((summary, required_rebuilds))
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
    ///
    /// returns a tuple containing the build stats and the list of files
    /// contained in the package
    async fn publish(
        &self,
        package: &mut Package,
        container: &ContainerId,
    ) -> anyhow::Result<(CgroupStats, Vec<PathBuf>)> {
        let mut output = self.runner.download_outputs(container).await?;

        let srcinfo = output.srcinfo().await?;
        package.upgrade(srcinfo).await?;

        let (stats_before, stats_after) = output.build_stats().await?;
        let build_stats = stats_after - stats_before;

        let archive_paths = self.repository.lock().await.publish(package, output).await?;
        let package_files = parse_mtrees(&archive_paths).await?;

        Ok((build_stats, package_files))
    }

    /// cleans a given container
    async fn clean(&self, container: &ContainerId) -> anyhow::Result<()> {
        self.runner.clean(container).await
    }
}

/// parse the .MTREE files of a built package and return the list of all files
/// which are installed by this package
async fn parse_mtrees(archive_paths: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for path in archive_paths {
        let file = tokio::fs::File::open(path).await.map(|file| file.compat())?;
        let archive = async_tar::Archive::new(file);

        let mut entries = archive.entries()?;
        #[expect(for_loops_over_fallibles)]
        for entry in entries.next().await {
            let mut entry = entry?;
            if entry.path()?.file_name().is_some_and(|name| name == ".MTREE") {
                let mut buffer = String::new();
                entry
                    .read_to_string(&mut buffer)
                    .await
                    .context("unable to read .MTREE from package archive")?;

                let mtree = match alpm_mtree::parser::mtree(&mut buffer.as_str()) {
                    Ok(mtree) => mtree,
                    Err(err) => {
                        Err(anyhow!(err.to_string()).context("unable to parse .MTREE file"))?
                    }
                };

                let paths_iter = mtree.iter().filter_map(|statement| match statement {
                    alpm_mtree::parser::Statement::Path { path, .. } => Some(path.clone()),
                    _ => None,
                });
                paths.extend(paths_iter);
            }
        }
    }

    Ok(paths)
}

/// compute the hash of all `libsomething.so.<number>` files of `package_files`
fn compute_shared_objects_hash(package_files: &[PathBuf]) -> Option<String> {
    let mut shared_object_files = package_files
        .iter()
        .filter_map(|path| {
            if !path.is_file() {
                return None;
            }

            let name = path.file_name()?.to_string_lossy();
            let mut parts = name.split('.');
            if name.starts_with("/usr/lib")
            // second-last element of path needs to be `.so`
            && parts.nth_back(1).is_some_and(|part| part == "so")
            // last element of path needs to be .<integer>
            && parts.nth_back(0).is_some_and(|part| part.parse::<u32>().is_ok())
            {
                Some(path.to_string_lossy())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if shared_object_files.is_empty() {
        return None;
    }

    // sort the versioned shared object paths in ascending order;
    // needs to be done such that hash is reproducible
    shared_object_files.sort();

    let mut hasher = Sha256::new();
    hasher.update(shared_object_files.join(","));
    Some(base64::prelude::BASE64_URL_SAFE.encode(hasher.finalize()))
}
