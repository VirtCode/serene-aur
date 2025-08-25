use crate::build::session::BuildSession;
use crate::build::BuilderInstance;
use crate::database::Database;
use crate::package::srcinfo::SrcinfoGeneratorInstance;
use crate::package::Package;
use crate::web::broadcast::BroadcastInstance;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use cron::Schedule;
use log::{debug, error, info, warn};
use serene_data::build::BuildReason;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};

/// metadata associated with a build
/// can be used to override stuff like clean
pub struct BuildMeta {
    /// reason the build started
    pub reason: BuildReason,
    /// should build in order of dependencies (and abort if necessary)
    pub resolve: bool,
    /// remove the containers of the packages before building
    pub clean: bool,
    /// don't check if a package can be updated
    pub force: bool,
}

impl BuildMeta {
    pub fn new(reason: BuildReason, resolve: bool, clean: bool, force: bool) -> Self {
        Self { resolve, reason, clean, force }
    }
    pub fn normal(reason: BuildReason) -> Self {
        Self::new(reason, true, false, false)
    }
}

/// this struct schedules builds for all packages
pub struct BuildScheduler {
    db: Database,
    builder: BuilderInstance,
    broadcast: BroadcastInstance,
    srcinfo_generator: SrcinfoGeneratorInstance,

    signal: Option<Sender<()>>,
    jobs: Arc<Mutex<HashMap<DateTime<Utc>, HashSet<String>>>>,
    lock: Arc<Mutex<HashSet<String>>>,
}

impl BuildScheduler {
    /// creates a new scheduler
    pub async fn new(
        builder: BuilderInstance,
        db: Database,
        broadcast: BroadcastInstance,
        srcinfo_generator: SrcinfoGeneratorInstance,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            builder,
            db,
            broadcast,
            srcinfo_generator,
            signal: None,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            lock: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// runs a one-shot build for a package
    pub async fn run(&self, packages: Vec<Package>, meta: BuildMeta) -> anyhow::Result<()> {
        info!("scheduling one-shot build for some packages now");

        // we prematurely check for build lock, such that the user receives an error
        {
            let locked = self.lock.lock().await;

            for package in &packages {
                if locked.contains(&package.base) {
                    warn!("not building one-shot, {} is locked", &package.base);
                    return Err(anyhow!(
                        "cannot build now, {} is currently in a running build session",
                        &package.base
                    ));
                }
            }
        }

        let builder = self.builder.clone();
        let lock = self.lock.clone();
        let db = self.db.clone();
        let broadcast = self.broadcast.clone();
        let srcinfo_generator = self.srcinfo_generator.clone();

        tokio::spawn(async move {
            Self::run_now(packages, builder, lock, db, broadcast, srcinfo_generator, meta).await
        });

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

        let jobs = self.jobs.clone();
        let db = self.db.clone();
        let broadcast = self.broadcast.clone();
        let srcinfo_generator = self.srcinfo_generator.clone();
        let builder = self.builder.clone();
        let lock = self.lock.clone();

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

                    // run build
                    let builder = builder.clone();
                    let lock = lock.clone();
                    let db = db.clone();
                    let broadcast = broadcast.clone();
                    let srcinfo_generator = srcinfo_generator.clone();

                    tokio::spawn(async move {
                        Self::run_now(
                            packages,
                            builder,
                            lock,
                            db,
                            broadcast,
                            srcinfo_generator,
                            BuildMeta::normal(BuildReason::Schedule),
                        )
                        .await
                    });
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
    async fn run_now(
        mut packages: Vec<Package>,
        builder: BuilderInstance,
        lock: Arc<Mutex<HashSet<String>>>,
        db: Database,
        broadcast: BroadcastInstance,
        srcinfo_generator: SrcinfoGeneratorInstance,
        meta: BuildMeta,
    ) {
        info!(
            "running build for these packages: {}",
            packages.iter().map(|p| p.base.clone()).collect::<Vec<_>>().join(", ")
        );

        // remove duplicates, as this would cause problems later down the line
        packages.sort_by(|a, b| a.base.cmp(&b.base));
        packages.dedup_by(|a, b| a.base == b.base);

        {
            let mut locked = lock.lock().await;

            // remove locked packages
            packages.retain(|p| {
                if locked.contains(&p.base) {
                    warn!("package {} will not be built as it is currently locked", p.base);
                    false
                } else {
                    true
                }
            });

            // lock packages for build
            for package in &packages {
                locked.insert(package.base.clone());
            }
        }

        // update sources here as they are needed for the up-to-date check, and also for
        // the resolving
        for package in &mut packages {
            if let Err(e) = package.update(&srcinfo_generator).await {
                warn!("failed to update source for {}: {e:#}", package.base);
            } else if let Err(e) = package.change_sources(&db).await {
                error!("failed to store updated source in db for {}: {e:#}", package.base);
            }
        }

        // remove packages which are already built (and unlock them)
        if !meta.force {
            let mut locked = lock.lock().await;

            for p in packages.extract_if(.., |p| p.newest_built()) {
                debug!("skipping build for {}, is up-to-date", p.base);
                locked.remove(&p.base);
            }
        }

        let targets = packages.iter().map(|p| p.base.clone()).collect::<HashSet<_>>();

        match BuildSession::start(packages, &db, builder, broadcast, meta).await {
            Ok(mut session) => {
                if let Err(e) = session.run().await {
                    error!("failed to run build session: {e:#}");
                }
            }
            Err(e) => {
                error!("failed to start build session: {e:#}");
            }
        };

        {
            // unlock packages after build
            let mut locked = lock.lock().await;

            for package in targets {
                locked.remove(&package);
            }
        }
    }
}
