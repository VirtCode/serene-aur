use crate::package::srcinfo::SrcinfoWrapper;
use crate::package::{aur, Package};
use crate::resolve::sync::create_and_sync;
use crate::{database::Database, resolve::stub::StubAur};
use alpm::Alpm;
use anyhow::Context;
use aur_depends::{Actions, Flags, PkgbuildRepo, Resolver};
use log::{debug, warn};
use srcinfo::Srcinfo;
use std::collections::HashSet;

pub mod build;
pub mod stub;
pub mod sync;

/// information returned from the aur-resolve resolver
pub struct ResolveInfo {
    /// requirements missing from aur and repos
    pub missing: Vec<String>,
    /// packages required from the aur
    pub aur: HashSet<String>,
    /// packages depended on (already added)
    pub depend: HashSet<String>,
}

pub struct AurResolver {
    repos: Alpm,

    aur: Option<raur::Handle>,
    aur_cache: raur::Cache,

    local: Vec<Srcinfo>,
}

impl AurResolver {
    /// create a new resolver with packages in their current states, unless they
    /// are contained in the iterator next
    pub async fn next<'a, T>(db: &Database, next: T, aur: bool) -> anyhow::Result<Self>
    where
        T: Iterator<Item = &'a Package>,
    {
        let all = Package::find_all(db).await?;
        let mut added = vec![];

        let next = next.map(|p| p.base.clone()).collect::<Vec<_>>();

        for pkg in all {
            if next.contains(&pkg.base) {
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

        Self::new(added, aur).await
    }

    /// create a new resolver with an additional added package
    pub async fn with(db: &Database, srcinfo: &SrcinfoWrapper, aur: bool) -> anyhow::Result<Self> {
        let mut all: Vec<Srcinfo> = Package::find_all(db)
            .await?
            .into_iter()
            .filter_map(|p| p.srcinfo.map(|s| s.into()))
            .collect();

        all.push(srcinfo.clone().into());

        Self::new(all, aur).await
    }

    /// create a new resolver with a given local repo
    async fn new(local: Vec<Srcinfo>, aur: bool) -> anyhow::Result<Self> {
        Ok(Self {
            repos: create_and_sync().await?,
            aur: if aur { Some(aur::handle()?) } else { None },
            aur_cache: HashSet::new(),
            local,
        })
    }

    /// resolves a package, but returns the raw results
    /// see Self::resolve_package
    pub async fn resolve_package_raw(&mut self, package: &str) -> anyhow::Result<Actions<'_>> {
        debug!("resolving dependencies of package {}", &package);

        let own = PkgbuildRepo { name: "serene", pkgs: self.local.iter().collect() };

        let result = if let Some(aur) = &self.aur {
            Resolver::new(&self.repos, &mut self.aur_cache, aur, Flags::new()) // TODO: what can we change with these flags?
                .pkgbuild_repos(vec![own])
                .resolve_targets(&[package])
                .await
        } else {
            Resolver::new(&self.repos, &mut self.aur_cache, &StubAur, Flags::new())
                .pkgbuild_repos(vec![own])
                .resolve_targets(&[package])
                .await
        };

        let mut actions = result.context("failed to resolve deps for package")?;

        // we have to remove the package itself base from missing, as it is listed under
        // missing on split packages which don't contain a member of the same name
        actions.missing.retain(|missing| missing.dep != package);

        Ok(actions)
    }

    /// resolve the dependencies for one package
    /// this method returning an error is serious, as it must be a network
    /// problem or something
    /// this function takes a mutable reference, because of the cache
    pub async fn resolve_package(&mut self, package: &str) -> anyhow::Result<ResolveInfo> {
        let result = self.resolve_package_raw(package).await?;

        Ok(ResolveInfo {
            aur: result.iter_aur_pkgs().map(|aur| aur.pkg.package_base.clone()).collect(),
            depend: result.iter_pkgbuilds().map(|(info, _)| info.base.pkgbase.clone()).collect(),
            missing: result.missing.into_iter().map(|m| m.dep).collect(),
        })
    }
}
