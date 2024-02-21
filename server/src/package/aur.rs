use raur::Raur;

// this struct represents information about a package in the aur
pub struct AurInfo {
    // base name of the package
    pub base: String,
    // repository of the package source
    pub repository: String,
    // is development package
    pub devel: bool
}

/// finds a package in the aur
pub async fn find(name: &str) -> anyhow::Result<Option<AurInfo>> {
    let raur = raur::Handle::new();
    let pkg = raur.info(&[name]).await?;

    if let Some(info) = pkg.first() {
        Ok(Some(AurInfo {
            base: info.package_base.clone(),
            repository: to_aur_git(&info.package_base),
            devel: info.package_base.ends_with("-git")
        }))
    } else {
        Ok(None)
    }
}

fn to_aur_git(base: &str) -> String {
    format!("https://aur.archlinux.org/{base}.git")
}