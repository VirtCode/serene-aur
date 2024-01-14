use colored::{ColoredString, Colorize};
use serene_data::build::{BuildInfo, BuildProgress, BuildState};

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

pub trait BuildProgressFormatter {
    fn printable_string(&self) -> String;
}

impl BuildProgressFormatter for BuildProgress {
    fn printable_string(&self) -> String {
        match self {
            BuildProgress::Update => { "updating sources" }
            BuildProgress::Build => { "building package" }
            BuildProgress::Publish => { "publishing repository" }
            BuildProgress::Clean => { "cleaning up" }
        }.to_string()
    }
}

pub fn get_build_id(summary: &BuildInfo) -> String {
    let string = format!("{:x}", summary.started.timestamp());
    string[(string.len() - 4)..string.len()].to_owned()
}
