use actix_web_lab::sse;
use actix_web_lab::sse::{Data, Event, Sse};
use actix_web_lab::util::InfallibleStream;
use chrono::Utc;
use futures::future::join_all;
use log::{debug, error, trace};
use serene_data::build::BuildState;
use serene_data::package::BroadcastEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;

pub struct Broadcast {
    subscriptions: Mutex<HashMap<String, Vec<tokio::sync::mpsc::Sender<sse::Event>>>>,
    // cache contains build logs for packages which are currently building
    cache: Mutex<HashMap<String, (Vec<String>, BuildState)>>,
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
                recv.send(
                    Self::create_event("", BroadcastEvent::Ping)
                        .expect("ping should be serializable"),
                )
                .await
                .ok()
                .map(|_| recv.clone())
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
        if let Some((logs, state)) = cache.get(&pkg) {
            if let Some(state) = Self::create_event(&pkg, BroadcastEvent::Change(state.clone())) {
                let _ = tx.send(state).await;
            } else {
                error!("failed serialize state to send to new receiver");
            }

            if let Some(logs) = Self::create_event(&pkg, BroadcastEvent::Log(logs.join(""))) {
                let _ = tx.send(logs).await;
            } else {
                error!("failed serialize logs to send to new receiver");
            }
        }

        Ok(Sse::from_infallible_receiver(rx))
    }

    /// send a state change through the event source
    pub async fn change(&self, package: &str, state: BuildState) {
        let mut cache = self.cache.lock().await;
        let package = package.to_owned();

        // initialize or remove cache
        if state.done() {
            cache.remove(&package);
        } else if let Some((_, s)) = cache.get_mut(&package) {
            *s = state.clone();
        } else {
            cache.insert(package.to_owned(), (vec![], state.clone()));
        }

        self.notify(&package, BroadcastEvent::Change(state)).await
    }

    /// send a log through the event source
    pub async fn log(&self, package: &str, log: String) {
        let mut cache = self.cache.lock().await;
        let package = package.to_owned();

        // add logs to cache
        if let Some((logs, _)) = cache.get_mut(&package) {
            logs.push(log.clone())
        }

        self.notify(&package, BroadcastEvent::Log(log)).await
    }

    /// notify all subscriptions for a specific package with an event
    pub async fn notify(&self, package: &str, event: BroadcastEvent) {
        let package = package.to_owned();
        let subscriptions = self.subscriptions.lock().await;
        let receivers = subscriptions.get(&package).cloned().unwrap_or_default();

        trace!("notifying package {package} with {} receivers", receivers.len());

        let Some(event) = Self::create_event(&package, event) else {
            error!("failed to serialize event to send to event source");
            return;
        };

        for receiver in receivers {
            // we can ignore errors since the stale client gets removed in next cleanup
            // anyways
            receiver.send(event.clone()).await.ok();
        }
    }

    /// create a sse event for a package and an event
    fn create_event(package: &str, event: BroadcastEvent) -> Option<Event> {
        serde_json::to_string(&event).map(|event| Event::Data(Data::new(event).event(package))).ok()
    }
}
