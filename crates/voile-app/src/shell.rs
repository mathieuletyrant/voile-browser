//! Le shell : possède le [`Workspace`], applique les commandes et traduit les
//! [`Effect`] en opérations CEF (cadres, navigation, synchro des Live Folders).
//! Vit dans un `thread_local` du thread UI ; on n'y accède que via
//! [`handle`] et [`flush`], appelés par [`crate::bridge`].

use crate::bridge::{self, Event};
use crate::frame::{FrameId, FrameKey, FrameSpec, FrameWindow, SIDEBAR_MAX, SIDEBAR_MIN};
use crate::paths::Dirs;
use cef::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use voile_core::view::{ui_state, ViewContext};
use voile_core::{Command, Effect, Id, Store, Workspace};
use voile_live::{LiveService, LiveSink};

const SAVE_DELAY_MS: i64 = 800;
const TICK_MS: i64 = 60_000;
/// Token GitHub des Live Folders, à part du workspace : il n'est jamais
/// envoyé à la sidebar.
const GITHUB_TOKEN_FILE: &str = "github-token";

/// Écrit un fichier lisible par l'utilisateur seul.
fn write_private(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(content.as_bytes())
}

/// Actions de l'interface qui ne touchent pas au modèle (fenêtre, focus…).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UiAction {
    /// La sidebar d'un cadre a chargé son script.
    Ready {
        frame: String,
    },
    FocusCommandBar {
        new_tab: bool,
    },
    FocusPage,
    ToggleSidebar,
    SetSidebarWidth {
        width: i32,
    },
    CycleTab {
        delta: i32,
    },
    SelectIndex {
        index: usize,
    },
    ReopenClosed,
    /// Ferme l'onglet actif (un onglet épinglé est seulement déchargé).
    CloseTab,
    CopyUrl,
    TogglePin,
    Minimize,
    ToggleMaximize,
    /// Ramène la fenêtre au premier plan (relance de l'app, clic sur le Dock).
    Raise,
    Quit,
    /// Enregistre le token GitHub des Live Folders (vide : l'efface).
    SetGithubToken {
        token: String,
    },
    DevTools {
        sidebar: bool,
    },
}

/// Géométrie de la fenêtre, gardée à part du workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Geometry {
    bounds: Option<[i32; 4]>,
    maximized: bool,
    sidebar_width: i32,
    sidebar_visible: bool,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            bounds: None,
            maximized: false,
            sidebar_width: 260,
            sidebar_visible: true,
        }
    }
}

pub struct Shell {
    ws: Workspace,
    store: Store,
    live: LiveService,
    dirs: Dirs,
    frames: HashMap<FrameKey, FrameWindow>,
    visible: Option<FrameKey>,
    /// Fenêtres créées et pas encore détruites (anciennes générations comprises).
    open_windows: usize,
    gen: u32,
    geom: Geometry,
    last_bounds: Option<Rect>,
    sidebar_width: Arc<AtomicI32>,
    /// Cadres à fermer une fois tous les effets d'une commande appliqués
    /// (on montre le suivant avant de fermer l'ancien : pas de flash).
    retire: Vec<FrameKey>,
    dirty: bool,
    save_scheduled: bool,
    quitting: bool,
    toast: Option<String>,
    /// Un token GitHub est enregistré (`github-token` dans le dossier de données).
    github_token: bool,
}

thread_local! {
    static SHELL: RefCell<Option<Shell>> = const { RefCell::new(None) };
}

/// Crée le shell (appelé dans `on_context_initialized`).
pub fn start(dirs: Dirs) {
    let shell = Shell::new(dirs);
    SHELL.with(|cell| *cell.borrow_mut() = Some(shell));
    SHELL.with(|cell| {
        if let Some(shell) = cell.borrow_mut().as_mut() {
            shell.boot();
        }
    });
    flush();
}

/// Traite un événement. Rend l'événement si le shell est déjà emprunté.
pub fn handle(event: Event) -> Result<(), Event> {
    SHELL.with(|cell| match cell.try_borrow_mut() {
        Ok(mut guard) => {
            if let Some(shell) = guard.as_mut() {
                shell.handle(event);
            }
            Ok(())
        }
        Err(_) => Err(event),
    })
}

/// Pousse l'état vers la sidebar visible si quelque chose a changé.
pub fn flush() {
    SHELL.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            if let Some(shell) = guard.as_mut() {
                shell.flush();
            }
        }
    });
}

