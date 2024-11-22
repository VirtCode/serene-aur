use crate::build::schedule::BuildMeta;
use crate::build::{BuildSummary, Builder};
use crate::config::CONFIG;
use crate::database::Database;
use crate::package::Package;
use crate::resolve::build::BuildResolver;
use crate::web::broadcast::Broadcast;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::mpsc::Sender;
use tokio::sync::{oneshot, RwLock};
use tokio::task::LocalSet;

pub struct BuildSession<'a> {
    packages: Vec<(Package, BuildSummary, HashSet<String>)>,
    building: HashSet<String>,
    meta: BuildMeta,

    builder: Arc<RwLock<Builder>>,
    broadcast: Arc<Broadcast>,
    db: &'a Database,
}

/// specifies whether a build was successful
struct BuildResult(String, bool);

impl<'a> BuildSession<'a> {
    /// starts a session by resolving the packages.
    /// packages should be updated outside, as they might not build successfully
    /// anyway
    pub async fn start(
        packages: Vec<Package>,
        db: &'a Database,
        builder: Arc<RwLock<Builder>>,
        broadcast: Arc<Broadcast>,
        meta: BuildMeta,
    ) -> Result<Self> {
        let result = if meta.resolve && CONFIG.resolve_build_sequence {
            Self::resolve(packages, meta.reason, db, broadcast.clone()).await?
        } else {
            let mut result = vec![];

            for package in packages {
                let summary = BuildSummary::start(&package, meta.reason);
                summary.save(db).await?;
                broadcast.change(&package.base, summary.state.clone()).await;

                result.push((package, summary, HashSet::new()))
            }

            result
        };

        Ok(Self { builder, broadcast, db, packages: result, building: HashSet::new(), meta })
    }

    // FIXME: remove this once Alpm is sync, see sync.rs
    /// resolves the packages in a seperate thread
    /// yes, this is pretty ugly, but async is anyways
    /// (cause alpm is non-send)
    async fn resolve(
        packages: Vec<Package>,
        reason: BuildReason,
        db: &'a Database,
        broadcast: Arc<Broadcast>,
    ) -> Result<Vec<(Package, BuildSummary, HashSet<String>)>> {
        let (tx, rx) = oneshot::channel();

        let db = db.clone();
        let broadcast = broadcast.clone();

        // spawn new thread
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("no build tokio runtime?")
                .block_on(async move {
                    debug!("resolving packages on seperate thread");

                    let result = 'content: {
                        let mut resolver = match BuildResolver::new(&db, broadcast).await {
                            Ok(r) => r,
                            Err(e) => break 'content Err(e),
                        };

                        match resolver.add_and_resolve(packages, reason).await {
                            Ok(r) => Ok(r),
                            Err(e) => {
                                if let Err(again) = resolver.finish_fatally(&e.to_string()).await {
                                    warn!("failed to finish started resolver builds: {again}");
                                }

                                warn!("failed to resolve packages for build: {e}");
                                Err(e)
                            }
                        }
                    };

                    debug!("resolving on seperate thread finished");

                    tx.send(result).unwrap_or_else(|_| error!("failed to send resolving error"));
                })
        });

        rx.await.context("failed to receive resolving info from thread")?
    }

    /// builds all packages in the optimal sequence
    pub async fn run(&mut self) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        loop {
            // run ready packages
            let buildable = self.packages.extract_if(|(_, _, d)| d.is_empty()).collect::<Vec<_>>();
            for (package, summary, _) in buildable {
                self.build_package(package, summary, tx.clone()).await?;
            }

            // check if empty
            if self.building.is_empty() {
                info!("finished building successfully");

                break;
            }

            // wait for next
            let Some(BuildResult(built, success)) = rx.recv().await else {
                warn!("didn't catch previous ending condition!");
                break;
            };

            info!("received build result for package {built} with status {success}");
            self.building.remove(&built);

            // updating waiting packages
            if success || CONFIG.resolve_ignore_failed {
                for (_, _, deps) in &mut self.packages {
                    deps.remove(&built);
                }
            } else {
                for (pkg, mut sum, _) in self.packages.extract_if(|(_, _, d)| d.contains(&built)) {
                    sum.end(BuildState::Cancelled(format!(
                        "failed to build dependency {built} successfully"
                    )));
                    sum.change(self.db).await?;
                    self.broadcast.change(&pkg.base, sum.state.clone()).await;
                }
            }
        }

        for (p, summary, rem) in &mut self.packages {
            warn!("orphaned package {} found during build", p.base);

            summary.end(BuildState::Fatal(
                format!(
                    "package was orphaned in the build process, waiting for {}",
                    rem.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
                BuildProgress::Resolve,
            ));
            summary.change(self.db).await?;
            self.broadcast.change(&p.base, summary.state.clone()).await;
        }

        Ok(())
    }

    /// builds a package in a separate thread / routine
    async fn build_package(
        &mut self,
        package: Package,
        summary: BuildSummary,
        tx: Sender<BuildResult>,
    ) -> Result<()> {
        info!("starting build for package {}", package.base);

        self.building.insert(package.base.clone());
        let builder = self.builder.clone();
        let clean = self.meta.clean;

        tokio::spawn(async move {
            let base = package.base.clone();

            let success = match builder.read().await.run_build(package, false, clean, summary).await
            {
                Ok(summary) => {
                    matches!(summary.state, BuildState::Success)
                }
                Err(e) => {
                    warn!("build failed beyond fatally: {e:#}");

                    false
                }
            };

            if let Err(e) = tx.send(BuildResult(base, success)).await {
                error!("failed to send result back to main thread: {e}");
            }
        });

        Ok(())
    }
}
