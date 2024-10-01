use crate::config::CONFIG;
use crate::runner::Runner;
use anyhow::Context;
use log::{error, info};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

/// Schedules the pulling of the runner image
pub struct ImageScheduler {
    runner: Arc<RwLock<Runner>>,
    sched: JobScheduler,
    job: Uuid,
}

impl ImageScheduler {
    /// creates a new image scheduler
    pub async fn new(runner: Arc<RwLock<Runner>>) -> anyhow::Result<Self> {
        let mut s = Self {
            runner,
            sched: JobScheduler::new().await.context("failed to initialize job scheduler")?,
            job: Uuid::from_u128(0u128),
        };

        s.schedule().await?;
        Ok(s)
    }

    /// starts the scheduler
    pub async fn start(&self) -> anyhow::Result<()> {
        self.sched.start().await.context("failed to start image scheduler")
    }

    // schedules an image update
    async fn schedule(&mut self) -> anyhow::Result<()> {
        let runner = self.runner.clone();

        info!("scheduling image update");

        let job = Job::new_async(CONFIG.schedule_image.as_str(), move |_, _| {
            let runner = runner.clone();

            Box::pin(async move {
                info!("updating runner image");

                if let Err(e) = runner.read().await.update_image().await {
                    error!("failed to update runner image: {e:#}");
                } else {
                    info!("successfully updated runner image");
                }
            })
        })
        .context("failed to schedule job image updating")?;

        self.job = job.guid();
        self.sched.add(job).await.context("failed to schedule image update")?;

        Ok(())
    }
}
