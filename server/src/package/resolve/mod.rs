use super::{source::SrcinfoWrapper, Package};
use crate::database::Database;
use crate::package::resolve::sync::{initialize_alpm, synchronize_alpm};
use alpm::Alpm;
use anyhow::{anyhow, Context, Result};
use aur_depends::{Flags, PkgbuildRepo, Resolver};
use either::{Either, Left, Right};
use log::warn;
use raur::{ArcPackage, Handle};
use srcinfo::Srcinfo;
use std::collections::HashMap;
use std::{collections::HashSet, ops::Deref};

pub mod sync;

pub struct AurResolver {
    alpm: Alpm,

    added: Vec<Srcinfo>,

    aur: Handle,
    aur_cache: HashSet<ArcPackage>,
}

impl AurResolver {
    /// starts the resolver by initializing all required data
    /// an instance should not be kept around for long, and should only be used
    /// for one "occcasion" for packages in future, to-be-built srcinfo will
    /// be used for resolving
    pub async fn start(db: &Database, future: &Vec<Package>) -> Result<Self> {
        let added = Package::find_all(db)
            .await?
            .into_iter()
            .filter_map(|p| p.srcinfo.map(|s| s.into()))
            .collect();

        Ok(Self {
            alpm: initialize_alpm()?,
            added,
            aur: raur::Handle::new(),
            aur_cache: HashSet::new(),
        })
    }

    /// creates a resolver
    /// takes a vec of packages for which the to-be-built srcinfo is used
    /// instead
    pub fn create_resolver(&mut self) -> Resolver {
        let own = PkgbuildRepo { name: "serene", pkgs: self.added.iter().collect() };

        Resolver::new(&self.alpm, &mut self.aur_cache, &self.aur, Flags::new())
            .pkgbuild_repos(vec![own])
    }

    /// resolve the given srcinfo
    pub async fn resolve_add(&mut self, srcinfo: &SrcinfoWrapper) -> Result<HashSet<ArcPackage>> {
        let resolver = self.create_resolver();

        // resolve deps
        let deps: HashSet<String> = srcinfo
            .pkgs
            .iter()
            .filter_map(|p| p.depends.first().map(|p| p.vec.clone()))
            .flatten()
            .collect();

        let make = srcinfo
            .base
            .makedepends
            .first()
            .map(|p| p.vec.iter().collect::<Vec<_>>())
            .unwrap_or_else(|| vec![]);

        let result = resolver
            .resolve_depends(&deps.iter().collect::<Vec<_>>().as_slice(), make.as_slice())
            .await
            .context("failure during dependency resolving")?;

        if !result.missing.is_empty() {
            return Err(anyhow!(
                "failed to find all dependencies for {}, missing are {}",
                srcinfo.base.pkgbase,
                result.missing.iter().map(|a| a.dep.clone()).collect::<Vec<_>>().join(", ")
            ));
        }

        Ok(result.iter_aur_pkgs().map(|p| p.pkg.clone()).collect())
    }

    /// sync integrated alpm dbs
    pub async fn sync(&mut self) -> Result<()> {
        synchronize_alpm(&mut self.alpm)
    }
}
