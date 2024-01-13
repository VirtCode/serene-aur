use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::build::BuildInfo;

#[derive(Serialize, Deserialize)]
pub struct PackagePeek {
    /// base of the package
    pub base: String,
    /// current serving version
    pub version: String,

    /// is the package enabled
    pub enabled: bool,
    /// is the package a devel package
    pub devel: bool,

    /// latest build of the package
    pub build: Option<BuildInfo>
}

#[derive(Serialize, Deserialize)]
pub struct PackageInfo {
    /// base of the package
    pub base: String,
    /// members of the package
    pub members: Vec<String>,

    /// version of the package
    pub version: String,
    /// is development package
    pub devel: bool,

    /// is enabled
    pub enabled: bool,
    /// does clean-build
    pub clean: bool,
    /// schedule of the package
    pub schedule: String,

    /// date added
    pub added: DateTime<Utc>,

    /// all build info
    pub builds: Vec<BuildInfo>
}