use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

/// reports the progress of a running build
#[derive(Clone, Serialize, Deserialize, EnumString, Display, Copy)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BuildProgress {
    /// the build is resolving dependencies
    Resolve,
    /// the build is updating the sources
    Update,
    /// the build is building the package in the container
    Build,
    /// the build is publishing the built packages in the repository
    Publish,
    /// the build is cleaning the environment
    Clean,
}

/// reports the state of the current build
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "state", content = "info")]
pub enum BuildState {
    /// the build is pending, i.e. waiting for dependencies
    Pending,
    /// the build was not started because of issues found before the build
    Cancelled(String),
    /// the build is running
    Running(BuildProgress),
    /// the build succeeded
    Success,
    /// the build failed when building the package
    Failure,
    /// a fatal error occurred in a given step of the build
    Fatal(String, BuildProgress),
}

impl BuildState {
    pub fn done(&self) -> bool {
        match self {
            BuildState::Pending | BuildState::Running(_) => false,
            BuildState::Cancelled(_)
            | BuildState::Success
            | BuildState::Failure
            | BuildState::Fatal(_, _) => true,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BuildInfo {
    /// state of the build
    pub state: BuildState,

    /// reason why the build ran
    pub reason: BuildReason,

    /// version that was built
    pub version: Option<String>,

    /// start time of the build
    pub started: DateTime<Utc>,
    /// end time of the build
    pub ended: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, EnumString, Display, Clone, Copy)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BuildReason {
    /// build was triggered by a webhook
    Webhook,
    /// build was manually triggered by a user
    Manual,
    /// build was triggered in a schedule
    Schedule,
    /// initial build of package after addition
    Initial,
    /// build reason is not known
    ///
    /// only here for compatibility with older versions
    Unknown,
}