/// Enregistre tout et libère les objets CEF avant `cef::shutdown()`.
pub fn shutdown() {
    let shell = SHELL.with(|cell| cell.borrow_mut().take());
    if let Some(mut shell) = shell {
        shell.save();
    }
}

struct Sink;

impl LiveSink for Sink {
    fn started(&self, folder: Id) {
        bridge::send(Event::Internal(Command::LiveSyncStarted { folder }));
    }
    fn finished(&self, folder: Id, result: Result<Vec<voile_core::LiveItem>, String>) {
        bridge::send(Event::Internal(Command::LiveResult { folder, result }));
    }
}

impl Shell {
    fn new(dirs: Dirs) -> Self {
        let store = Store::new(&dirs.data);
        let ws = store.load().unwrap_or_else(|e| {
            tracing::error!("{e} : workspace neuf (l'ancien fichier n'est pas modifié)");
            Workspace::new()
        });
        let geom: Geometry = std::fs::read(dirs.data.join("window.json"))
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        let sidebar_width = Arc::new(AtomicI32::new(
            geom.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX),
        ));
        // Le token avant la configuration des dossiers : pas de premier essai à vide.
        let live = LiveService::spawn(Sink);
        let token = std::fs::read_to_string(dirs.data.join(GITHUB_TOKEN_FILE))
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let github_token = token.is_some();
        live.set_token(token);
        Self {
            ws,
            store,
            live,
            dirs,
            frames: HashMap::new(),
            visible: None,
            open_windows: 0,
            gen: 0,
            geom,
            last_bounds: None,
            sidebar_width,
            retire: Vec::new(),
            dirty: true,
            save_scheduled: false,
            quitting: false,
            toast: None,
            github_token,
        }
    }

    fn boot(&mut self) {
        let fx = self.ws.startup_effects();
        self.run(fx);
        bridge::send_later(TICK_MS, Event::Tick);
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Ui(cmd) => self.apply(cmd, true),
            Event::Internal(cmd) => self.apply(cmd, false),
            Event::Action(action) => self.action(action),
            Event::SidebarReady(id) => {
                if let Some(frame) = self.frame_mut(id) {
                    frame.ready = true;
                    frame.stale = true;
                }
            }
            Event::WindowClosing(id) => {
                if self.frame_mut(id).is_some() {
                    self.quit();
                }
            }
            Event::WindowDestroyed(id) => {
                self.open_windows = self.open_windows.saturating_sub(1);
                if self.frames.get(&id.key).is_some_and(|f| f.id == id) {
                    self.frames.remove(&id.key);
                    if self.visible == Some(id.key) {
                        self.visible = None;
                    }
                }
                self.exit_when_closed();
            }
            Event::ExtraClosed => self.exit_when_closed(),
            Event::Save => {
                self.save_scheduled = false;
                self.save();
            }
            Event::Tick => {
                self.apply(
                    Command::Tick {
                        now: voile_core::now(),
                    },
                    false,
                );
                bridge::send_later(TICK_MS, Event::Tick);
            }
        }
    }

    fn frame_mut(&mut self, id: FrameId) -> Option<&mut FrameWindow> {
        self.frames.get_mut(&id.key).filter(|f| f.id == id)
    }

    // --- modèle --------------------------------------------------------------

    fn apply(&mut self, cmd: Command, from_ui: bool) {
        if self.quitting {
            return;
        }
        let persist = !matches!(cmd, Command::LiveSyncStarted { .. });
        match self.ws.apply(cmd, voile_core::now()) {
            Ok(fx) => self.run(fx),
            Err(e) if from_ui => self.toast = Some(e.to_string()),
            Err(e) => tracing::debug!("commande interne ignorée : {e}"),
        }
        self.dirty = true;
        if persist {
            self.schedule_save();
        }
    }

    fn run(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::CreateView { tab, url } => self.create_frame(FrameKey::Tab(tab), url),
                Effect::ShowView { tab } => self.show(FrameKey::Tab(tab)),
                Effect::ShowEmpty => self.show(FrameKey::Home),
                Effect::DestroyView { tab } => self.retire.push(FrameKey::Tab(tab)),
                Effect::LoadUrl { tab, url } => {
                    if let Some(frame) = self.page(tab).and_then(|b| b.main_frame()) {
                        frame.load_url(Some(&CefString::from(url.as_str())));
                    }
                }
                Effect::GoBack { tab } => {
                    if let Some(b) = self.page(tab) {
                        b.go_back();
                    }
                }
                Effect::GoForward { tab } => {
                    if let Some(b) = self.page(tab) {
                        b.go_forward();
                    }
                }
                Effect::Reload { tab } => {
                    if let Some(b) = self.page(tab) {
                        b.reload();
                    }
                }
                Effect::LiveConfigChanged => self.configure_live(),
                Effect::LiveRefresh { folder } => self.live.refresh(folder),
                Effect::ToolbarChanged => self.rebuild_tab_frames(),
                // La sidebar lit le thème dans l'état poussé par flush().
                Effect::Theme(_) => {}
            }
        }
        for key in std::mem::take(&mut self.retire) {
            if self.visible == Some(key) {
                self.visible = None;
            }
            if let Some(frame) = self.frames.remove(&key) {
                frame.close();
            }
        }
    }

    fn page(&self, tab: Id) -> Option<Browser> {
        self.frames.get(&FrameKey::Tab(tab))?.page_browser()
    }

    fn configure_live(&self) {
        let folders = self
            .ws
            .live_sources()
            .into_iter()
            .map(|(id, source, repos)| (id, voile_live::scoped(&source, &repos)))
            .collect();
        let every = Duration::from_secs(self.ws.settings.live_refresh_secs);
        self.live.configure(folders, every);
    }

    // --- cadres --------------------------------------------------------------

    fn create_frame(&mut self, key: FrameKey, url: String) {
        self.gen += 1;
        let spec = FrameSpec {
            id: FrameId { key, gen: self.gen },
            url,
            show_toolbar: self.ws.settings.show_toolbar,
            bounds: self.current_bounds(),
            sidebar_width: self.sidebar_width.clone(),
            sidebar_visible: self.geom.sidebar_visible,
        };
        match FrameWindow::create(spec) {
            Some(frame) => {
                self.open_windows += 1;
                #[cfg(target_os = "macos")]
                crate::mac::disable_window_animations();
                if let Some(old) = self.frames.insert(key, frame) {
                    old.close();
                }
            }
            None => {
                tracing::error!("création de la fenêtre {key:?} impossible");
                self.toast = Some("Impossible d'ouvrir l'onglet".into());
            }
        }
    }

    /// Montre le cadre `key` à la place du cadre visible, même position, même
    /// taille, même état (maximisé, plein écran).
    fn show(&mut self, key: FrameKey) {
        if !self.frames.contains_key(&key) {
            let url = match key {
                FrameKey::Tab(id) => match self.ws.tab(id) {
                    Ok(tab) => tab.url.clone(),
                    Err(_) => return,
                },
                FrameKey::Home => String::new(),
            };
            self.create_frame(key, url);
        }
        if self.visible == Some(key) {
            if let Some(frame) = self.frames.get(&key) {
                frame.window.show();
                frame.window.activate();
            }
            return;
        }

        let prev = self.visible.and_then(|k| self.frames.get(&k));
        let (maximized, fullscreen) = match prev {
            Some(p) => (p.window.is_maximized() != 0, p.window.is_fullscreen() != 0),
            None => (self.geom.maximized, false),
        };
        if let Some(p) = prev {
            if !maximized && !fullscreen {
                self.last_bounds = Some(p.window.bounds());
            }
        }
        let bounds = self.current_bounds();
        let state = self.state_js();

        let Some(frame) = self.frames.get_mut(&key) else {
            return;
        };
        if frame.ready && frame.stale {
            frame.eval_sidebar(&state);
            frame.stale = false;
        }
        if !maximized && !fullscreen {
            frame.window.set_bounds(Some(&bounds));
        }
        frame.window.show();
        if maximized {
            frame.window.maximize();
        }
        if fullscreen {
            frame.window.set_fullscreen(1);
        }
        frame.window.activate();
        match key {
            FrameKey::Tab(_) => frame.page.request_focus(),
            FrameKey::Home => frame.sidebar.request_focus(),
        }

        if let Some(prev) = self.visible.replace(key).and_then(|k| self.frames.get(&k)) {
            prev.window.hide();
        }
    }

    /// Le réglage de barre d'outils est figé à la création d'une BrowserView :
    /// on recrée les cadres des onglets chargés.
    fn rebuild_tab_frames(&mut self) {
        if let Some(p) = self.visible.and_then(|k| self.frames.get(&k)) {
            self.last_bounds = Some(p.window.bounds());
        }
        let visible = self.visible.take();
        let keys: Vec<FrameKey> = self
            .frames
            .keys()
            .copied()
            .filter(|k| matches!(k, FrameKey::Tab(_)))
            .collect();
        for key in keys {
            let FrameKey::Tab(id) = key else { continue };
            let url = self.ws.tab(id).map(|t| t.url.clone()).unwrap_or_default();
            self.create_frame(key, url);
        }
        if let Some(key) = visible {
            self.show(key);
        }
    }

    fn current_bounds(&self) -> Rect {
        if let Some(frame) = self.visible.and_then(|k| self.frames.get(&k)) {
            if frame.window.is_maximized() == 0 && frame.window.is_fullscreen() == 0 {
                return frame.window.bounds();
            }
        }
        if let Some(b) = &self.last_bounds {
            return b.clone();
        }
        if let Some([x, y, width, height]) = self.geom.bounds {
            return Rect {
                x,
                y,
                width,
                height,
            };
        }
        let area = display_get_primary()
            .map(|d| d.work_area())
            .unwrap_or(Rect {
                x: 0,
                y: 0,
                width: 1440,
                height: 900,
            });
        let width = 1280.min(area.width - 80).max(640);
        let height = 840.min(area.height - 80).max(420);
        Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        }
    }

    fn visible_frame(&self) -> Option<&FrameWindow> {
        self.visible.and_then(|k| self.frames.get(&k))
    }

    // --- actions d'interface ---------------------------------------------------

    fn action(&mut self, action: UiAction) {
        match action {
            UiAction::Ready { .. } => {}
            UiAction::FocusCommandBar { new_tab } => {
                if !self.geom.sidebar_visible {
                    self.set_sidebar_visible(true);
                }
                if let Some(frame) = self.visible_frame() {
                    frame.sidebar.request_focus();
                    frame.eval_sidebar(&format!("window.voile&&voile.focusCommand({new_tab})"));
                }
            }
            UiAction::FocusPage => {
                if let Some(frame) = self.visible_frame() {
                    frame.page.request_focus();
                }
            }
            UiAction::ToggleSidebar => self.set_sidebar_visible(!self.geom.sidebar_visible),
            UiAction::SetSidebarWidth { width } => {
                let width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
                self.sidebar_width.store(width, Ordering::Relaxed);
                self.geom.sidebar_width = width;
                for frame in self.frames.values() {
                    frame.relayout();
                }
                self.schedule_save();
            }
            UiAction::CycleTab { delta } => {
                let order = self.ws.tab_order();
                if order.is_empty() {
                    return;
                }
                let len = order.len() as i64;
                let pos = self
                    .ws
                    .active_tab()
                    .and_then(|t| order.iter().position(|o| *o == t))
                    .map_or(if delta > 0 { -1 } else { 0 }, |p| p as i64);
                let next = (pos + i64::from(delta)).rem_euclid(len) as usize;
                self.apply(Command::Activate { tab: order[next] }, true);
            }
            UiAction::SelectIndex { index } => {
                let order = self.ws.tab_order();
                if let Some(tab) = order.get(index.min(order.len().saturating_sub(1))) {
                    self.apply(Command::Activate { tab: *tab }, true);
                }
            }
            UiAction::ReopenClosed => {
                if !self.ws.archive.is_empty() {
                    let last = self.ws.archive.remove(0);
                    self.apply(
                        Command::Open {
                            input: last.url,
                            new_tab: true,
                        },
                        true,
                    );
                }
            }
            UiAction::CopyUrl => {
                let url = self
                    .ws
                    .active_tab()
                    .and_then(|t| self.ws.tab(t).ok())
                    .map(|t| t.url.clone());
                if let (Some(url), Some(frame)) = (url, self.visible_frame()) {
                    let url = serde_json::to_string(&url).unwrap_or_default();
                    frame.eval_sidebar(&format!("window.voile&&voile.copy({url})"));
                }
            }
            UiAction::CloseTab => {
                if let Some(tab) = self.ws.active_tab() {
                    self.apply(Command::Close { tab }, true);
                }
            }
            UiAction::TogglePin => {
                if let Some(tab) = self.ws.active_tab() {
                    let cmd = if self.ws.is_pinned(tab) {
                        Command::Unpin { tab }
                    } else {
                        Command::Pin { tab }
                    };
                    self.apply(cmd, true);
                }
            }
            UiAction::Minimize => {
                if let Some(frame) = self.visible_frame() {
                    frame.window.minimize();
                }
            }
            UiAction::ToggleMaximize => {
                if let Some(frame) = self.visible_frame() {
                    if frame.window.is_maximized() != 0 {
                        frame.window.restore();
                    } else {
                        frame.window.maximize();
                    }
                }
            }
            UiAction::Raise => {
                if let Some(frame) = self.visible_frame() {
                    frame.window.show();
                    frame.window.activate();
                }
            }
            UiAction::Quit => self.quit(),
            UiAction::SetGithubToken { token } => self.set_github_token(token.trim()),
            UiAction::DevTools { sidebar } => {
                if let Some(frame) = self.visible_frame() {
                    let view = if sidebar { &frame.sidebar } else { &frame.page };
                    if let Some(host) = view.browser().and_then(|b| b.host()) {
                        host.show_dev_tools(None, None, None, None);
                    }
                }
            }
        }
        self.dirty = true;
    }

    fn set_sidebar_visible(&mut self, visible: bool) {
        self.geom.sidebar_visible = visible;
        for frame in self.frames.values() {
            frame.set_sidebar_visible(visible);
        }
        self.schedule_save();
    }

    fn quit(&mut self) {
        if self.quitting {
            return;
        }
        self.save();
        self.quitting = true;
        self.live.configure(Vec::new(), Duration::from_secs(3600));
        for (_, frame) in self.frames.drain() {
            frame.close();
        }
        self.visible = None;
        crate::frame::close_extras();
        self.exit_when_closed();
    }

    /// Quitte la boucle de messages une fois toutes les fenêtres détruites.
    fn exit_when_closed(&self) {
        if self.quitting && self.open_windows == 0 && crate::frame::extras_open() == 0 {
            quit_message_loop();
        }
    }

    // --- état et persistance ----------------------------------------------------

    fn set_github_token(&mut self, token: &str) {
        let path = self.dirs.data.join(GITHUB_TOKEN_FILE);
        let saved = if token.is_empty() {
            std::fs::remove_file(&path).or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            })
        } else {
            write_private(&path, token)
        };
        if let Err(e) = saved {
            self.toast = Some(format!("Token GitHub non enregistré : {e}"));
        }
        self.github_token = !token.is_empty();
        self.live
            .set_token(self.github_token.then(|| token.to_string()));
        self.dirty = true;
    }

    fn state_js(&self) -> String {
        let ctx = ViewContext {
            platform: std::env::consts::OS,
            sidebar_visible: self.geom.sidebar_visible,
            github_token: self.github_token,
            toast: self.toast.clone(),
        };
        format!("window.voile&&voile.render({})", ui_state(&self.ws, &ctx))
    }

    fn flush(&mut self) {
        if self.dirty {
            self.dirty = false;
            for frame in self.frames.values_mut() {
                frame.stale = true;
            }
        }
        let Some(key) = self.visible else { return };
        let needs_push = self.frames.get(&key).is_some_and(|f| f.ready && f.stale);
        if needs_push {
            let state = self.state_js();
            if let Some(frame) = self.frames.get_mut(&key) {
                frame.eval_sidebar(&state);
                frame.stale = false;
            }
            self.toast = None;
        }
    }

    fn schedule_save(&mut self) {
        if !self.save_scheduled && !self.quitting {
            self.save_scheduled = true;
            bridge::send_later(SAVE_DELAY_MS, Event::Save);
        }
    }

    fn save(&mut self) {
        if self.quitting {
            return;
        }
        if let Err(e) = self.store.save(&self.ws) {
            tracing::error!("sauvegarde du workspace : {e}");
        }
        let window = self.visible_frame().map(|f| f.window.clone());
        if let Some(window) = window {
            let maximized = window.is_maximized() != 0;
            self.geom.maximized = maximized;
            if !maximized && window.is_fullscreen() == 0 {
                let b = window.bounds();
                self.geom.bounds = Some([b.x, b.y, b.width, b.height]);
            }
        }
        let path = self.dirs.data.join("window.json");
        if let Ok(raw) = serde_json::to_vec_pretty(&self.geom) {
            if let Err(e) = std::fs::write(&path, raw) {
                tracing::warn!("écriture de {} : {e}", path.display());
            }
        }
    }
}
