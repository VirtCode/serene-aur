use crate::build::session::BuildSession;
use crate::build::Builder;
use crate::config::CONFIG;
use crate::database::Database;
use crate::package::Package;
use crate::runner::Runner;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use cron::Schedule;
use log::{debug, error, info, warn};
use serene_data::build::BuildReason;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::LocalSet;
use tokio_cron_scheduler::{Job, JobScheduler};
use tokio_stream::StreamExt;
use uuid::Uuid;

/// this struct schedules builds for all packages
pub struct BuildScheduler {
    db: Database,
    builder: Arc<RwLock<Builder>>,

    signal: Option<Sender<()>>,
    queue: Option<Sender<Vec<Package>>>,
    jobs: Arc<Mutex<HashMap<DateTime<Utc>, HashSet<String>>>>,
}

impl BuildScheduler {
    /// creates a new scheduler
    pub async fn new(builder: Arc<RwLock<Builder>>, db: Database) -> anyhow::Result<Self> {
        Ok(Self {
            builder,
            db,
            signal: None,
            queue: None,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// runs a one-shot build for a package
    pub async fn run(
        &self,
        packages: Vec<Package>,
        clean: bool,
        reason: BuildReason,
    ) -> anyhow::Result<()> {
        info!("scheduling one-shot build for some packages now");

        let Some(queue) = &self.queue else {
            return Err(anyhow!("scheduler is not yet started, build won't happen"));
        };

        queue.send(packages).await.context("failed to submit packages")?;

        Ok(())
    }

    /// schedules the builds for a package
    pub async fn schedule(&mut self, package: &Package) -> anyhow::Result<()> {
        info!("scheduling recurring build for package {}", &package.base);
        self.unschedule(package).await?;

        Self::schedule_into(&[package.clone()], &self.jobs).await;
        if let Some(signal) = &mut self.signal {
            signal.send(()).await.context("failed to signal rescheduling")?;
        }

        Ok(())
    }

    /// unschedules the build for a package
    pub async fn unschedule(&mut self, package: &Package) -> anyhow::Result<()> {
        for set in self.jobs.lock().await.values_mut() {
            set.remove(&package.base);
        }

        Ok(())
    }

    /// starts the scheduling thread
    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.signal.is_some() {
            return Err(anyhow!("tried to start scheduler twice!"));
        }

        let (tx, mut rx) = mpsc::channel::<()>(1);
        self.signal = Some(tx);

        let queue = self.spawn_runner();
        self.queue = Some(queue.clone());

        let jobs = self.jobs.clone();
        let db = self.db.clone();
        let builder = self.builder.clone();

        tokio::spawn(async move {
            loop {
                let min = jobs.lock().await.keys().min().cloned();

                if let Some(min) = min {
                    debug!("blocking until next schedule target at {min:#}");

                    // only wait if time is in the future (otherwise this would error)
                    if let Ok(duration) = (min - Utc::now()).to_std() {
                        let timeout = tokio::time::sleep(duration);

                        select! {
                            result = rx.recv() => { match result {
                                None => { break; }
                                Some(_) => { continue; }
                            }}
                            _ = timeout => {}
                        }
                    }

                    debug!("schedule target reached");

                    let Some(set) = jobs.lock().await.remove(&min) else {
                        warn!("schedule target no longer present, rescheduling");
                        continue;
                    };

                    let mut packages = vec![];
                    for base in set {
                        match Package::find(&base, &db).await {
                            Ok(Some(p)) => packages.push(p),
                            Ok(None) => {
                                warn!("package with base {base} was scheduled but is no longer present")
                            }
                            Err(e) => {
                                error!("failed to access database: {e:#}")
                            }
                        }
                    }

                    // reschedule these packages
                    Self::schedule_into(&packages, &jobs).await;

                    // submit to builder thread
                    if let Err(e) = queue.send(packages).await {
                        error!("failed to schedule build, sending builder thread failed: {e:#}")
                    }
                } else {
                    debug!("blocking until woken with reschedule");

                    // block until we receive something
                    if let None = rx.recv().await {
                        break;
                    }
                }
            }

            info!("channel for scheduler closed, no rescheduling happening anymore");
        });

        Ok(())
    }

    /// this spawns an OS thread which will run the builds
    /// this is necessary, because Alpm is not send, and thus we cannot run the
    /// resolving on tokio
    /// see https://docs.rs/tokio/latest/tokio/task/struct.LocalSet.html#use-inside-tokiospawn
    /// FIXME: remove this once Alpm is sync: https://github.com/archlinux/alpm.rs/issues/42
    fn spawn_runner(&self) -> Sender<Vec<Package>> {
        let (tx, mut rx) = mpsc::channel(1);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("no build tokio runtime?");

        let builder = self.builder.clone();
        let db = self.db.clone();
        std::thread::spawn(move || {
            let local = LocalSet::new();

            local.spawn_local(async move {
                // receive stuff from channel
                while let Some(packages) = rx.recv().await {
                    let db = db.clone();
                    let builder = builder.clone();

                    // we can use spawn_local here
                    tokio::task::spawn_local(async move {
                        Self::run_now(packages, &builder, &db).await;
                    });
                }
            });

            rt.block_on(local)
        });

        tx
    }

    /// schedules a set of packages into the given schedule map
    /// this is usually done before they are run so they are ready for the next
    /// target
    async fn schedule_into(
        package: &[Package],
        targets: &Arc<Mutex<HashMap<DateTime<Utc>, HashSet<String>>>>,
    ) {
        for package in package {
            let Ok(schedule) = Schedule::from_str(&package.get_schedule()) else {
                error!("failed to schedule package {}, couldn't parse cron", package.base);
                return;
            };

            let Some(time) = schedule.upcoming(Utc).next() else {
                error!("failed to schedule package {}, cron string won't happen", package.base);
                return;
            };

            let mut jobs = targets.lock().await;

            if let Some(set) = jobs.get_mut(&time) {
                set.insert(package.base.clone());
            } else {
                jobs.insert(time, HashSet::from([package.base.clone()]));
            }
        }
    }

    /// runs a build for a set of packages right now
    async fn run_now(packages: Vec<Package>, builder: &Arc<RwLock<Builder>>, db: &Database) {
        info!(
            "running build for these packages: {}",
            packages.iter().map(|p| p.base.clone()).collect::<Vec<_>>().join(", ")
        );

        // TODO: filter for updateable
        // TODO: respect stuff like clean, reason, resolve
        match BuildSession::start(packages, BuildReason::Unknown, db, builder.clone(), true).await {
            Ok(mut session) => {
                if let Err(e) = session.run().await {
                    error!("failed to run build session: {e:#}");
                }
            }
            Err(e) => {
                error!("failed to start build session: {e:#}");
            }
        };
    }
}
