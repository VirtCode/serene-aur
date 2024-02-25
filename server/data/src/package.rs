use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::build::BuildInfo;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PackageAddRequest {
    Aur { name: String },
    Custom { url: String, devel: bool }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "key", content = "value", rename_all = "lowercase")]
pub enum PackageSettingsRequest {
    Clean(bool),
    Enabled(bool),
    Schedule(String),
    Prepare(String)
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

    /// date added
    pub added: DateTime<Utc>,
}