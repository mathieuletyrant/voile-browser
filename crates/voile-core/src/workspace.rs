use crate::command::{Command, Container, Effect, Location, TabUpdate};
use crate::input::resolve_input;
use crate::live::merge_live_items;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: u32 = 1;
const ARCHIVE_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub version: u32,
    next_id: Id,
    pub spaces: Vec<Space>,
    pub active_space: Id,
    /// Grosses icônes en haut de la sidebar, partagées entre espaces.
    #[serde(default)]
    pub favorites: Vec<Id>,
    pub tabs: BTreeMap<Id, Tab>,
    /// Onglet affiché, par espace.
    #[serde(default)]
    pub active_tabs: BTreeMap<Id, Id>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub archive: Vec<ArchivedTab>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("onglet {0} introuvable")]
    NoTab(Id),
    #[error("espace {0} introuvable")]
    NoSpace(Id),
    #[error("élément {0:?} introuvable")]
    NoNode(NodeRef),
    #[error("déplacement impossible : {0}")]
    BadMove(&'static str),
    #[error("dossier {0} introuvable")]
    NoLive(Id),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    /// Espace de départ : un seul, « Perso », avec les Live Folders
    /// « À relire » et « Mes PR ».
    pub fn new() -> Self {
        let mut ws = Self {
            version: FORMAT_VERSION,
            next_id: 1,
            spaces: Vec::new(),
            active_space: 0,
            favorites: Vec::new(),
            tabs: BTreeMap::new(),
            active_tabs: BTreeMap::new(),
            settings: Settings::default(),
            archive: Vec::new(),
        };
        let space = ws.add_space("Perso", "🏡", Theme::new("#f6d5c4", "#d9c6f2"));
        let live = LiveFolder {
            id: ws.id(),
            name: "À relire".into(),
            open: true,
            source: LiveSource::github_review_requested(),
            items: Vec::new(),
            seen: BTreeMap::new(),
            status: LiveStatus::default(),
        };
        let mine = LiveFolder {
            id: ws.id(),
            name: "Mes PR".into(),
            open: false,
            source: LiveSource::github_my_prs(),
            items: Vec::new(),
            seen: BTreeMap::new(),
            status: LiveStatus::default(),
        };
        ws.space_mut(space).expect("créé").pinned = vec![Node::Live(live), Node::Live(mine)];
        ws.active_space = space;
        ws
    }

    /// À appeler après désérialisation : remet l'état d'exécution à zéro.
    pub fn after_load(&mut self) {
        for tab in self.tabs.values_mut() {
            tab.reset_runtime();
        }
        for space in &mut self.spaces {
            visit_nodes_mut(&mut space.pinned, &mut |n| {
                if let Node::Live(l) = n {
                    l.status.syncing = false;
                }
            });
        }
        if self.space(self.active_space).is_err() {
            self.active_space = self.spaces.first().map_or(0, |s| s.id);
        }
    }

    /// Effets à rejouer au démarrage : afficher l'onglet actif de l'espace.
    pub fn startup_effects(&mut self) -> Vec<Effect> {
        let mut fx = vec![
            Effect::Theme(self.active_space().theme.clone()),
            Effect::LiveConfigChanged,
        ];
        match self.active_tab() {
            Some(tab) => fx.extend(self.show(tab)),
            None => fx.push(Effect::ShowEmpty),
        }
        fx
    }

    fn id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_space(&mut self, name: &str, icon: &str, theme: Theme) -> Id {
        let id = self.id();
        self.spaces.push(Space {
            id,
            name: name.into(),
            icon: icon.into(),
            theme,
            pinned: Vec::new(),
            today: Vec::new(),
            github_repos: Vec::new(),
        });
        id
    }

    // --- lecture -------------------------------------------------------------

    pub fn space(&self, id: Id) -> Result<&Space> {
        self.spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or(Error::NoSpace(id))
    }

    fn space_mut(&mut self, id: Id) -> Result<&mut Space> {
        self.spaces
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(Error::NoSpace(id))
    }

    pub fn active_space(&self) -> &Space {
        self.space(self.active_space)
            .or_else(|_| self.spaces.first().ok_or(Error::NoSpace(0)))
            .expect("au moins un espace")
    }

    pub fn active_tab(&self) -> Option<Id> {
        self.active_tabs
            .get(&self.active_space)
            .copied()
            .filter(|t| self.tabs.contains_key(t))
    }

    pub fn tab(&self, id: Id) -> Result<&Tab> {
        self.tabs.get(&id).ok_or(Error::NoTab(id))
    }

    /// Tous les Live Folders, tous espaces confondus.
    pub fn live_folders(&self) -> Vec<&LiveFolder> {
        let mut out = Vec::new();
        for space in &self.spaces {
            visit_nodes(&space.pinned, &mut |n| {
                if let Node::Live(l) = n {
                    out.push(l);
                }
            });
        }
        out
    }

    /// Chaque Live Folder avec sa source et les dépôts de son espace.
    pub fn live_sources(&self) -> Vec<(Id, LiveSource, Vec<String>)> {
        let mut out = Vec::new();
        for space in &self.spaces {
            visit_nodes(&space.pinned, &mut |n| {
                if let Node::Live(l) = n {
                    out.push((l.id, l.source.clone(), space.github_repos.clone()));
                }
            });
        }
        out
    }

    pub fn live_folder_mut(&mut self, id: Id) -> Result<&mut LiveFolder> {
        for space in &mut self.spaces {
            if let Some(Node::Live(l)) = find_node_mut(&mut space.pinned, NodeRef::Live(id)) {
                return Ok(l);
            }
        }
        Err(Error::NoLive(id))
    }

    /// Espace qui contient l'onglet (favoris : l'espace actif).
    pub fn space_of_tab(&self, tab: Id) -> Option<Id> {
        if self.favorites.contains(&tab) {
            return Some(self.active_space);
        }
        self.spaces
            .iter()
            .find(|s| s.today.contains(&tab) || contains_tab(&s.pinned, tab))
            .map(|s| s.id)
    }

    /// Onglets de l'espace actif dans l'ordre de la sidebar : favoris,
    /// épinglés (dossiers compris), puis Today. Sert à Ctrl+Tab et Cmd+1…9.
    pub fn tab_order(&self) -> Vec<Id> {
        let mut out = self.favorites.clone();
        let space = self.active_space();
        visit_nodes(&space.pinned, &mut |n| {
            if let Node::Tab { tab } = n {
                out.push(*tab);
            }
        });
        out.extend(space.today.iter().copied());
        out
    }

    pub fn is_pinned(&self, tab: Id) -> bool {
        self.favorites.contains(&tab) || self.spaces.iter().any(|s| contains_tab(&s.pinned, tab))
    }

    // --- écriture --------------------------------------------------------------

    /// Applique une commande et renvoie les effets à exécuter par le shell.
    pub fn apply(&mut self, cmd: Command, now: u64) -> Result<Vec<Effect>> {
        use Command::*;
        let fx = match cmd {
            Open { input, new_tab } => {
                let url = resolve_input(&input, &self.settings.search_url);
                match self.active_tab().filter(|_| !new_tab) {
                    Some(tab) => {
                        let t = self.tabs.get_mut(&tab).ok_or(Error::NoTab(tab))?;
                        t.url = url.clone();
                        vec![Effect::LoadUrl { tab, url }]
                    }
                    None => self.open_tab(url, None, now),
                }
            }
            PopupRequested { from: _, url } => self.open_tab(url, None, now),
            Activate { tab } => self.activate(tab, now)?,
            Close { tab } => self.close(tab, now)?,
            Back { tab } => vec![Effect::GoBack { tab }],
            Forward { tab } => vec![Effect::GoForward { tab }],
            Reload { tab } => vec![Effect::Reload { tab }],
            Pin { tab } => {
                let url = self.tab(tab)?.url.clone();
                let space = self.space_of_tab(tab).ok_or(Error::NoTab(tab))?;
                self.detach_tab(tab);
                let s = self.space_mut(space)?;
                s.pinned.push(Node::Tab { tab });
                self.tabs.get_mut(&tab).expect("vérifié").pinned_url = Some(url);
                vec![]
            }
            Unpin { tab } => {
                let space = self.space_of_tab(tab).ok_or(Error::NoTab(tab))?;
                self.detach_tab(tab);
                self.space_mut(space)?.today.insert(0, tab);
                self.tabs.get_mut(&tab).expect("vérifié").pinned_url = None;
                vec![]
            }
            Favorite { tab } => {
                self.tab(tab)?;
                let url = self.tabs[&tab].url.clone();
                self.detach_tab(tab);
                self.favorites.push(tab);
                self.tabs.get_mut(&tab).expect("vérifié").pinned_url = Some(url);
                vec![]
            }
            Unfavorite { tab } => {
                self.tab(tab)?;
                self.detach_tab(tab);
                let space = self.active_space;
                self.space_mut(space)?.today.insert(0, tab);
                self.tabs.get_mut(&tab).expect("vérifié").pinned_url = None;
                vec![]
            }
            ResetPinned { tab } => {
                let t = self.tabs.get_mut(&tab).ok_or(Error::NoTab(tab))?;
                match t.pinned_url.clone() {
                    Some(url) if t.loaded => {
                        t.url = url.clone();
                        vec![Effect::LoadUrl { tab, url }]
                    }
                    Some(url) => {
                        t.url = url;
                        vec![]
                    }
                    None => vec![],
                }
            }
            ClearToday => {
                let space = self.active_space;
                let tabs = std::mem::take(&mut self.space_mut(space)?.today);
                let mut fx = Vec::new();
                for tab in tabs {
                    fx.extend(self.remove_tab(tab, now));
                }
                fx.extend(self.after_active_removed(now));
                fx
            }
            CreateFolder { name } => {
                let id = self.id();
                let space = self.active_space;
                self.space_mut(space)?.pinned.push(Node::Folder(Folder {
                    id,
                    name,
                    open: true,
                    children: Vec::new(),
                }));
                vec![]
            }
            Rename { node, name } => {
                let n = self.node_mut(node)?;
                match n {
                    Node::Folder(f) => f.name = name,
                    Node::Live(l) => l.name = name,
                    Node::Tab { tab } => {
                        let tab = *tab;
                        self.tabs.get_mut(&tab).ok_or(Error::NoTab(tab))?.title = name;
                    }
                }
                vec![]
            }
            ToggleOpen { node } => {
                match self.node_mut(node)? {
                    Node::Folder(f) => f.open = !f.open,
                    Node::Live(l) => l.open = !l.open,
                    Node::Tab { .. } => {}
                }
                vec![]
            }
            Delete { node } => self.delete(node, now)?,
            Move { node, to } => {
                self.move_node(node, to)?;
                vec![]
            }
            SwitchSpace { space } => {
                self.space(space)?;
                self.active_space = space;
                let mut fx = vec![Effect::Theme(self.active_space().theme.clone())];
                match self.active_tab() {
                    Some(tab) => fx.extend(self.show(tab)),
                    None => fx.push(Effect::ShowEmpty),
                }
                fx
            }
            CreateSpace { name, icon, theme } => {
                let id = self.add_space(&name, &icon, theme);
                self.apply(SwitchSpace { space: id }, now)?
            }
            UpdateSpace {
                space,
                name,
                icon,
                theme,
            } => {
                let active = self.active_space == space;
                let s = self.space_mut(space)?;
                let mut fx = Vec::new();
                if let Some(n) = name {
                    s.name = n;
                }
                if let Some(i) = icon {
                    s.icon = i;
                }
                if let Some(t) = theme {
                    s.theme = t.clone();
                    if active {
                        fx.push(Effect::Theme(t));
                    }
                }
                fx
            }
            SetSpaceRepos { space, repos } => {
                let space = space.unwrap_or(self.active_space);
                self.space_mut(space)?.github_repos = clean_repos(repos);
                vec![Effect::LiveConfigChanged]
            }
            AddLiveFolder { name, source } => {
                let id = self.id();
                let space = self.active_space;
                self.space_mut(space)?.pinned.push(Node::Live(LiveFolder {
                    id,
                    name,
                    open: true,
                    source,
                    items: Vec::new(),
                    seen: BTreeMap::new(),
                    status: LiveStatus::default(),
                }));
                vec![
                    Effect::LiveConfigChanged,
                    Effect::LiveRefresh { folder: id },
                ]
            }
            RefreshLive { folder } => {
                self.live_folder_mut(folder)?;
                vec![Effect::LiveRefresh { folder }]
            }
            OpenLiveItem { folder, key } => {
                let live = self.live_folder_mut(folder)?;
                let item = live
                    .items
                    .iter_mut()
                    .find(|i| i.key == key)
                    .ok_or(Error::NoLive(folder))?;
                item.unread = false;
                live.seen.insert(item.key.clone(), item.updated_at.clone());
                let url = item.url.clone();
                let origin = LiveOrigin { folder, key };
                // Réutilise l'onglet déjà ouvert pour cette PR.
                if let Some(tab) = self
                    .tabs
                    .values()
                    .find(|t| t.origin.as_ref() == Some(&origin))
                    .map(|t| t.id)
                {
                    self.activate(tab, now)?
                } else {
                    self.open_tab(url, Some(origin), now)
                }
            }
            MarkLiveRead { folder } => {
                let live = self.live_folder_mut(folder)?;
                for item in &mut live.items {
                    item.unread = false;
                    live.seen.insert(item.key.clone(), item.updated_at.clone());
                }
                vec![]
            }
            SetShowToolbar { show } => {
                self.settings.show_toolbar = show;
                vec![Effect::ToolbarChanged]
            }
            TabUpdated { tab, update } => {
                self.update_tab(tab, update);
                vec![]
            }
            LiveSyncStarted { folder } => {
                self.live_folder_mut(folder)?.status.syncing = true;
                vec![]
            }
            LiveResult { folder, result } => {
                let live = self.live_folder_mut(folder)?;
                live.status.syncing = false;
                match result {
                    Ok(items) => {
                        live.items = merge_live_items(items, &live.seen);
                        live.status.error = None;
                        live.status.last_sync = Some(now);
                    }
                    Err(e) => live.status.error = Some(e),
                }
                vec![]
            }
            Tick { now } => self.archive_stale(now),
        };
        Ok(fx)
    }

    fn open_tab(&mut self, url: String, origin: Option<LiveOrigin>, now: u64) -> Vec<Effect> {
        let id = self.id();
        let mut tab = Tab::new(id, url.clone());
        tab.origin = origin;
        tab.last_active = now;
        tab.loaded = true;
        self.tabs.insert(id, tab);
        let space = self.active_space;
        if let Ok(s) = self.space_mut(space) {
            s.today.insert(0, id);
        }
        self.active_tabs.insert(space, id);
        vec![
            Effect::CreateView { tab: id, url },
            Effect::ShowView { tab: id },
        ]
    }

    fn activate(&mut self, tab: Id, now: u64) -> Result<Vec<Effect>> {
        self.tab(tab)?;
        let mut fx = Vec::new();
        // Activer un onglet d'un autre espace bascule d'espace.
        if let Some(space) = self.space_of_tab(tab) {
            if space != self.active_space {
                self.active_space = space;
                fx.push(Effect::Theme(self.active_space().theme.clone()));
            }
        }
        self.active_tabs.insert(self.active_space, tab);
        self.tabs.get_mut(&tab).expect("vérifié").last_active = now;
        fx.extend(self.show(tab));
        Ok(fx)
    }

    /// Effets pour afficher un onglet, en créant sa vue si besoin.
    fn show(&mut self, tab: Id) -> Vec<Effect> {
        let Some(t) = self.tabs.get_mut(&tab) else {
            return vec![Effect::ShowEmpty];
        };
        let mut fx = Vec::new();
        if !t.loaded {
            t.loaded = true;
            fx.push(Effect::CreateView {
                tab,
                url: t.url.clone(),
            });
        }
        fx.push(Effect::ShowView { tab });
        fx
    }

    /// Fermer : un onglet épinglé est seulement déchargé, les autres sont
    /// supprimés (et vont dans l'archive).
    fn close(&mut self, tab: Id, now: u64) -> Result<Vec<Effect>> {
        let pinned = self.is_pinned(tab);
        let t = self.tabs.get_mut(&tab).ok_or(Error::NoTab(tab))?;
        let mut fx = Vec::new();
        if pinned {
            if t.loaded {
                fx.push(Effect::DestroyView { tab });
            }
            t.reset_runtime();
            if let Some(url) = t.pinned_url.clone() {
                t.url = url;
            }
            if self.active_tab() == Some(tab) {
                self.active_tabs.remove(&self.active_space);
            }
        } else {
            self.detach_tab(tab);
            fx.extend(self.remove_tab(tab, now));
        }
        fx.extend(self.after_active_removed(now));
        Ok(fx)
    }

    /// Si l'espace n'a plus d'onglet actif, active le plus récent du Today.
    fn after_active_removed(&mut self, now: u64) -> Vec<Effect> {
        if self.active_tab().is_some() {
            return vec![];
        }
        self.active_tabs.remove(&self.active_space);
        let next = self.active_space().today.first().copied();
        match next {
            Some(tab) => self.activate(tab, now).unwrap_or_default(),
            None => vec![Effect::ShowEmpty],
        }
    }

    /// Supprime l'onglet du registre (doit déjà être détaché de l'arbre).
    fn remove_tab(&mut self, tab: Id, now: u64) -> Vec<Effect> {
        let space = self.active_space;
        let Some(t) = self.tabs.remove(&tab) else {
            return vec![];
        };
        self.active_tabs.retain(|_, v| *v != tab);
        if !t.url.starts_with("about:") {
            self.archive.insert(
                0,
                ArchivedTab {
                    url: t.url,
                    title: t.title,
                    archived_at: now,
                    space,
                },
            );
            self.archive.truncate(ARCHIVE_LIMIT);
        }
        if t.loaded {
            vec![Effect::DestroyView { tab }]
        } else {
            vec![]
        }
    }

    /// Retire l'onglet de partout (favoris, épinglés, Today) sans le supprimer.
    fn detach_tab(&mut self, tab: Id) {
        self.favorites.retain(|t| *t != tab);
        for s in &mut self.spaces {
            s.today.retain(|t| *t != tab);
            take_node(&mut s.pinned, NodeRef::Tab(tab));
        }
    }

    fn update_tab(&mut self, tab: Id, u: TabUpdate) {
        let Some(t) = self.tabs.get_mut(&tab) else {
            return;
        };
        if let Some(url) = u.url {
            t.url = url;
        }
        if let Some(title) = u.title.filter(|s| !s.is_empty()) {
            t.title = title;
        }
        if let Some(f) = u.favicon {
            t.favicon = Some(f);
        }
        if let Some(l) = u.loading {
            t.loading = l;
        }
        if let Some(b) = u.can_go_back {
            t.can_go_back = b;
        }
        if let Some(f) = u.can_go_forward {
            t.can_go_forward = f;
        }
    }

    fn node_mut(&mut self, node: NodeRef) -> Result<&mut Node> {
        // Deux passes pour satisfaire le borrow checker.
        let idx = self
            .spaces
            .iter_mut()
            .position(|s| find_node_mut(&mut s.pinned, node).is_some())
            .ok_or(Error::NoNode(node))?;
        Ok(find_node_mut(&mut self.spaces[idx].pinned, node).expect("trouvé"))
    }

    fn delete(&mut self, node: NodeRef, now: u64) -> Result<Vec<Effect>> {
        if let NodeRef::Tab(tab) = node {
            self.tab(tab)?;
            self.detach_tab(tab);
            let mut fx = self.remove_tab(tab, now);
            fx.extend(self.after_active_removed(now));
            return Ok(fx);
        }
        let mut removed = None;
        for s in &mut self.spaces {
            if let Some(n) = take_node(&mut s.pinned, node) {
                removed = Some(n);
                break;
            }
        }
        let removed = removed.ok_or(Error::NoNode(node))?;
        let mut tabs = Vec::new();
        visit_nodes(std::slice::from_ref(&removed), &mut |n| {
            if let Node::Tab { tab } = n {
                tabs.push(*tab);
            }
        });
        let mut fx = Vec::new();
        for tab in tabs {
            fx.extend(self.remove_tab(tab, now));
        }
        if matches!(removed, Node::Live(_)) {
            fx.push(Effect::LiveConfigChanged);
        }
        fx.extend(self.after_active_removed(now));
        Ok(fx)
    }

    fn move_node(&mut self, node: NodeRef, to: Location) -> Result<()> {
        self.space(to.space)?;
        let is_tab = matches!(node, NodeRef::Tab(_));
        if matches!(to.container, Container::Today | Container::Favorites) && !is_tab {
            return Err(Error::BadMove(
                "seuls les onglets vont dans Today ou les favoris",
            ));
        }
        if let (NodeRef::Folder(moving), Container::Folder(target)) = (node, to.container) {
            if moving == target || self.folder_contains(moving, NodeRef::Folder(target)) {
                return Err(Error::BadMove("un dossier ne peut pas aller dans lui-même"));
            }
        }
        if let Container::Folder(target) = to.container {
            match self.node_mut(NodeRef::Folder(target)) {
                Ok(Node::Folder(_)) => {}
                _ => return Err(Error::NoNode(NodeRef::Folder(target))),
            }
        }

        // Extraction.
        let extracted: Node = match node {
            NodeRef::Tab(tab) => {
                self.tab(tab)?;
                self.detach_tab(tab);
                Node::Tab { tab }
            }
            other => {
                let mut found = None;
                for s in &mut self.spaces {
                    if let Some(n) = take_node(&mut s.pinned, other) {
                        found = Some(n);
                        break;
                    }
                }
                found.ok_or(Error::NoNode(other))?
            }
        };

        // Insertion.
        let tab_id = match extracted {
            Node::Tab { tab } => Some(tab),
            _ => None,
        };
        match to.container {
            Container::Favorites => {
                let tab = tab_id.expect("vérifié");
                let i = to.index.min(self.favorites.len());
                self.favorites.insert(i, tab);
                self.set_pinned_url(tab, true);
            }
            Container::Today => {
                let tab = tab_id.expect("vérifié");
                let s = self.space_mut(to.space)?;
                let i = to.index.min(s.today.len());
                s.today.insert(i, tab);
                self.set_pinned_url(tab, false);
            }
            Container::Pinned => {
                let s = self.space_mut(to.space)?;
                let i = to.index.min(s.pinned.len());
                s.pinned.insert(i, extracted);
                if let Some(tab) = tab_id {
                    self.set_pinned_url(tab, true);
                }
            }
            Container::Folder(target) => {
                let Node::Folder(f) = self.node_mut(NodeRef::Folder(target))? else {
                    unreachable!("vérifié plus haut")
                };
                let i = to.index.min(f.children.len());
                f.children.insert(i, extracted);
                if let Some(tab) = tab_id {
                    self.set_pinned_url(tab, true);
                }
            }
        }
        Ok(())
    }

    fn set_pinned_url(&mut self, tab: Id, pinned: bool) {
        if let Some(t) = self.tabs.get_mut(&tab) {
            if pinned {
                if t.pinned_url.is_none() {
                    t.pinned_url = Some(t.url.clone());
                }
            } else {
                t.pinned_url = None;
            }
        }
    }

    fn folder_contains(&self, folder: Id, needle: NodeRef) -> bool {
        for s in &self.spaces {
            let mut hit = false;
            visit_nodes(&s.pinned, &mut |n| {
                if let Node::Folder(f) = n {
                    if f.id == folder {
                        let mut inner = false;
                        visit_nodes(&f.children, &mut |c| inner |= c.node_ref() == needle);
                        hit |= inner;
                    }
                }
            });
            if hit {
                return true;
            }
        }
        false
    }

    /// Archive les onglets « Today » inactifs depuis trop longtemps.
    fn archive_stale(&mut self, now: u64) -> Vec<Effect> {
        let max_age = u64::from(self.settings.archive_after_hours) * 3600;
        if max_age == 0 {
            return vec![];
        }
        let active: Vec<Id> = self.active_tabs.values().copied().collect();
        let stale: Vec<Id> = self
            .spaces
            .iter()
            .flat_map(|s| s.today.iter().copied())
            .filter(|t| !active.contains(t))
            .filter(|t| {
                self.tabs
                    .get(t)
                    .is_some_and(|tab| now.saturating_sub(tab.last_active) > max_age)
            })
            .collect();
        let mut fx = Vec::new();
        for tab in stale {
            self.detach_tab(tab);
            fx.extend(self.remove_tab(tab, now));
        }
        fx
    }
}

