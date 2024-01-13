use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// reports the progress of a running build
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

/// reports the state of the current build
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

#[derive(Serialize, Deserialize)]
pub struct BuildInfo {
    /// state of the build
    pub state: BuildState,

    /// version that was built
    pub version: Option<String>,

    /// start time of the build
    pub started: DateTime<Utc>,
    /// end time of the build
    pub ended: Option<DateTime<Utc>>
}