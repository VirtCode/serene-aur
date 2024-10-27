use crate::build::BuildSummary;
use crate::database::Database;
use crate::package::resolve::sync::create_and_sync;
use crate::package::Package;
use crate::web::broadcast::Broadcast;
use alpm::Alpm;
use anyhow::Context;
use aur_depends::{Flags, PkgbuildRepo, Resolver};
use log::{debug, warn};
use raur::ArcPackage;
use serene_data::build::{BuildProgress, BuildReason, BuildState};
use srcinfo::Srcinfo;
use std::collections::HashSet;
use std::sync::Arc;

pub struct BuildResolver<'a> {
    /// database
    db: &'a Database,
    broadcast: Arc<Broadcast>,

    /// handle on alpm for dependency resolving
    alpm: Alpm,

    /// handle on the aur rpc service
    aur: raur::Handle,
    /// aur cache so we don't flood the server with requests
    cache: HashSet<ArcPackage>,

    /// packages involved in this build round
    packages: Vec<(Package, BuildSummary)>,
}

/// information returned from the aur-resolve resolver
struct ResolveInfo {
    /// requirements missing from aur and repos
    missing: Vec<String>,
    /// packages required from the aur
    aur: HashSet<String>,
    /// packages depended on (already added)
    depend: HashSet<String>,
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
        Ok(Self {
            alpm: create_and_sync().await?,
            aur: raur::Handle::new(),
            cache: HashSet::new(),
            packages: Vec::new(),
            db,
            broadcast,
        })
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
        // build srcinfo repo
        let all = Package::find_all(self.db).await?;
        let mut added = vec![];

        for pkg in all {
            if self.packages.iter().any(|(o, _)| o.base == pkg.base) {
                match pkg.get_next_srcinfo().await {
                    Ok(srcinfo) => added.push(srcinfo.into()),
                    Err(e) => {
                        warn!("failed to read next srcinfo for package to be built: {e:#}");
                    }
                }
            } else if let Some(srcinfo) = pkg.srcinfo {
                added.push(srcinfo.into())
            }
        }

        // resolve packages
        debug!("starting to resolve all packages for build");
        let mut infos = Vec::new(); // can't use map cause async
        for x in self.packages.iter().map(|(p, _)| p.base.clone()).collect::<Vec<_>>() {
            infos.push(self.resolve_package(&x, &added).await?);
        }

        debug!("parsing resolve infos");
        let mut status = Vec::new();
        for info in infos.into_iter() {
            let result = if !info.missing.is_empty() {
                // totally missing deps

                Status::Failure(format!(
                    "could not resolve dependencies: {}",
                    info.missing.join(", ")
                ))
            } else if !info.aur.is_empty() {
                // missing deps from aur

                let mut result = Status::Failure(format!(
                    "missing dependencies from the AUR: {}",
                    info.aur.iter().cloned().collect::<Vec<_>>().join(", ")
                ));

                for pkg in &info.aur {
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

    /// resolve the dependencies for one package
    /// this method returning an error is serious, as it must be a network
    /// problem or something
    async fn resolve_package(
        &mut self,
        package: &str,
        repo: &Vec<Srcinfo>,
    ) -> anyhow::Result<ResolveInfo> {
        debug!("resolving dependencies of package {}", &package);

        let own = PkgbuildRepo { name: "serene", pkgs: repo.iter().collect() };

        let result = Resolver::new(&self.alpm, &mut self.cache, &self.aur, Flags::new()) // TODO: what can we change with these flags?
            .pkgbuild_repos(vec![own])
            .resolve_targets(&[package])
            .await
            .context("failed to resolve deps for package")?;

        Ok(ResolveInfo {
            aur: result.iter_aur_pkgs().map(|aur| aur.pkg.package_base.clone()).collect(),
            depend: result.iter_pkgbuilds().map(|(info, _)| info.base.pkgbase.clone()).collect(),
            missing: result.missing.into_iter().map(|m| m.dep).collect(),
        })
    }

    /// can be called after resolving failed fatally, such that begun builds are
    /// ended
    pub async fn finish_fatally(&mut self, message: &str) -> anyhow::Result<()> {
        for (_, summary) in &mut self.packages {
            summary.end(BuildState::Fatal(message.to_string(), BuildProgress::Resolve));
            summary.save(self.db).await?;
        }

        Ok(())
    }
}
