use crate::config::Config;
use crate::web::{delete_empty, eventsource, get, post, post_simple, Result};
use reqwest_eventsource::Event;
use serene_data::build::BuildInfo;
use serene_data::package::{
    BroadcastEvent, PackageAddRequest, PackageBuildRequest, PackageInfo, PackagePeek,
    PackageSettingsRequest,
};
use serene_data::SereneInfo;
use std::str::FromStr;

pub fn get_info(c: &Config) -> Result<SereneInfo> {
    get::<SereneInfo>(c, "")
}

/// add a package
pub fn add_package(c: &Config, request: PackageAddRequest) -> Result<PackagePeek> {
    post::<PackageAddRequest, PackagePeek>(c, "package/add", request)
}

/// remove a package
pub fn remove_package(c: &Config, package: &str) -> Result<()> {
    delete_empty(c, &format!("package/{package}"))
}

/// build a package immediately
pub fn build_package(c: &Config, package: &str, request: PackageBuildRequest) -> Result<()> {
    post_simple(c, &format!("package/{package}/build"), request)
}

/// changes a setting of a package
pub fn set_package_setting(
    c: &Config,
    package: &str,
    request: PackageSettingsRequest,
) -> Result<()> {
    post_simple(c, &format!("package/{package}/set"), request)
}

/// get a specific build for a package
pub fn get_build(c: &Config, package: &str, id: &str) -> Result<BuildInfo> {
    get::<BuildInfo>(c, &format!("package/{package}/build/{id}"))
}

/// get multiple builds for a package
pub fn get_builds(c: &Config, package: &str, amount: Option<u32>) -> Result<Vec<BuildInfo>> {
    let query = amount.map(|u| format!("?count={u}")).unwrap_or_default();

    get::<Vec<BuildInfo>>(c, &format!("package/{package}/build{query}"))
}

/// gets the logs of a build
pub fn get_build_logs(c: &Config, package: &str, id: &str) -> Result<String> {
    get::<String>(c, &format!("package/{package}/build/{id}/logs"))
}

/// get the secret for the webhook of a given package
pub fn get_webhook_secret(c: &Config, package: &str) -> Result<String> {
    get::<String>(c, &format!("webhook/package/{package}/secret"))
}

/// get info about a specific package
pub fn get_package(c: &Config, package: &str) -> Result<PackageInfo> {
    get::<PackageInfo>(c, &format!("package/{package}"))
}

/// get info about all packages
pub fn get_packages(c: &Config) -> Result<Vec<PackagePeek>> {
    get::<Vec<PackagePeek>>(c, "package/list")
}

/// subscribe to build events and logs
pub fn subscribe_events<F>(c: &Config, package: &str, mut callback: F) -> Result<()>
where
    F: FnMut(BroadcastEvent, String) -> bool,
{
    eventsource(c, &format!("package/{package}/build/logs/subscribe"), |event| {
        if let Event::Message(event) = event {
            // ignore unknown events
            if let Ok(brd) = BroadcastEvent::from_str(&event.event) {
                return callback(brd, event.data);
            }
        }

        false
    })
}

// get last used pkgbuild of package
pub fn get_package_pkgbuild(c: &Config, package: &str) -> Result<String> {
    get::<String>(c, &format!("package/{package}/pkgbuild"))
}
