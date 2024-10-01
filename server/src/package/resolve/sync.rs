use std::{fs, path::PathBuf};

use alpm::{Alpm, SigLevel};
use anyhow::{Context, Result};
use log::{debug, info};

use crate::config::CONFIG;

/// databases that are used by stock pacman
const STOCK_DATABASES: [&str; 2] = ["core", "extra"];

const SYNC_FOLDER: &str = "sync";

/// A thread-safe wrapper around a raw [`alpm::Alpm`].
/// FIXME: remove this once Alpm is Sync: https://github.com/archlinux/alpm.rs/issues/42
pub struct AlpmWrapper {
    /// A wrapper around an inner database handle.
    pub alpm: Alpm,
}

impl From<Alpm> for AlpmWrapper {
    fn from(alpm: Alpm) -> Self {
        AlpmWrapper { alpm }
    }
}

unsafe impl Send for AlpmWrapper {}

/// creates an alpm reference and syncs the db
pub async fn create_and_sync() -> Result<Alpm> {
    debug!("starting alpm creation in new thread");

    // we do this in another thread as it could take a moment
    let wrapper = tokio::task::spawn_blocking(|| {
        let mut alpm = initialize_alpm()?;
        synchronize_alpm(&mut alpm)?;

        Ok::<_, anyhow::Error>(AlpmWrapper::from(alpm))
    })
    .await
    .context("failed to create alpm spawning thread")??;

    Ok(wrapper.alpm)
}

/// returns the server for a given database
fn get_server_for(name: &str) -> String {
    CONFIG.sync_mirror.replace("{repo}", name).replace("{arch}", &CONFIG.architecture)
}

/// initializes a libalpm reference by adding required databases and mirrors
fn initialize_alpm() -> Result<Alpm> {
    debug!("creating new libalpm reference");

    let path = PathBuf::from(SYNC_FOLDER);
    if !path.exists() {
        fs::create_dir(&path).context("failed to create folder for sync libalpm")?;
    }

    let mut alpm = Alpm::new("/", path.to_string_lossy().to_string().as_str())
        .context("failed to initialize libalpm")?;

    for db in STOCK_DATABASES {
        let handle = alpm
            .register_syncdb_mut(db, SigLevel::NONE) // we don't care whether our package index can be trusted
            .with_context(|| format!("failed to add database '{db}' to libalpm"))?;

        handle
            .add_server(get_server_for(db))
            .with_context(|| format!("failed to add server to database '{db}' of libalpm"))?;
    }

    Ok(Alpm::from(alpm))
}

/// updates the sync databases of a libalmp reference
fn synchronize_alpm(alpm: &mut Alpm) -> Result<()> {
    info!("updating sync databases");

    alpm.syncdbs_mut().update(false).context("failed to synchronize databases with libalpm")?;

    debug!("finished updating sync databases successfully");

    Ok(())
}
