use base32::Alphabet::Crockford;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use chrono::{DateTime, Utc};
use colored::{ColoredString, Colorize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use serene_data::build::{BuildInfo, BuildState};

pub trait BuildStateFormatter {
    fn colored_passive(&self) -> ColoredString;
    fn colored_substantive(&self) -> ColoredString;
}

impl BuildStateFormatter for BuildState {
    fn colored_passive(&self) -> ColoredString {
        match self {
            BuildState::Running(_) => { "working".blue() }
            BuildState::Success => { "passing".green() }
            BuildState::Failure => { "failing".red() }
            BuildState::Fatal(_, _) => { "fatal".bright_red() }
        }
    }

    fn colored_substantive(&self) -> ColoredString {
        match self {
            BuildState::Running(_) => { "working".blue() }
            BuildState::Success => { "success".green() }
            BuildState::Failure => { "failure".red() }
            BuildState::Fatal(_, _) => { "fatal".bright_red() }
        }
    }
}

pub fn get_build_hash(summary: &BuildInfo) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{:#}", summary.started));

    base32::encode(Crockford, &hasher.finalize()).as_str()[0..3].to_owned()
}
