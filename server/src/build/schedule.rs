use std::collections::HashMap;
use std::sync::{Arc};
use std::time::Duration;
use anyhow::{anyhow, Context};
use log::{debug, info, warn};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;
use crate::build::Builder;
use crate::package::Package;

/// this struct schedules builds for all packages
pub struct BuildScheduler {
    builder: Arc<RwLock<Builder>>,

    sched: JobScheduler,
    jobs: HashMap<String, Uuid>,
    /// stores whether a package is currently being built
    locks: HashMap<String, Arc<RwLock<bool>>>
}

impl BuildScheduler {

    /// creates a new scheduler
    pub async fn new(builder: Arc<RwLock<Builder>>) -> anyhow::Result<Self> {
        Ok(Self {
            builder,
            sched: JobScheduler::new().await
                .context("failed to initialize job scheduler")?,
            jobs: HashMap::new(),
            locks: HashMap::new()
        })
    }

    /// starts the scheduler
    pub async fn start(&self) -> anyhow::Result<()> {
        self.sched.start().await
            .context("failed to start scheduler")
    }

    /// get the build lock of a package
    fn get_lock(&mut self, package: &Package) -> Arc<RwLock<bool>> {
        self.locks.entry(package.base.clone())
            .or_insert_with(|| Arc::new(RwLock::new(false)))
            .clone()
    }

    /// runs a one-shot build for a package
    pub async fn run(&mut self, package: &Package, clean: bool) -> anyhow::Result<()> {
        info!("scheduling one-shot build for package {} now", &package.base);

        let lock = self.get_lock(package);
        let base = package.base.clone();
        let builder = self.builder.clone();

        if *lock.read().await {
            return Err(anyhow!("cannot run build for package {base} now because lock for build is set"))
        }

        let job = Job::new_one_shot_async(Duration::from_secs(0), move |_, _| {
            let lock = lock.clone();
            let base = base.clone();
            let builder = builder.clone();

            Box::pin(async move { run(lock, builder, true, base, clean).await })
        }).context(format!("failed to create job for package {}", package.base))?;

        self.sched.add(job).await
            .context(format!("failed to schedule oneshot for package {}", &package.base))?;

        Ok(())
    }

    /// unschedules the build for a package
    pub async fn unschedule(&mut self, package: &Package) -> anyhow::Result<()> {
        if let Some(id) = self.jobs.remove(&package.base) {
            debug!("unscheduling job for {}", package.base);
            self.sched.remove(&id).await
                .context(format!("failed to unschedule job for package {}", package.base))?;
        }

        Ok(())
    }

    /// schedules the builds for a package
    pub async fn schedule(&mut self, package: &Package) -> anyhow::Result<()>{
        info!("scheduling recurring build for package {}", &package.base);
        self.unschedule(package).await?;

        let lock = self.get_lock(package);
        let base = package.base.clone();
        let builder = self.builder.clone();

        let job = Job::new_async(package.get_schedule().as_str(), move |_, _| {
            let lock = lock.clone();
            let base = base.clone();
            let builder = builder.clone();

            Box::pin(async move { run(lock, builder, false, base, false).await })
        }).context(format!("failed to create job for package {}", package.base))?;

        self.jobs.insert(package.base.clone(), job.guid());

        self.sched.add(job).await
            .context(format!("failed to schedule job for package {}", package.base))?;

        Ok(())
    }
}

/// runs a build for a package
async fn run(lock: Arc<RwLock<bool>>, builder: Arc<RwLock<Builder>>, force: bool, base: String, clean: bool) {
    // makes sure a package is not built twice at the same time
    if *lock.read().await {
        warn!("cancelling schedule for package {base} because the lock is set");
        return
    }

    *lock.write().await = true;
    builder.read().await.run_scheduled(&base, force, clean).await;
    *lock.write().await = false;
}