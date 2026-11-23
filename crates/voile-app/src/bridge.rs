//! Pont vers le thread UI de CEF.
//!
//! Tous les callbacks CEF (thread UI, IO, threads tokio des Live Folders)
//! poussent un [`Event`] ici ; il est traité plus tard, sur le thread UI, par
//! le [`crate::shell::Shell`]. Ça évite toute réentrance : un appel CEF fait
//! depuis le shell peut déclencher un callback synchrone, qui se contente
//! alors d'empiler un événement.

use crate::frame::FrameId;
use crate::shell::UiAction;
use cef::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use voile_core::Command;

#[derive(Debug)]
pub enum Event {
    /// Commande venue de la sidebar ou de la page d'accueil.
    Ui(Command),
    /// Commande émise par le shell lui-même (mise à jour d'onglet, résultat
    /// de synchro…). Une erreur n'est pas affichée à l'utilisateur.
    Internal(Command),
    Action(UiAction),
    SidebarReady(FrameId),
    WindowClosing(FrameId),
    WindowDestroyed(FrameId),
    Save,
    Tick,
}

static QUEUE: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static PENDING: AtomicBool = AtomicBool::new(false);

/// Empile un événement et planifie son traitement sur le thread UI.
/// Appelable depuis n'importe quel thread.
pub fn send(event: Event) {
    QUEUE.lock().expect("file d'événements").push_back(event);
    if !PENDING.swap(true, Ordering::AcqRel) {
        let mut task = DrainTask::new();
        post_task(ThreadId::UI, Some(&mut task));
    }
}

/// Envoie `event` après `delay_ms` millisecondes.
pub fn send_later(delay_ms: i64, event: Event) {
    let mut task = DelayedTask::new(Arc::new(Mutex::new(Some(event))));
    post_delayed_task(ThreadId::UI, Some(&mut task), delay_ms);
}

fn drain() {
    loop {
        let Some(event) = QUEUE.lock().expect("file d'événements").pop_front() else {
            break;
        };
        if let Err(event) = crate::shell::handle(event) {
            // Shell déjà emprunté (boucle de messages imbriquée, ex. boîte de
            // dialogue modale) : on rendra la main et on réessaiera.
            QUEUE.lock().expect("file d'événements").push_front(event);
            if !PENDING.swap(true, Ordering::AcqRel) {
                let mut task = DrainTask::new();
                post_delayed_task(ThreadId::UI, Some(&mut task), 16);
            }
            return;
        }
    }
    crate::shell::flush();
}

wrap_task! {
    struct DrainTask;

    impl Task {
        fn execute(&self) {
            PENDING.store(false, Ordering::Release);
            drain();
        }
    }
}

wrap_task! {
    struct DelayedTask {
        event: Arc<Mutex<Option<Event>>>,
    }

    impl Task {
        fn execute(&self) {
            if let Some(event) = self.event.lock().ok().and_then(|mut e| e.take()) {
                send(event);
            }
        }
    }
}
