use std::{collections::HashSet, ops::Deref};

use alpm::Alpm;
use anyhow::{anyhow, Context, Result};
use aur_depends::{Flags, PkgbuildRepo, Resolver};
use raur::{ArcPackage, Handle};

use crate::database::Database;

use super::{source::SrcinfoWrapper, Package};

pub mod sync;

pub struct AurResolver<'a> {
    alpm: &'a Alpm,

    added: Vec<Package>,

    aur: Handle,
    aur_cache: HashSet<ArcPackage>,
}

impl<'a> AurResolver<'a> {

    /// starts the resolver by initializing all required data
    /// an instance should not be kept around for long, and should only be used for one "request"
    pub async fn start(alpm: &'a Alpm, db: &Database) -> Result<Self> {
        Ok(Self {
            alpm,
            added: Package::find_all(db).await?,
            aur: raur::Handle::new(),
            aur_cache: HashSet::new()
        })
    }

    /// resolve the given srcinfo
    pub async fn resolve(&mut self, srcinfo: &SrcinfoWrapper) -> Result<HashSet<ArcPackage>> {
        let own = PkgbuildRepo {
            name: "serene",
            pkgs: self.added.iter()
                .filter_map(|p| p.srcinfo.as_ref().map(|s| s.deref()))
                .collect()
        };

        let resolver = Resolver::new(self.alpm, &mut self.aur_cache, &self.aur, Flags::new())
            .pkgbuild_repos(vec!(own));


        // resolve deps
        let deps: HashSet<String> = srcinfo.pkgs.iter()
            .filter_map(|p|  p.depends.first().map(|p| p.vec.clone()))
            .flatten()
            .collect();

        let make = srcinfo.base.makedepends.first().map(|p| p.vec.iter().collect::<Vec<_>>()).unwrap_or_else(|| vec!());

        let result = resolver.resolve_depends(&deps.iter().collect::<Vec<_>>().as_slice(), make.as_slice()).await
            .context("failure during dependency resolving")?;

        if !result.missing.is_empty() {
            return Err(anyhow!("failed to find all dependencies for {}, missing are {}",
                srcinfo.base.pkgbase,
                result.missing.iter().map(|a| a.dep.clone()).collect::<Vec<_>>().join(", ")
            ));
        }

        Ok(result.iter_aur_pkgs().map(|p| p.pkg.clone()).collect())
    }
}
