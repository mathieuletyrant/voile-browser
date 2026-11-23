//! Cœur de Voile : le modèle façon Arc (espaces, favoris, épinglés, Today,
//! dossiers, Live Folders) et la logique de commandes → effets.
//!
//! Ce crate ne dépend pas de CEF : le shell (`voile-app`) lui envoie des
//! [`Command`] et exécute les [`Effect`] renvoyés.

pub mod command;
pub mod input;
pub mod live;
pub mod model;
pub mod store;
pub mod view;
pub mod workspace;

pub use command::{Command, Container, Effect, Location, TabUpdate};
pub use model::*;
pub use store::Store;
pub use workspace::Workspace;

/// Secondes Unix courantes.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests;
