use crate::build::next::BuildResolver;
use crate::build::{BuildSummary, Builder};
use crate::config::CONFIG;
use crate::database::Database;
use crate::package::Package;
use anyhow::Result;
use log::{debug, error, info, warn};
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

pub struct BuildSession<'a> {
    packages: Vec<(Package, BuildSummary, HashSet<String>)>,
    building: HashSet<String>,

    builder: Arc<RwLock<Builder>>,
    db: &'a Database,
}

/// specifies whether a build was successful
struct BuildResult(String, bool);

impl<'a> BuildSession<'a> {
    /// starts a session by resolving the packages
    /// also updates all the sources as part of the resolving
    pub async fn start(
        packages: Vec<Package>,
        reason: BuildReason,
        db: &'a Database,
        builder: Arc<RwLock<Builder>>,
        resolve: bool,
    ) -> Result<Self> {
        let result = if resolve && CONFIG.resolve_build_sequence {
            // updates the sources too
            let mut resolver = BuildResolver::new(db).await?;

            match resolver.add_and_resolve(packages, reason).await {
                Ok(r) => r,
                Err(e) => {
                    if let Err(again) = resolver.finish_fatally(&e.to_string()).await {
                        warn!("failed to finish started resolver builds: {again}");
                    }

                    warn!("failed to resolve packages for build: {e}");
                    return Err(e);
                }
            }
        } else {
            let mut result = vec![];

            for package in packages {
                let summary = BuildSummary::start(&package, reason.clone());
                summary.save(db).await?;

                result.push((package, summary, HashSet::new()))
            }

            result
        };

        Ok(Self { builder, db, packages: result, building: HashSet::new() })
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
                }

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
                for (_, mut sum, _) in self.packages.extract_if(|(_, _, d)| d.contains(&built)) {
                    sum.end(BuildState::Cancelled(format!(
                        "failed to build dependency {built} successfully"
                    )));
                    sum.change(self.db).await?
                }
            }
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

        tokio::spawn(async move {
            let base = package.base.clone();

            let success = match builder.read().await.run_build(package, false, false, summary).await
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
