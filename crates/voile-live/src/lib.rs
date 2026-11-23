//! Rafraîchissement des Live Folders, hors du thread UI de CEF.
//!
//! [`LiveService::spawn`] démarre un thread avec un runtime tokio. Le shell
//! lui pousse la configuration ([`LiveService::configure`]) et reçoit les
//! résultats via un [`LiveSink`], qu'il re-poste sur le thread UI.

pub mod github;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use voile_core::{Id, LiveItem, LiveSource};

pub use github::{GithubClient, GithubError};

/// Reçoit les événements de synchro. Appelé depuis un thread tokio.
pub trait LiveSink: Send + Sync + 'static {
    fn started(&self, folder: Id);
    fn finished(&self, folder: Id, result: Result<Vec<LiveItem>, String>);
}

enum Msg {
    Configure {
        folders: Vec<(Id, LiveSource)>,
        every: Duration,
    },
    Refresh(Id),
}

#[derive(Clone)]
pub struct LiveService {
    tx: mpsc::UnboundedSender<Msg>,
}

impl LiveService {
    pub fn spawn(sink: impl LiveSink) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let sink: Arc<dyn LiveSink> = Arc::new(sink);
        std::thread::Builder::new()
            .name("voile-live".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("runtime tokio");
                rt.block_on(run(rx, sink));
            })
            .expect("thread live");
        Self { tx }
    }

    /// Remplace la liste des dossiers suivis. Ceux qui sont nouveaux sont
    /// synchronisés tout de suite.
    pub fn configure(&self, folders: Vec<(Id, LiveSource)>, every: Duration) {
        let _ = self.tx.send(Msg::Configure { folders, every });
    }

    pub fn refresh(&self, folder: Id) {
        let _ = self.tx.send(Msg::Refresh(folder));
    }
}

async fn run(mut rx: mpsc::UnboundedReceiver<Msg>, sink: Arc<dyn LiveSink>) {
    let client: Arc<Mutex<Option<GithubClient>>> = Arc::default();
    let mut folders: HashMap<Id, LiveSource> = HashMap::new();
    let mut every = Duration::from_secs(60);
    let mut tick = tokio::time::interval(every);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                None => break,
                Some(Msg::Configure { folders: next, every: e }) => {
                    let next: HashMap<_, _> = next.into_iter().collect();
                    for (id, source) in &next {
                        if folders.get(id) != Some(source) {
                            spawn_sync(*id, source.clone(), client.clone(), sink.clone());
                        }
                    }
                    folders = next;
                    if e != every {
                        every = e.max(Duration::from_secs(15));
                        tick = tokio::time::interval_at(tokio::time::Instant::now() + every, every);
                        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    }
                }
                Some(Msg::Refresh(id)) => {
                    if let Some(source) = folders.get(&id) {
                        spawn_sync(id, source.clone(), client.clone(), sink.clone());
                    }
                }
            },
            _ = tick.tick() => {
                for (id, source) in &folders {
                    spawn_sync(*id, source.clone(), client.clone(), sink.clone());
                }
            }
        }
    }
}

fn spawn_sync(
    folder: Id,
    source: LiveSource,
    client: Arc<Mutex<Option<GithubClient>>>,
    sink: Arc<dyn LiveSink>,
) {
    tokio::spawn(async move {
        sink.started(folder);
        let result = fetch(&source, &client).await.map_err(|e| e.to_string());
        if let Err(e) = &result {
            tracing::warn!(folder, "synchro Live Folder échouée : {e}");
        }
        sink.finished(folder, result);
    });
}

async fn fetch(
    source: &LiveSource,
    client: &Mutex<Option<GithubClient>>,
) -> Result<Vec<LiveItem>, GithubError> {
    match source {
        LiveSource::GithubSearch { query } => {
            let gh = {
                let mut guard = client.lock().await;
                if guard.is_none() {
                    *guard = Some(GithubClient::from_env().await?);
                }
                guard.clone().expect("initialisé")
            };
            let res = gh.search(query).await;
            if matches!(res, Err(GithubError::Unauthorized)) {
                // Token révoqué/changé : on le relira au prochain essai.
                *client.lock().await = None;
            }
            res
        }
    }
}
