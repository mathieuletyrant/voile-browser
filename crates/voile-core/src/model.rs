//! Données persistées du navigateur. Tout est sérialisé tel quel dans
//! `workspace.json` (même idée que le `StorableSidebar.json` d'Arc).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Id = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: Id,
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub favicon: Option<String>,
    /// URL « maison » d'un onglet épinglé : fermer l'onglet le décharge et le
    /// ramène ici au lieu de le supprimer (comportement d'Arc).
    #[serde(default)]
    pub pinned_url: Option<String>,
    /// Onglet ouvert depuis un Live Folder.
    #[serde(default)]
    pub origin: Option<LiveOrigin>,
    /// Secondes Unix de la dernière activation (sert à l'archivage auto).
    #[serde(default)]
    pub last_active: u64,

    // --- état d'exécution, remis à zéro au chargement -----------------------
    /// Une vue CEF existe pour cet onglet.
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub loading: bool,
    #[serde(default)]
    pub can_go_back: bool,
    #[serde(default)]
    pub can_go_forward: bool,
}

impl Tab {
    pub fn new(id: Id, url: impl Into<String>) -> Self {
        let url = url.into();
        Self {
            id,
            title: url.clone(),
            url,
            favicon: None,
            pinned_url: None,
            origin: None,
            last_active: 0,
            loaded: false,
            loading: false,
            can_go_back: false,
            can_go_forward: false,
        }
    }

    pub(crate) fn reset_runtime(&mut self) {
        self.loaded = false;
        self.loading = false;
        self.can_go_back = false;
        self.can_go_forward = false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveOrigin {
    pub folder: Id,
    pub key: String,
}

/// Un élément de la zone épinglée d'un espace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Node {
    Tab { tab: Id },
    Folder(Folder),
    Live(LiveFolder),
}

impl Node {
    pub fn node_ref(&self) -> NodeRef {
        match self {
            Node::Tab { tab } => NodeRef::Tab(*tab),
            Node::Folder(f) => NodeRef::Folder(f.id),
            Node::Live(l) => NodeRef::Live(l.id),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum NodeRef {
    Tab(Id),
    Folder(Id),
    Live(Id),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: Id,
    pub name: String,
    #[serde(default = "yes")]
    pub open: bool,
    #[serde(default)]
    pub children: Vec<Node>,
}

fn yes() -> bool {
    true
}

/// Dossier alimenté automatiquement par une source externe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveFolder {
    pub id: Id,
    pub name: String,
    #[serde(default = "yes")]
    pub open: bool,
    pub source: LiveSource,
    #[serde(default)]
    pub items: Vec<LiveItem>,
    /// `key -> updated_at` vus par l'utilisateur (pour la pastille « non lu »).
    #[serde(default)]
    pub seen: BTreeMap<String, String>,
    #[serde(default)]
    pub status: LiveStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LiveSource {
    /// Recherche GitHub (syntaxe de la barre de recherche), ex.
    /// `is:pr is:open review-requested:@me archived:false`.
    #[serde(rename_all = "camelCase")]
    GithubSearch { query: String },
}

impl LiveSource {
    pub fn github_review_requested() -> Self {
        Self::GithubSearch {
            query: "is:pr is:open review-requested:@me archived:false".into(),
        }
    }
    pub fn github_my_prs() -> Self {
        Self::GithubSearch {
            query: "is:pr is:open author:@me archived:false".into(),
        }
    }
    pub fn github_repo(repo: &str) -> Self {
        Self::GithubSearch {
            query: format!("is:pr is:open repo:{repo}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveItem {
    /// Identifiant stable, ex. `owner/repo#123`.
    pub key: String,
    pub title: String,
    pub url: String,
    /// Ligne secondaire, ex. `owner/repo · @auteur`.
    pub subtitle: String,
    pub updated_at: String,
    #[serde(default)]
    pub badges: Vec<Badge>,
    #[serde(default)]
    pub avatar: Option<String>,
    /// Calculé au merge : nouveau ou modifié depuis la dernière ouverture.
    #[serde(default)]
    pub unread: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Badge {
    Draft,
    ChecksPassing,
    ChecksFailing,
    ChecksPending,
    Approved,
    ChangesRequested,
    ReviewRequired,
    Conflicts,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveStatus {
    #[serde(default)]
    pub syncing: bool,
    #[serde(default)]
    pub last_sync: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: Id,
    pub name: String,
    pub icon: String,
    pub theme: Theme,
    #[serde(default)]
    pub pinned: Vec<Node>,
    /// Onglets non épinglés (« Today » chez Arc), du plus récent au plus ancien.
    #[serde(default)]
    pub today: Vec<Id>,
    /// Dépôts GitHub de l'espace (`owner/nom`, ou `owner` pour tous les siens) :
    /// ses Live Folders ne montrent que ceux-là. Vide : pas de filtre.
    #[serde(default)]
    pub github_repos: Vec<String>,
}

/// Dégradé de la sidebar, en hex CSS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub from: String,
    pub to: String,
}

impl Theme {
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Les onglets non épinglés inactifs depuis plus longtemps sont archivés.
    pub archive_after_hours: u32,
    /// Affiche la barre Chrome au-dessus de la page (omnibox, icônes
    /// d'extensions, menu). Sans elle, les extensions tournent mais leurs
    /// popups ne sont pas accessibles.
    pub show_toolbar: bool,
    /// `%s` est remplacé par la requête encodée.
    pub search_url: String,
    /// Intervalle de rafraîchissement des Live Folders.
    pub live_refresh_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            archive_after_hours: 12,
            show_toolbar: true,
            search_url: "https://www.google.com/search?q=%s".into(),
            live_refresh_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedTab {
    pub url: String,
    pub title: String,
    pub archived_at: u64,
    pub space: Id,
}
