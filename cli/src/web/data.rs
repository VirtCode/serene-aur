use anyhow::anyhow;
use chrono::{Local, Offset};
use colored::{ColoredString, Colorize};
use cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron_options;
use cron_descriptor::cronparser::Options;
use serene_data::build::{BuildInfo, BuildProgress, BuildReason, BuildState};
use std::str::FromStr;

pub trait BuildStateFormatter {
    fn colored_passive(&self) -> ColoredString;
    fn colored_substantive(&self) -> ColoredString;
}

impl BuildStateFormatter for BuildState {
    fn colored_passive(&self) -> ColoredString {
        match self {
            BuildState::Pending => "pending".dimmed(),
            BuildState::Cancelled(_) => "cancelled".bright_yellow(),
            BuildState::Running(_) => "working".blue(),
            BuildState::Success => "passing".green(),
            BuildState::Failure => "failing".red(),
            BuildState::Fatal(_, _) => "fatal".bright_red(),
        }
    }

    fn colored_substantive(&self) -> ColoredString {
        match self {
            BuildState::Pending => "pending".dimmed(),
            BuildState::Cancelled(_) => "cancelled".bright_yellow(),
            BuildState::Running(_) => "working".blue(),
            BuildState::Success => "success".green(),
            BuildState::Failure => "failure".red(),
            BuildState::Fatal(_, _) => "fatal".bright_red(),
        }
    }
}

pub trait BuildReasonFormatter {
    fn colored(&self) -> ColoredString;
}

impl BuildReasonFormatter for BuildReason {
    fn colored(&self) -> ColoredString {
        match self {
            BuildReason::Webhook => "webhook".bright_blue(),
            BuildReason::Manual => "manual".bright_blue(),
            BuildReason::Schedule => "schedule".dimmed(),
            BuildReason::Initial => "initial".bright_blue(),
            BuildReason::Unknown => "unknown".dimmed(),
        }
    }
}

pub trait BuildProgressFormatter {
    fn printable_string(&self) -> String;
}

impl BuildProgressFormatter for BuildProgress {
    fn printable_string(&self) -> String {
        match self {
            BuildProgress::Resolve => "resolving dependencies",
            BuildProgress::Update => "updating sources",
            BuildProgress::Build => "building package",
            BuildProgress::Publish => "publishing repository",
            BuildProgress::Clean => "cleaning up",
        }
        .to_string()
    }
}

pub fn get_build_id(summary: &BuildInfo) -> String {
    let string = format!("{:x}", summary.started.timestamp());
    string[(string.len() - 4)..string.len()].to_owned()
}

/// this converts a cron string from utc to local time
/// note that this is a very hacky implementation and does not work in all cases
pub fn describe_cron_timezone_hack(schedule: &str) -> anyhow::Result<String> {
    let mut parts = schedule.split(' ').map(|s| s.to_owned()).collect::<Vec<_>>();

    if parts.len() < 5 {
        return Err(anyhow!("invalid cron string provided"));
    }
    let index = parts.len() - 4; // fourth entry from the left

    // hack is only possible if it is comma separated or a single number
    let possible = parts[index].chars().all(|c| "0123456789,".contains(c));

    if possible {
        let offset = Local::now().offset().fix().local_minus_utc() / (60 * 60);

        parts[index] = parts[index]
            .split(',')
            .map(|s| {
                i32::from_str(s)
                    .map(|i| (i + offset % 24).to_string())
                    .unwrap_or_else(|_| s.to_string())
            })
            .intersperse(",".to_string())
            .collect();
    }

    get_description_cron_options(
        &parts.into_iter().intersperse(" ".to_string()).collect::<String>(),
        &Options::twenty_four_hour(),
    )
    .map(|s| format!("{}{}", s, if possible { "" } else { " (UTC)" }))
    .map_err(|_| anyhow!("failed to parse cron string"))
}
