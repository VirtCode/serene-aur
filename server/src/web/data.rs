use crate::build::BuildSummary;
use crate::package::Package;
use serene_data::build::BuildInfo;
use serene_data::package::{PackageInfo, PackagePeek};

impl Package {
    pub fn to_peek(&self, build: Option<BuildSummary>) -> PackagePeek {
        PackagePeek {
            base: self.base.clone(),
            enabled: self.enabled,
            devel: self.source.devel,
            version: self.get_version(),
            added: self.added,
            members: self.get_packages(),
            build: build.map(|b| b.as_info()),
        }
    }

    pub fn to_info(&self, build_count: u32) -> PackageInfo {
        PackageInfo {
            base: self.base.clone(),
            members: self.get_packages(),
            description: self.get_description(),
            builds: build_count,
            version: self.get_version(),
            source: self.source.get_type(),
            source_url: self.source.get_url(),
            devel: self.source.devel,
            srcinfo_override: self.source.srcinfo_override,
            enabled: self.enabled,
            clean: self.clean,
            dependency: self.dependency,
            schedule: self.get_schedule(),
            schedule_changed: self.schedule.is_some(),
            prepare_commands: self.prepare.clone(),
            makepkg_flags: self.flags.clone(),
            added: self.added,
        }
    }
}

impl BuildSummary {
    pub fn as_info(&self) -> BuildInfo {
        BuildInfo {
            version: self.version.clone(),
            state: self.state.clone(),
            started: self.started,
            ended: self.ended,
            reason: self.reason,
        }
    }
}
