use crate::config::CONFIG;
use crate::runner::{Runner, RunnerInstance};
use anyhow::Context;
use chrono::Utc;
use log::{debug, error, info};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Schedules the pulling of the runner image
pub struct ImageScheduler {
    runner: RunnerInstance,
}

impl ImageScheduler {
    /// creates a new image scheduler
    pub fn new(runner: RunnerInstance) -> Self {
        Self { runner }
    }

    /// starts the scheduler
    pub async fn start(&self) -> anyhow::Result<()> {
        let runner = self.runner.clone();
        let cron = cron::Schedule::from_str(&CONFIG.schedule_image)
            .context("failed to parse image cron string")?;

        tokio::task::spawn(async move {
            loop {
                let Some(time) = cron.upcoming(Utc).next() else {
                    error!("image schedule cron string has no time, aborting image scheduler");
                    break;
                };

                debug!("blocking until next image schedule {time:#}");

                if let Ok(duration) = (time - Utc::now()).to_std() {
                    tokio::time::sleep(duration).await;

                    Self::run_now(&runner).await;
                } else {
                    error!("next image schedule out of range, aborting image scheduler");
                    break;
                }
            }

            debug!("image scheduler finished");
        });

        Ok(())
    }

    // runs updating now, in the evoking thread
    pub async fn run_sync(&self) {
        Self::run_now(&self.runner).await;
    }

    // runs updating now, in a nother task
    pub async fn run_async(&self) {
        let runner = self.runner.clone();

        tokio::task::spawn(async move {
            Self::run_now(&runner).await;
        });
    }

    async fn run_now(runner: &RunnerInstance) {
        if let Err(e) = runner.update_image().await {
            error!("failed to update runner image: {e:#}");
        } else {
            info!("successfully updated runner image");
        }
    }
}
