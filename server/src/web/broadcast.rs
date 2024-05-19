use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use futures::future::join_all;
use log::debug;
use sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;
use actix_web_lab::sse;
use actix_web_lab::sse::Sse;
use actix_web_lab::util::InfallibleStream;
use tokio_stream::wrappers::ReceiverStream;

use crate::build::BuildSummary;

const BUILD_START_EVENT: &str = "build_start";
const BUILD_END_EVENT: &str = "build_end";
const LOG_EVENT: &str = "log";
const PING_EVENT: &str = "ping"; 

pub enum BroadcastEvent {
    BuildStart,
    BuildFinish,
    Log(String)
}

pub struct Broadcast {
    subscriptions: Mutex<HashMap<String, Vec<tokio::sync::mpsc::Sender<sse::Event>>>>,
    // cache contains build logs for packages which are currently building
    cache: Mutex<HashMap<String, Vec<String>>>,
    db: Pool<Sqlite>
}

impl Broadcast {
    pub fn new(db: Pool<Sqlite>) -> Arc<Self> {
        let broadcast = Arc::new(Self { 
            subscriptions: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            db
        });
        Broadcast::spawn_ping(broadcast.clone());
        broadcast
    }

    fn spawn_ping(this: Arc<Self>) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;
                this.remove_stale_connections().await;
            }
        });
    }

    /// remove all connections for which the ping broadcast fails to avoid unnecessary broadcasts
    async fn remove_stale_connections(&self) {
        let mut subscriptions = self.subscriptions.lock().await;

        *subscriptions = join_all(
            subscriptions.iter().map(|(package, receivers)| async {
                let receivers = join_all(
                    receivers.iter().map(|recv| async {
                        let event = sse::Event::Data(sse::Data::new(Utc::now().timestamp_millis().to_string()).event(PING_EVENT));
                        recv.send(event).await.ok().map(|_| recv.clone())
                    })
                ).await.into_iter().filter_map(|r| r).collect::<Vec<_>>();
                if receivers.len() > 0 {
                    Some((package.clone(), receivers))
                } else {
                    None
                }
            })
        ).await.into_iter().filter_map(|e| e).collect::<HashMap<_, _>>();
    }

    /// subscribe to all package events
    pub async fn subscribe(&self, package: String) -> actix_web::Result<Sse<InfallibleStream<ReceiverStream<sse::Event>>>> {
        let pkg = package.to_lowercase();
        let (tx, rx) = tokio::sync::mpsc::channel::<sse::Event>(10);
        let mut subscriptions = self.subscriptions.lock().await;
        let mut receivers = subscriptions.get(&pkg).cloned().unwrap_or_default();
        debug!("added new receiver for package {pkg}");
        receivers.push(tx.clone());
        subscriptions.insert(pkg.clone(), receivers);

        let cache = self.cache.lock().await;
        // should there be logs in the cache then there is currently a build running and we want to return those logs
        // otherwise we return the latest logs from the database
        if let Some(logs) = cache.get(&pkg) {
            let event = sse::Event::Data(sse::Data::new(logs.join("")).event(LOG_EVENT));
            tx.send(event).await.ok();
        } else {
            if let Some(summary) = BuildSummary::find_latest_for_package(&pkg, &self.db).await.ok().unwrap_or_default() {
                if let Some(logs) = summary.logs {
                    let event = sse::Event::Data(sse::Data::new(logs.logs).event(LOG_EVENT));
                    tx.send(event).await.ok();
                    let event = sse::Event::Data(sse::Data::new(String::new()).event(BUILD_END_EVENT));
                    tx.send(event).await.ok();
                }
            }
        }

        Ok(Sse::from_infallible_receiver(rx))
    }

    /// notify all subscriptions for a specific package with an event
    pub async fn notify(&self, package: &String, event: BroadcastEvent) {
        let pkg = package.to_lowercase();
        let subscriptions = self.subscriptions.lock().await;
        let receivers = subscriptions.get(&pkg).cloned().unwrap_or_default();
        debug!("notifying package {pkg} with {} receivers", receivers.len());

        let mut cache = self.cache.lock().await;
        let event = match event {
            BroadcastEvent::BuildStart => {
                cache.insert(pkg, vec![]);
                sse::Event::Data(sse::Data::new(String::new()).event(BUILD_START_EVENT))
            },
            BroadcastEvent::BuildFinish => {
                cache.remove(&pkg);
                sse::Event::Data(sse::Data::new(String::new()).event(BUILD_END_EVENT))
            },
            BroadcastEvent::Log(log) => {
                if let Some(logs) = cache.get_mut(&pkg) {
                    logs.push(log.clone())
                }
                sse::Event::Data(sse::Data::new(log).event(LOG_EVENT))
            },
        };

        for receiver in receivers {
            // we can ignore errors since the stale client gets removed in next cleanup anyways
            receiver.send(event.clone()).await.ok();
        }
    }
}