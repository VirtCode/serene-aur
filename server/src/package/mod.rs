use crate::build::schedule::{BuildMeta, BuildScheduler};
use crate::build::BuildSummary;
use crate::config::{CLI_PACKAGE_NAME, CONFIG};
use crate::database::Database;
use crate::package::source::Source;
use crate::package::srcinfo::{SrcinfoGeneratorInstance, SrcinfoWrapper};
use crate::resolve::AurResolver;
use crate::runner;
use crate::runner::archive;
use crate::runner::archive::InputArchive;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use hyper::Body;
use log::{debug, info, warn};
use serene_data::build::{BuildReason, BuildState};
use serene_data::package::MakepkgFlag;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

pub mod aur;
pub mod git;
pub mod source;
pub mod srcinfo;

pub const SOURCE_FOLDER: &str = "sources";

pub(crate) const PACKAGE_EXTENSION: &str = ".pkg.tar.zst"; // see /etc/makepkg.conf

pub async fn add_source(
    db: &Database,
    srcinfo_generator: &SrcinfoGeneratorInstance,
    source: Source,
    replace: bool,
) -> anyhow::Result<Option<Vec<Package>>> {
    let temp = get_temp();

    let result = add(db, srcinfo_generator, source, &temp, replace).await;

    if let Err(e) = fs::remove_dir_all(&temp).await {
        warn!("failed to remove temp for checkout: {e:#}");
    }

    result
}

/// get transaction name for temporary folders
fn get_temp() -> PathBuf {
    Path::new(SOURCE_FOLDER)
        .join("tmp")
        .join(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string())
}

/// get temporary folder to check out source
fn get_temp_package(temp: &Path) -> PathBuf {
    temp.join(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string())
}

async fn checkout(
    source: &mut Source,
    temp: &Path,
    srcinfo_generator: &SrcinfoGeneratorInstance,
) -> anyhow::Result<(PathBuf, SrcinfoWrapper)> {
    let folder = get_temp_package(temp);
    fs::create_dir_all(&folder).await?;

    source.initialize(srcinfo_generator, &folder).await.context("failed to checkout source")?;

    let srcinfo = source.get_srcinfo(&folder).await?;

    Ok((folder, srcinfo))
}

async fn add(
    db: &Database,
    srcinfo_generator: &SrcinfoGeneratorInstance,
    mut source: Source,
    temp: &Path,
    replace: bool,
) -> anyhow::Result<Option<Vec<Package>>> {
    // checkout target
    let (path, srcinfo) = checkout(&mut source, temp, srcinfo_generator).await?;
    let target = srcinfo.base.pkgbase.clone();
    info!("adding new package {target}");

    if Package::find(&srcinfo.base.pkgbase, db).await?.is_some() && !replace {
        return Ok(None);
    }

    // resolve deps - this already resolves transitive deps
    let mut resolver = AurResolver::with(db, &srcinfo).await?;
    let actions = resolver.resolve_package_raw(&srcinfo.base.pkgbase).await?;

    if !actions.missing.is_empty() {
        return Err(anyhow!(
            "failed to satisfy all dependencies for {}, missing are {}",
            srcinfo.base.pkgbase,
            actions.missing.iter().map(|a| a.dep.clone()).collect::<Vec<_>>().join(", ")
        ));
    }

    // checkout other packages
    let mut packages = vec![(path, srcinfo, source, replace)];

    for dep in actions.iter_aur_pkgs().map(|p| &p.pkg) {
        let mut source = source::aur::new(&dep, false);

        let (path, srcinfo) = checkout(&mut source, temp, srcinfo_generator)
            .await
            .context("failed to checkout source for dependency")?;

        info!("adding new dependency {}", srcinfo.base.pkgbase);

        packages.push((path, srcinfo, source, false));
    }

    // finish up packages
    let mut result = vec![];

    for (path, srcinfo, source, replace) in packages {
        // check other packages
        let (package, new) =
            if let Some(mut package) = Package::find(&srcinfo.base.pkgbase, db).await? {
                // only proceed if replacing enabled
                if !replace {
                    warn!("aur-resolve suggested package that was already added: {}", package.base);
                    continue;
                }

                package.source = source;
                (package, false)
            } else {
                let dependency = srcinfo.base.pkgbase != target;

                (Package::new(srcinfo, source, dependency), true)
            };

        // move package
        if package.get_folder().exists() {
            fs::remove_dir_all(package.get_folder())
                .await
                .context("failed to remove previous source")?;
        }

        fs::rename(path, package.get_folder()).await.context("failed to move source")?;

        if new {
            package.save(db).await?
        } else {
            package.change_sources(db).await?
        }

        info!("successfully added package {}", &package.base);
        result.push(package);
    }

    Ok(Some(result))
}

