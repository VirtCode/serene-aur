use crate::build::schedule::BuildScheduler;
use crate::config::{CLI_PACKAGE_NAME, CONFIG};
use crate::database::Database;
use crate::package::source::cli::SereneCliSource;
use crate::package::source::devel::DevelGitSource;
use crate::package::source::normal::NormalSource;
use crate::package::source::{Source, SrcinfoWrapper};
use crate::runner;
use crate::runner::archive;
use anyhow::{anyhow, Context, Error};
use chrono::{DateTime, Utc};
use hyper::Body;
use log::{debug, info, warn};
use resolve::sync::initialize_alpm;
use resolve::AurResolver;
use serene_data::build::BuildReason;
use serene_data::package::MakepkgFlag;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use time::macros::offset;
use tokio::fs;

pub mod aur;
pub mod git;
pub mod resolve;
pub mod source;

const SOURCE_FOLDER: &str = "sources";

const PACKAGE_EXTENSION: &str = ".pkg.tar.zst"; // see /etc/makepkg.conf

pub async fn add_source(
    db: &Database,
    source: Box<dyn Source + Sync + Send>,
    replace: bool,
) -> anyhow::Result<Option<Vec<Package>>> {
    let temp = get_temp();

    let result = add(db, source, &temp, replace).await;

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
    source: &mut Box<dyn Source + Sync + Send>,
    temp: &Path,
) -> anyhow::Result<(PathBuf, SrcinfoWrapper)> {
    let folder = get_temp_package(temp);
    fs::create_dir_all(&folder).await?;

    source.create(&folder).await.context("failed to checkout source")?;

    let srcinfo = source.get_srcinfo(&folder).await?;

    Ok((folder, srcinfo))
}

async fn add(
    db: &Database,
    mut source: Box<dyn Source + Sync + Send>,
    temp: &Path,
    replace: bool,
) -> anyhow::Result<Option<Vec<Package>>> {
    // checkout target
    let (path, srcinfo) = checkout(&mut source, temp).await?;
    let target = srcinfo.base.pkgbase.clone();
    info!("adding new package {target}");

    if Package::find(&srcinfo.base.pkgbase, db).await?.is_some() && !replace {
        return Ok(None);
    }

    // resolve deps - this already resolves transitive deps (iirc)
    let mut resolver = AurResolver::start(db, &vec![]).await?;

    let needed = resolver.resolve_add(&srcinfo).await?;

    // checkout other packages
    let mut packages = vec![(path, srcinfo, source, replace)];

    for dep in needed {
        let mut source: Box<dyn Source + Sync + Send> = if aur::is_devel(&dep) {
            Box::new(DevelGitSource::empty(&aur::to_git(&dep)))
        } else {
            Box::new(NormalSource::empty(&aur::to_git(&dep)))
        };

        let (path, srcinfo) = checkout(&mut source, temp)
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
pub async fn try_add_cli(db: &Database, scheduler: &mut BuildScheduler) -> anyhow::Result<()> {
    if Package::has(CLI_PACKAGE_NAME, db).await? {
        return Ok(());
    }

    info!("adding and building serene-cli");
    if let Some(all) = add_source(db, Box::new(SereneCliSource::new()), false).await? {
        let Some(mut package) = all.into_iter().next() else {
            return Err(anyhow!("failed to add serene-cli, not in added pkgs"));
        };

        package.clean = true;
        package.change_settings(db).await?;

        scheduler.schedule(&package).await?;
        scheduler.run(&package, true, BuildReason::Initial).await?;

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
    pub source: Box<dyn Source + Sync + Send>,

    /// pkgbuild string used for the currently passing build for user pleasure
    pub pkgbuild: Option<String>,
    /// srcinfo of the current build, reported from the package for devel
    /// packages
    pub srcinfo: Option<SrcinfoWrapper>,
    /// DEPRECATED: version of the current build of the package
    pub version: Option<String>,

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
    fn new(
        srcinfo: SrcinfoWrapper,
        source: Box<dyn Source + Sync + Send>,
        dependency: bool,
    ) -> Self {
        Self {
            base: srcinfo.base.pkgbase.clone(),
            added: Utc::now(),

            dependency,
            clean: !source.is_devel(),
            enabled: true,
            schedule: None,
            prepare: None,
            flags: vec![],

            version: None,
            srcinfo: None,
            pkgbuild: None,

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
                if self.source.is_devel() {
                    &CONFIG.schedule_devel
                } else {
                    &CONFIG.schedule_default
                }
            })
            .clone()
    }

    pub async fn updatable(&self) -> anyhow::Result<bool> {
        self.source.update_available().await
    }

    pub async fn update(&mut self) -> anyhow::Result<()> {
        self.source.update(&self.get_folder()).await
    }

    /// upgrades the version of the package
    /// returns an error if a version mismatch is detected with the source files
    pub async fn upgrade(&mut self, reported: SrcinfoWrapper) -> anyhow::Result<()> {
        let mut srcinfo = self.source.get_srcinfo(&self.get_folder()).await?;
        let pkgbuild = self.source.get_pkgbuild(&self.get_folder()).await?;

        if self.source.is_devel() {
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

        self.version = Some(srcinfo.base.pkgver.clone());
        self.srcinfo = Some(srcinfo);
        self.pkgbuild = Some(pkgbuild);

        Ok(())
    }

    /// returns the next srcinfo that will be built
    pub async fn get_next_srcinfo(&self) -> anyhow::Result<SrcinfoWrapper> {
        self.source.get_srcinfo(&self.get_folder()).await
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
        let arch = select_arch(&srcinfo.pkg.arch);

        Ok(srcinfo
            .names()
            .map(|s| format!("{s}-{epoch}{version}-{rel}-{arch}{PACKAGE_EXTENSION}"))
            .collect())
    }

    pub async fn build_files(&self) -> anyhow::Result<Body> {
        let mut archive = archive::begin_write();

        // upload sources
        self.source.load_build_files(&self.get_folder(), &mut archive).await?;

        // upload repository file
        archive::write_file(runner::repository_file(), "custom-repo", false, &mut archive).await?;

        // upload prepare script
        archive::write_file(
            self.prepare.clone().unwrap_or_default(),
            "serene-prepare.sh",
            false,
            &mut archive,
        )
        .await?;

        // upload makepkg flags
        archive::write_file(
            self.flags.iter().map(|f| format!("--{f} ")).collect::<String>(),
            "makepkg-flags",
            false,
            &mut archive,
        )
        .await?;

        archive::end_write(archive).await
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
