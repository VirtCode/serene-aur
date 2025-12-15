use crate::build::BuildSummary;
use crate::database::Database;
use crate::package::Package;
use crate::resolve::AurResolver;
use crate::web::broadcast::Broadcast;
use log::debug;
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use std::collections::HashSet;
use std::sync::Arc;

pub struct BuildResolver<'a> {
    /// database
    db: &'a Database,
    broadcast: Arc<Broadcast>,

    /// packages involved in this build round
    packages: Vec<(Package, BuildSummary)>,
}

/// internal enum used for tracking package status
enum Status {
    /// package has successfully resolved deps
    Success(HashSet<String>),
    /// failed to resolve all dependencies
    Failure(String),
}

impl<'a> BuildResolver<'a> {
    pub async fn new(db: &'a Database, broadcast: Arc<Broadcast>) -> anyhow::Result<Self> {
        Ok(Self { packages: Vec::new(), db, broadcast })
    }

    /// combines the lower two functions
    pub async fn add_and_resolve(
        &mut self,
        packages: Vec<Package>,
        reason: BuildReason,
    ) -> anyhow::Result<Vec<(Package, BuildSummary, HashSet<String>)>> {
        self.add(packages, reason).await?;
        self.resolve().await
    }

    /// add packages that will be built
    pub async fn add(&mut self, packages: Vec<Package>, reason: BuildReason) -> anyhow::Result<()> {
        for package in packages {
            debug!("adding package {} to resolver", &package.base);

            // create build
            let summary = BuildSummary::start(&package, reason);
            summary.save(self.db).await?;

            // add them
            self.packages.push((package, summary));
        }

        Ok(())
    }

    /// resolves the added packages
    pub async fn resolve(
        &mut self,
    ) -> anyhow::Result<Vec<(Package, BuildSummary, HashSet<String>)>> {
        let mut resolver =
            AurResolver::next(self.db, self.packages.iter().map(|(p, _)| p), false).await?;

        // resolve packages
        debug!("starting to resolve all packages for build");
        let mut infos = Vec::new(); // can't use map cause async
        for x in self.packages.iter().map(|(p, _)| p.base.clone()).collect::<Vec<_>>() {
            infos.push(resolver.resolve_package(&x).await?);
        }

        debug!("parsing resolve infos");
        let mut status = Vec::new();
        for info in infos.into_iter() {
            debug_assert!(info.aur.is_empty()); // we use the stub resolver

            let result = if !info.missing.is_empty() {
                let mut result =
                    Status::Failure(format!("missing dependencies: {}", info.missing.join(", ")));

                for pkg in &info.missing {
                    if Package::has(pkg, self.db).await? {
                        result = Status::Failure(format!(
                            "dependency {pkg} is added but has never built successfully"
                        ));
                    }
                }

                result
            } else {
                // all good

                Status::Success(info.depend)
            };

            status.push(result)
        }

        let mut failed = status
            .iter()
            .zip(&self.packages)
            .filter(|(status, _)| matches!(status, Status::Failure(_)))
            .map(|(_, (p, _))| p.base.clone())
            .collect::<HashSet<_>>();

        loop {
            debug!("starting cleaning round for cancelled packages");
            let mut removed = false;

            for (status, (package, _)) in status.iter_mut().zip(&self.packages) {
                let missing = {
                    let Status::Success(set) = status else {
                        continue;
                    };

                    set.intersection(&failed).cloned().collect::<Vec<_>>()
                };

                if !missing.is_empty() {
                    debug!("package {} depends on cancelled packages", package.base);
                    *status = Status::Failure(format!(
                        "dependencies are added but have been cancelled: {}",
                        missing.join(", ")
                    ));

                    failed.insert(package.base.clone());
                    removed = true;
                }
            }

            if !removed {
                break;
            }
        }

        let succeeded = status
            .iter()
            .zip(&self.packages)
            .filter(|(status, _)| matches!(status, Status::Success(_)))
            .map(|(_, (p, _))| p.base.clone())
            .collect::<HashSet<_>>();
        let mut result = vec![];

        for ((package, mut summary), status) in self.packages.drain(0..).zip(status) {
            match status {
                Status::Success(set) => {
                    // remove itself and non-built deps
                    let mut deps = set.intersection(&succeeded).cloned().collect::<HashSet<_>>();
                    deps.remove(&package.base);

                    result.push((package, summary, deps));
                }
                Status::Failure(err) => {
                    debug!("cancelling package {} because: {err}", package.base);
                    summary.end(BuildState::Cancelled(err));
                    summary.change(self.db).await?;
                    self.broadcast.change(&package.base, summary.state.clone()).await;
                }
            }
        }

        Ok(result)
    }

    /// can be called after resolving failed fatally, such that begun builds are
    /// ended
    pub async fn finish_fatally(&mut self, message: &str) -> anyhow::Result<()> {
        for (_, summary) in &mut self.packages {
            summary.end(BuildState::Fatal(message.to_string(), BuildProgress::Resolve));
            summary.change(self.db).await?;
        }

        Ok(())
    }
}
