use crate::build::BuildInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Serialize, Deserialize)]
pub struct PackageAddRequest {
    pub replace: bool,
    pub source: PackageAddSource,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PackageAddSource {
    Aur { name: String },
    Custom { url: String, devel: bool },
    Single { pkgbuild: String, devel: bool },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "key", content = "value", rename_all = "lowercase")]
pub enum PackageSettingsRequest {
    Clean(bool),
    Enabled(bool),
    Schedule(String),
    Prepare(String),
    Flags(Vec<MakepkgFlag>),
}

#[derive(Serialize, Deserialize)]
pub struct PackageBuildRequest {
    pub clean: bool,
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

    /// version of the package
    pub version: Option<String>,
    /// is development package
    pub devel: bool,

    /// is enabled
    pub enabled: bool,
    /// does clean-build
    pub clean: bool,
    /// schedule of the package
    pub schedule: String,
    /// prepare commands ran before build
    pub prepare_commands: Option<String>,
    /// makepkg flags
    pub makepkg_flags: Vec<MakepkgFlag>,

    /// date added
    pub added: DateTime<Utc>,
}

/// All events which can be emitted by the broadcast for a package
#[derive(Serialize, Deserialize, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BroadcastEvent {
    /// A build job for the package was started
    BuildStart,
    /// A build job for the package finished
    BuildEnd,
    /// Log message for the package build
    Log,
    /// Ping to the event subscriber
    Ping,
}
