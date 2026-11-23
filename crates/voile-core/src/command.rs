//! Protocole entre la sidebar (JS), le shell CEF et le cœur.
//!
//! * [`Command`] : ce que l'UI ou le shell demande.
//! * [`Effect`] : ce que le shell CEF doit exécuter en retour (créer/montrer
//!   une vue, naviguer…). Le cœur ne touche jamais CEF directement, ce qui le
//!   rend entièrement testable.

use crate::model::{Id, LiveItem, LiveSource, NodeRef, Theme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    // --- onglets -------------------------------------------------------------
    /// Texte de la barre de commande : URL, domaine nu ou recherche.
    Open {
        input: String,
        new_tab: bool,
    },
    Activate {
        tab: Id,
    },
    Close {
        tab: Id,
    },
    Back {
        tab: Id,
    },
    Forward {
        tab: Id,
    },
    Reload {
        tab: Id,
    },
    /// Épingle un onglet du « Today » (en fin de zone épinglée).
    Pin {
        tab: Id,
    },
    Unpin {
        tab: Id,
    },
    Favorite {
        tab: Id,
    },
    Unfavorite {
        tab: Id,
    },
    /// Ramène un onglet épinglé sur son URL d'origine.
    ResetPinned {
        tab: Id,
    },
    ClearToday,

    // --- arbre ---------------------------------------------------------------
    CreateFolder {
        name: String,
    },
    Rename {
        node: NodeRef,
        name: String,
    },
    ToggleOpen {
        node: NodeRef,
    },
    Delete {
        node: NodeRef,
    },
    Move {
        node: NodeRef,
        to: Location,
    },

    // --- espaces -------------------------------------------------------------
    SwitchSpace {
        space: Id,
    },
    CreateSpace {
        name: String,
        icon: String,
        theme: Theme,
    },
    UpdateSpace {
        space: Id,
        name: Option<String>,
        icon: Option<String>,
        theme: Option<Theme>,
    },

    // --- Live Folders --------------------------------------------------------
    AddLiveFolder {
        name: String,
        source: LiveSource,
    },
    RefreshLive {
        folder: Id,
    },
    OpenLiveItem {
        folder: Id,
        key: String,
    },
    MarkLiveRead {
        folder: Id,
    },

    // --- réglages --------------------------------------------------------------
    SetShowToolbar {
        show: bool,
    },

    // --- événements remontés par le shell (pas par l'UI) ----------------------
    #[serde(skip)]
    TabUpdated {
        tab: Id,
        update: TabUpdate,
    },
    #[serde(skip)]
    LiveSyncStarted {
        folder: Id,
    },
    #[serde(skip)]
    LiveResult {
        folder: Id,
        result: Result<Vec<LiveItem>, String>,
    },
    #[serde(skip)]
    /// Un lien a demandé un nouvel onglet (target=_blank, clic molette…).
    PopupRequested {
        from: Id,
        url: String,
    },
    #[serde(skip)]
    Tick {
        now: u64,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TabUpdate {
    pub url: Option<String>,
    pub title: Option<String>,
    pub favicon: Option<String>,
    pub loading: Option<bool>,
    pub can_go_back: Option<bool>,
    pub can_go_forward: Option<bool>,
}

/// Destination d'un glisser-déposer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub space: Id,
    pub container: Container,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Container {
    Favorites,
    Pinned,
    Today,
    Folder(Id),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Crée la vue navigateur de l'onglet (sans l'afficher).
    CreateView {
        tab: Id,
        url: String,
    },
    /// Affiche cette vue et masque les autres.
    ShowView {
        tab: Id,
    },
    /// N'affiche aucune page (plus aucun onglet actif).
    ShowEmpty,
    DestroyView {
        tab: Id,
    },
    LoadUrl {
        tab: Id,
        url: String,
    },
    GoBack {
        tab: Id,
    },
    GoForward {
        tab: Id,
    },
    Reload {
        tab: Id,
    },
    /// La liste/config des Live Folders a changé : relancer le planificateur.
    LiveConfigChanged,
    /// Synchroniser un Live Folder maintenant.
    LiveRefresh {
        folder: Id,
    },
    /// La barre Chrome doit être recréée (changement de réglage).
    ToolbarChanged,
    /// Couleurs de l'espace actif (fond de la fenêtre autour de la page).
    Theme(Theme),
}