/// adds the cli to the current packages
pub async fn try_add_cli(
    db: &Database,
    scheduler: &mut BuildScheduler,
    srcinfo_generator: &SrcinfoGeneratorInstance,
) -> anyhow::Result<()> {
    if Package::has(CLI_PACKAGE_NAME, db).await? {
        return Ok(());
    }

    info!("adding and building serene-cli");
    if let Some(all) = add_source(db, srcinfo_generator, source::cli::new(), false).await? {
        // TODO: cleanify with support for deps
        let Some(mut package) = all.into_iter().next() else {
            return Err(anyhow!("failed to add serene-cli, not in added pkgs"));
        };

        package.clean = true;
        package.change_settings(db).await?;

        scheduler.schedule(&package).await?;

        let packages = vec![package];
        scheduler.run(packages, BuildMeta::normal(BuildReason::Initial)).await?;

        info!("successfully added serene-cli");
    }

    Ok(())
}

/// this struct represents a package built by serene
#[derive(Clone)]
pub struct Package {
    /// base of the package
    pub base: String,
    /// time when the package was added
    pub added: DateTime<Utc>,

    /// source of the package
    pub source: Source,

    /// pkgbuild string used for the currently passing build for user pleasure
    pub pkgbuild: Option<String>,
    /// srcinfo of the current build, reported from the package for devel
    /// packages
    pub srcinfo: Option<SrcinfoWrapper>,
    /// state of the source that is built, can be used to check if the source
    /// has new stuff
    pub built_state: String,

    /// whether package is enabled, meaning it is built automatically
    pub enabled: bool,
    /// whether the package was added as a dependency
    pub dependency: bool,
    /// whether package should be cleaned after building
    pub clean: bool,
    /// potential custom cron schedule string
    pub schedule: Option<String>,
    /// commands to run in container before package build, they are written to
    /// the shell
    pub prepare: Option<String>,
    /// special makepkg flags
    pub flags: Vec<MakepkgFlag>,
}

impl Package {
    /// creates a new package with default values
    fn new(srcinfo: SrcinfoWrapper, source: Source, dependency: bool) -> Self {
        Self {
            base: srcinfo.base.pkgbase.clone(),
            added: Utc::now(),

            dependency,
            clean: !source.devel,
            enabled: true,
            schedule: None,
            prepare: None,
            flags: vec![],

            srcinfo: None,
            pkgbuild: None,
            built_state: "init".to_owned(),

            source,
        }
    }

    /// gets the current folder for the source for the package
    fn get_folder(&self) -> PathBuf {
        Path::new(SOURCE_FOLDER).join(&self.base)
    }

    /// gets the schedule string for the package
    pub fn get_schedule(&self) -> String {
        self.schedule
            .as_ref()
            .unwrap_or_else(|| {
                if self.source.devel {
                    &CONFIG.schedule_devel
                } else {
                    &CONFIG.schedule_default
                }
            })
            .clone()
    }

    /// is the newest version of the package already built and in the repos
    pub fn newest_built(&self) -> bool {
        self.built_state == self.source.get_state()
    }

    pub async fn update(
        &mut self,
        srcinfo_generator: &SrcinfoGeneratorInstance,
    ) -> anyhow::Result<()> {
        self.source.update(srcinfo_generator, &self.get_folder()).await
    }

