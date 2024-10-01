use actix_web_lab::sse;
use actix_web_lab::sse::Sse;
use actix_web_lab::util::InfallibleStream;
use chrono::Utc;
use futures::future::join_all;
use log::{debug, trace};
use serene_data::package::BroadcastEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;

pub enum Event {
    BuildStart,
    BuildFinish,
    Log(String),
}

pub struct Broadcast {
    subscriptions: Mutex<HashMap<String, Vec<tokio::sync::mpsc::Sender<sse::Event>>>>,
    // cache contains build logs for packages which are currently building
    cache: Mutex<HashMap<String, Vec<String>>>,
}

impl Broadcast {
    pub fn new() -> Arc<Self> {
        let broadcast = Arc::new(Self {
            subscriptions: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
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

    /// remove all connections for which the ping broadcast fails to avoid
    /// unnecessary broadcasts
    async fn remove_stale_connections(&self) {
        let mut subscriptions = self.subscriptions.lock().await;

        *subscriptions = join_all(subscriptions.iter().map(|(package, receivers)| async {
            let receivers = join_all(receivers.iter().map(|recv| async {
                let event = sse::Event::Data(
                    sse::Data::new(Utc::now().timestamp_millis().to_string())
                        .event(BroadcastEvent::Ping.to_string()),
                );
                recv.send(event).await.ok().map(|_| recv.clone())
            }))
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            if !receivers.is_empty() {
                Some((package.clone(), receivers))
            } else {
                None
            }
        }))
        .await
        .into_iter()
        .flatten()
        .collect::<HashMap<_, _>>();
    }

    /// subscribe to all package events
    pub async fn subscribe(
        &self,
        package: String,
    ) -> actix_web::Result<Sse<InfallibleStream<ReceiverStream<sse::Event>>>> {
        let pkg = package.to_lowercase();
        let (tx, rx) = tokio::sync::mpsc::channel::<sse::Event>(10);
        let mut subscriptions = self.subscriptions.lock().await;
        let mut receivers = subscriptions.get(&pkg).cloned().unwrap_or_default();
        debug!("added new receiver for package {pkg}");
        receivers.push(tx.clone());
        subscriptions.insert(pkg.clone(), receivers);

        let cache = self.cache.lock().await;
        // should there be logs in the cache then there is currently a build running and
        // we want to return those logs
        if let Some(logs) = cache.get(&pkg) {
            let event = sse::Event::Data(
                sse::Data::new(logs.join("")).event(BroadcastEvent::Log.to_string()),
            );
            tx.send(event).await.ok();
        }

        Ok(Sse::from_infallible_receiver(rx))
    }

    /// notify all subscriptions for a specific package with an event
    pub async fn notify(&self, package: &str, event: Event) {
        let pkg = package.to_lowercase();
        let subscriptions = self.subscriptions.lock().await;
        let receivers = subscriptions.get(&pkg).cloned().unwrap_or_default();
        trace!("notifying package {pkg} with {} receivers", receivers.len());

        let mut cache = self.cache.lock().await;
        let event = match event {
            Event::BuildStart => {
                cache.insert(pkg, vec![]);
                sse::Event::Data(
                    sse::Data::new(String::new()).event(BroadcastEvent::BuildStart.to_string()),
                )
            }
            Event::BuildFinish => {
                cache.remove(&pkg);
                sse::Event::Data(
                    sse::Data::new(String::new()).event(BroadcastEvent::BuildEnd.to_string()),
                )
            }
            Event::Log(log) => {
                if let Some(logs) = cache.get_mut(&pkg) {
                    logs.push(log.clone())
                }
                sse::Event::Data(sse::Data::new(log).event(BroadcastEvent::Log.to_string()))
            }
        };

        for receiver in receivers {
            // we can ignore errors since the stale client gets removed in next cleanup
            // anyways
            receiver.send(event.clone()).await.ok();
        }
    }
}