// --- parcours d'arbre ---------------------------------------------------------

pub(crate) fn visit_nodes<'a>(nodes: &'a [Node], f: &mut dyn FnMut(&'a Node)) {
    for n in nodes {
        f(n);
        if let Node::Folder(folder) = n {
            visit_nodes(&folder.children, f);
        }
    }
}

fn visit_nodes_mut(nodes: &mut [Node], f: &mut dyn FnMut(&mut Node)) {
    for n in nodes {
        f(n);
        if let Node::Folder(folder) = n {
            visit_nodes_mut(&mut folder.children, f);
        }
    }
}

fn contains_tab(nodes: &[Node], tab: Id) -> bool {
    let mut hit = false;
    visit_nodes(nodes, &mut |n| {
        hit |= matches!(n, Node::Tab { tab: t } if *t == tab)
    });
    hit
}

fn find_node_mut(nodes: &mut [Node], target: NodeRef) -> Option<&mut Node> {
    for n in nodes {
        if n.node_ref() == target {
            return Some(n);
        }
        if let Node::Folder(f) = n {
            if let Some(found) = find_node_mut(&mut f.children, target) {
                return Some(found);
            }
        }
    }
    None
}

fn take_node(nodes: &mut Vec<Node>, target: NodeRef) -> Option<Node> {
    if let Some(i) = nodes.iter().position(|n| n.node_ref() == target) {
        return Some(nodes.remove(i));
    }
    for n in nodes.iter_mut() {
        if let Node::Folder(f) = n {
            if let Some(found) = take_node(&mut f.children, target) {
                return Some(found);
            }
        }
    }
    None
}

/// Entrées nettoyées et sans doublon. Leur forme est vérifiée par qui s'en
/// sert (voile-live, en construisant la recherche).
fn clean_repos(repos: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for repo in repos {
        let repo = repo.trim().to_string();
        if !repo.is_empty() && !out.contains(&repo) {
            out.push(repo);
        }
    }
    out
}