    /// upgrades the version of the package
    /// returns an error if a version mismatch is detected with the source files
    pub async fn upgrade(&mut self, reported: SrcinfoWrapper) -> anyhow::Result<()> {
        let mut srcinfo = self.source.get_srcinfo(&self.get_folder()).await?;
        let pkgbuild = self.source.get_pkgbuild(&self.get_folder()).await?;
        let state = self.source.get_state();

        if self.source.devel {
            // upgrade devel package srcinfo to reflect version and rel
            srcinfo = reported;
        } else if srcinfo.base.pkgver != reported.base.pkgver {
            // check for version mismatch for non-devel packages
            return Err(anyhow!(
                "version mismatch on package {}, expected {} but built {}",
                &self.base,
                &srcinfo.base.pkgver,
                &reported.base.pkgver
            ));
        }

        self.srcinfo = Some(srcinfo);
        self.pkgbuild = Some(pkgbuild);
        self.built_state = state;

        Ok(())
    }

    /// returns the next srcinfo that will be built
    pub async fn get_next_srcinfo(&self) -> anyhow::Result<SrcinfoWrapper> {
        self.source.get_srcinfo(&self.get_folder()).await
    }

    /// returns the currently built version of the package
    pub fn get_version(&self) -> Option<String> {
        self.srcinfo.as_ref().map(|s| s.base.pkgver.clone())
    }

    /// returns the expected built files
    /// requires the version to be upgraded
    pub async fn expected_files(&self) -> anyhow::Result<Vec<String>> {
        let srcinfo = self.srcinfo.as_ref().ok_or(anyhow!(
            "no srcinfo loaded, upgrade version first. this is an internal error, please report"
        ))?;

        let rel = &srcinfo.base.pkgrel;
        let version = &srcinfo.base.pkgver;
        let epoch = srcinfo
            .base
            .epoch
            .as_ref()
            .map(|s| format!("{}:", s))
            .unwrap_or_else(|| "".to_string());

        Ok(srcinfo
            .pkgs
            .iter()
            .map(|pkg| {
                let arch = select_arch(&pkg.arch);

                format!("{}-{epoch}{version}-{rel}-{arch}{PACKAGE_EXTENSION}", pkg.pkgname)
            })
            .collect())
    }

    pub async fn build_files(&self) -> anyhow::Result<InputArchive> {
        let mut archive = InputArchive::new();

        // upload sources
        self.source.load_build_files(&self.get_folder(), &mut archive).await?;

        // upload repository file
        archive.write_file(&runner::repository_file(), Path::new("custom-repo"), false).await?;

        // upload prepare script
        archive
            .write_file(
                &self.prepare.clone().unwrap_or_default(),
                Path::new("serene-prepare.sh"),
                false,
            )
            .await?;

        // upload makepkg flags
        archive
            .write_file(
                &self.flags.iter().map(|f| format!("--{f} ")).collect::<String>(),
                Path::new("makepkg-flags"),
                false,
            )
            .await?;

        Ok(archive)
    }

    /// removes the source files of the source
    pub async fn self_destruct(&self) -> anyhow::Result<()> {
        fs::remove_dir_all(self.get_folder()).await.context("could not delete source directory")
    }

    pub fn get_packages(&self) -> Vec<String> {
        self.srcinfo
            .as_ref()
            .map(|s| s.names().map(|s| s.to_owned()).collect())
            .unwrap_or_else(|| vec![])
    }
}

/// selects the built architecture from a list of architectures
fn select_arch(available: &Vec<String>) -> String {
    // system can only build either itself or any
    if available.iter().any(|s| s == &CONFIG.architecture) {
        CONFIG.architecture.to_owned()
    } else {
        "any".to_string()
    }
}

/// performs heuristics to migrate packages to the new built_state
/// will check whether the latest build was a success and if so will assume the
/// source has not changed
pub async fn migrate_build_state(db: &Database) -> anyhow::Result<()> {
    for mut package in Package::find_migrated_built_state(db).await? {
        debug!("trying to migrate package {} to built_state", package.base);
        let Some(summary) = BuildSummary::find_latest_for_package(&package.base, db).await? else {
            continue;
        };

        if let BuildState::Success = summary.state {
            info!("migrating package {} to built_state, assuming up-to-date", package.base);

            package.built_state = package.source.get_state();
            package.change_sources(db).await?;
        }
    }
    Ok(())
}
