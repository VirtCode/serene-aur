use chrono::{DateTime, Utc};
use colored::{ColoredString, Colorize};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum BuildProgress {
    /// the build is updating the sources
    Update,
    /// the build is building the package in the container
    Build,
    /// the build is publishing the built packages in the repository
    Publish,
    /// the build is cleaning the environment
    Clean
}

#[derive(Clone, Serialize, Deserialize)]
pub enum BuildState {
    /// the build is running
    Running(BuildProgress),
    /// the build succeeded
    Success,
    /// the build failed when building the package
    Failure,
    /// a fatal error occurred in a given step of the build
    Fatal(String, BuildProgress)
}

impl BuildState {
    pub fn colored_passive(&self) -> ColoredString {
        match self {
            BuildState::Running(_) =>   { "working".blue() }
            BuildState::Success =>      { "passing".green() }
            BuildState::Failure =>      { "failing".red() }
            BuildState::Fatal(_, _) =>  { "fatal".bright_red() }
        }
    }

    pub fn colored_substantive(&self) -> ColoredString {
        match self {
            BuildState::Running(_) =>   { "working".blue() }
            BuildState::Success =>      { "success".green() }
            BuildState::Failure =>      { "failure".red() }
            BuildState::Fatal(_, _) =>  { "fatal".bright_red() }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PackagePeek {
    pub base: String,
    pub version: String,
    pub enabled: bool,
    pub devel: bool,
    pub build: Option<BuildPeek>
}

#[derive(Serialize, Deserialize)]
pub struct BuildPeek {
    pub state: BuildState,
    pub version: Option<String>,
    pub started: DateTime<Utc>,
    pub ended: Option<DateTime<Utc>>
}

#[derive(Serialize, Deserialize)]
pub struct PackageInfo {
    pub base: String,
    pub version: String,
    pub enabled: bool,
    pub devel: bool,
    pub clean: bool,
    pub schedule: String,
    pub added: DateTime<Utc>,
    pub builds: Vec<BuildPeek>
}