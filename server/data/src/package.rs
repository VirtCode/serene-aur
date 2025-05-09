use crate::build::{BuildInfo, BuildState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Serialize, Deserialize)]
pub struct PackageAddRequest {
    /// replace package of the same name
    pub replace: bool,
    /// resolve dependencies while adding
    pub resolve: bool,
    /// source of the package
    pub source: PackageAddSource,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PackageAddSource {
    Aur { name: String },
    Git { url: String, devel: bool },
    Raw { pkgbuild: String, devel: bool },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "key", content = "value", rename_all = "lowercase")]
pub enum PackageSettingsRequest {
    Clean(bool),
    Private(bool),
    Enabled(bool),
    Dependency(bool),
    Schedule(Option<String>),
    Prepare(Option<String>),
    Flags(Vec<MakepkgFlag>),
    Devel(bool),
    SrcinfoOverride(bool),
}

/// parameters for requesting package builds
#[derive(Serialize, Deserialize)]
pub struct PackageBuildRequest {
    /// packages to build
    pub packages: Vec<String>,
    /// perform a clean build
    pub clean: bool,
    /// resolve dependencies between packages when building
    pub resolve: bool,
    /// force rebuild
    pub force: bool,
}

impl PackageBuildRequest {
    /// create a build request for an all build
    pub fn all(clean: bool, resolve: bool, force: bool) -> Self {
        Self { packages: vec![], clean, resolve, force }
    }

    /// create a build request for a specific build
    pub fn specific(packages: Vec<String>, clean: bool, resolve: bool, force: bool) -> Self {
        Self { packages, clean, resolve, force }
    }
}

/// All supported makepkg flags which make sense to supply. Name the enum
/// entries just like the args (caseinsenitive). See `makepkg --help` for these
/// args
#[derive(Serialize, Deserialize, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MakepkgFlag {
    /// Ignore incomplete arch field in PKGBUILD
    IgnoreArch,
    /// Clean up work files after build
    Clean,
    /// Remove $srcdir/ dir before building the package
    CleanBuild,
    /// Skip all dependency checks
    NoDeps,
    /// Do not extract source files (use existing $srcdir/ dir)
    NoExtract,
    /// Install package after successful build
    Install,
    /// Remove installed dependencies after a successful build
    RmDeps,
    /// Repackage contents of the package without rebuilding
    Repackage,
    /// Do not update VCS sources
    HoldVer,
    /// Do not run the check() function in the PKGBUILD
    NoCheck,
    /// Do not run the prepare() function in the PKGBUILD
    NoPrepare,
    /// Do not verify checksums of the source files
    SkipChecksums,
    /// Do not perform any verification checks on source files
    SkipInteg,
    /// Do not verify source files with PGP signatures
    SkipPgpCheck,
}

#[derive(Serialize, Deserialize)]
pub struct PackagePeek {
    /// base of the package
    pub base: String,
    /// members of the package
    pub members: Vec<String>,
    /// current serving version
    pub version: Option<String>,

    /// is the package enabled
    pub enabled: bool,
    /// is the package a devel package
    pub devel: bool,

    /// latest build of the package
    pub build: Option<BuildInfo>,

    /// date added
    pub added: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct PackageInfo {
    /// base of the package
    pub base: String,
    /// members of the package
    pub members: Vec<String>,
    /// description of the package if present
    pub description: Option<String>,
    /// upstream url of the package if present
    pub upstream_url: Option<String>,

    /// total count of builds
    pub builds: u32,
    /// version of the package
    pub version: Option<String>,

    /// type of source of the package
    pub source: String,
    /// upstream url of the source
    pub source_url: Option<String>,
    /// is development package
    pub devel: bool,
    /// is the srcinfo forcibly generated by serene
    pub srcinfo_override: bool,

    /// is enabled
    pub enabled: bool,
    /// does clean-build
    pub clean: bool,
    /// is marked as private
    pub private: bool,
    /// is added as a dependency
    pub dependency: bool,
    /// schedule of the package
    pub schedule: String,
    /// schedule of the package was changed
    pub schedule_changed: bool,
    /// prepare commands ran before build
    pub prepare_commands: Option<String>,
    /// makepkg flags
    pub makepkg_flags: Vec<MakepkgFlag>,

    /// date added
    pub added: DateTime<Utc>,
}

/// All events which can be emitted by the broadcast for a package
#[derive(Serialize, Deserialize, Display, Clone)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastEvent {
    /// Change in the package build state
    Change(BuildState),
    /// Log message for the package build
    Log(String),
    /// Ping to the event subscriber
    Ping,
}
