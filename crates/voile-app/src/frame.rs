//! Un *cadre* = une fenêtre CEF sans bordure qui contient la sidebar (vue
//! Alloy, `voile://app/sidebar.html`) et la page.
//!
//! Pourquoi une fenêtre par onglet ? Les extensions Chrome n'existent qu'en
//! « Chrome style », et CEF n'accepte qu'**une** BrowserView Chrome style par
//! fenêtre, définitivement liée à son widget (voir docs/architecture.md).
//! Voile garde donc une fenêtre cachée par onglet chargé et n'en montre
//! qu'une, aux coordonnées de la précédente : changer d'onglet revient à
//! permuter deux fenêtres identiques.

use crate::bridge::{self, Event};
use crate::shell::UiAction;
use cef::*;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use voile_core::{Command, Id, TabUpdate};

pub const SIDEBAR_MIN: i32 = 200;

// Type de l'événement natif passé aux handlers clavier, propre à chaque OS.
#[cfg(target_os = "linux")]
type OsEvent<'a> = Option<&'a mut sys::XEvent>;
#[cfg(target_os = "windows")]
type OsEvent<'a> = Option<&'a mut sys::MSG>;
#[cfg(target_os = "macos")]
type OsEvent<'a> = *mut u8;
pub const SIDEBAR_MAX: i32 = 420;

// --- identité ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameKey {
    /// Aucun onglet actif : sidebar + page d'accueil.
    Home,
    Tab(Id),
}

/// Clé + génération : un onglet dont le cadre est recréé (changement de
/// réglage) ne doit pas recevoir les événements de l'ancienne fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameId {
    pub key: FrameKey,
    pub gen: u32,
}

impl fmt::Display for FrameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.key {
            FrameKey::Home => write!(f, "home.{}", self.gen),
            FrameKey::Tab(id) => write!(f, "tab{id}.{}", self.gen),
        }
    }
}

impl FromStr for FrameId {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        let (key, gen) = s.split_once('.').ok_or(())?;
        let gen = gen.parse().map_err(|_| ())?;
        let key = match key {
            "home" => FrameKey::Home,
            k => FrameKey::Tab(k.strip_prefix("tab").ok_or(())?.parse().map_err(|_| ())?),
        };
        Ok(Self { key, gen })
    }
}

// --- création ----------------------------------------------------------------

pub struct FrameSpec {
    pub id: FrameId,
    /// URL de la page (ignorée pour l'accueil).
    pub url: String,
    pub show_toolbar: bool,
    pub bounds: Rect,
    pub sidebar_width: Arc<AtomicI32>,
    pub sidebar_visible: bool,
}

pub struct FrameWindow {
    pub id: FrameId,
    pub window: Window,
    pub sidebar: BrowserView,
    pub page: BrowserView,
    allow_close: Arc<AtomicBool>,
    /// La sidebar a chargé son JS et peut recevoir l'état.
    pub ready: bool,
    /// L'état affiché par la sidebar est périmé.
    pub stale: bool,
}

impl FrameWindow {
    pub fn create(spec: FrameSpec) -> Option<FrameWindow> {
        let FrameSpec {
            id,
            url,
            show_toolbar,
            bounds,
            sidebar_width,
            sidebar_visible,
        } = spec;
        let settings = BrowserSettings {
            background_color: 0x0000_0000,
            ..Default::default()
        };

        let sidebar_url = format!("voile://app/sidebar.html?frame={id}");
        let mut sidebar_client = UiClient::new(true);
        let mut sidebar_delegate = SidebarViewDelegate::new(sidebar_width);
        let sidebar = browser_view_create(
            Some(&mut sidebar_client),
            Some(&CefString::from(sidebar_url.as_str())),
            Some(&settings),
            None,
            None,
            Some(&mut sidebar_delegate),
        )?;
        sidebar.set_visible(sidebar_visible as _);

        let page = match id.key {
            FrameKey::Tab(tab) => {
                let mut client = PageClient::new(tab);
                let mut delegate = PageViewDelegate::new(show_toolbar);
                browser_view_create(
                    Some(&mut client),
                    Some(&CefString::from(url.as_str())),
                    Some(&BrowserSettings::default()),
                    None,
                    None,
                    Some(&mut delegate),
                )?
            }
            FrameKey::Home => {
                let mut client = UiClient::new(false);
                let mut delegate = SidebarViewDelegate::new(Arc::new(AtomicI32::new(0)));
                browser_view_create(
                    Some(&mut client),
                    Some(&CefString::from("voile://app/home.html")),
                    Some(&settings),
                    None,
                    None,
                    Some(&mut delegate),
                )?
            }
        };

        let allow_close = Arc::new(AtomicBool::new(false));
        let mut delegate = FrameWindowDelegate::new(
            id,
            sidebar.clone(),
            page.clone(),
            bounds,
            allow_close.clone(),
        );
        let window = window_create_top_level(Some(&mut delegate))?;
        Some(FrameWindow {
            id,
            window,
            sidebar,
            page,
            allow_close,
            ready: false,
            stale: true,
        })
    }

    /// Ferme la fenêtre (les navigateurs d'abord, `beforeunload` compris).
    pub fn close(&self) {
        self.allow_close.store(true, Ordering::Release);
        self.window.close();
    }

    pub fn page_browser(&self) -> Option<Browser> {
        self.page.browser()
    }

    pub fn eval_sidebar(&self, code: &str) {
        if let Some(frame) = self.sidebar.browser().and_then(|b| b.main_frame()) {
            frame.execute_java_script(
                Some(&CefString::from(code)),
                Some(&CefString::from("voile://app/shell.js")),
                0,
            );
        }
    }

    pub fn set_sidebar_visible(&self, visible: bool) {
        self.sidebar.set_visible(visible as _);
        self.window.layout();
    }

    pub fn relayout(&self) {
        self.sidebar.invalidate_layout();
        self.window.layout();
    }
}

fn try_close(view: &BrowserView) -> bool {
    view.browser()
        .and_then(|b| b.host())
        .is_none_or(|h| h.try_close_browser() != 0)
}

// --- fenêtre -------------------------------------------------------------------

wrap_window_delegate! {
    struct FrameWindowDelegate {
        id: FrameId,
        sidebar: BrowserView,
        page: BrowserView,
        bounds: Rect,
        allow_close: Arc<AtomicBool>,
    }

    impl ViewDelegate {
        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 640, height: 420 }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else { return };
            let layout = window.set_to_box_layout(Some(&BoxLayoutSettings {
                horizontal: 1,
                ..Default::default()
            }));
            let mut sidebar = View::from(&self.sidebar);
            let mut page = View::from(&self.page);
            window.add_child_view(Some(&mut sidebar));
            window.add_child_view(Some(&mut page));
            if let Some(layout) = layout {
                layout.set_flex_for_view(Some(&mut sidebar), 0);
                layout.set_flex_for_view(Some(&mut page), 1);
            }
            window.set_title(Some(&CefString::from("Voile")));
            // Pas de show() ici : le shell montre le cadre quand l'onglet est activé.
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            bridge::send(Event::WindowDestroyed(self.id));
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            if !self.allow_close.load(Ordering::Acquire) {
                // Bouton fermer / Alt+F4 : on quitte Voile proprement.
                bridge::send(Event::WindowClosing(self.id));
                return 0;
            }
            // Les deux appels lancent la fermeture ; CEF rappellera can_close
            // quand chaque navigateur aura fini (`&` évite le court-circuit).
            (try_close(&self.page) & try_close(&self.sidebar)) as i32
        }

        fn initial_bounds(&self, _window: Option<&mut Window>) -> Rect {
            self.bounds.clone()
        }

        fn is_frameless(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> i32 {
            cfg!(target_os = "macos") as i32
        }

        fn titlebar_height(&self, _window: Option<&mut Window>, height: Option<&mut f32>) -> i32 {
            // Aligne les feux tricolores de macOS sur la barre d'adresse.
            if let Some(h) = height {
                *h = 46.0;
            }
            1
        }

        fn can_resize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }
    }
}

// Fenêtre ordinaire pour les vraies popups (OAuth, `window.open` avec
// dimensions) et les DevTools.
wrap_window_delegate! {
    struct PopupWindowDelegate {
        view: BrowserView,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 900, height: 720 }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else { return };
            window.set_to_fill_layout();
            window.add_child_view(Some(&mut View::from(&self.view)));
            window.show();
            self.view.request_focus();
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            try_close(&self.view) as i32
        }

        fn can_resize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }
    }
}

// --- vues ------------------------------------------------------------------------

wrap_browser_view_delegate! {
    struct SidebarViewDelegate {
        width: Arc<AtomicI32>,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: self.width.load(Ordering::Relaxed), height: 0 }
        }
    }

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_browser_view_delegate! {
    struct PageViewDelegate {
        show_toolbar: bool,
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }

        fn chrome_toolbar_type(&self, _view: Option<&mut BrowserView>) -> ChromeToolbarType {
            if self.show_toolbar {
                ChromeToolbarType::NORMAL
            } else {
                ChromeToolbarType::NONE
            }
        }

        fn delegate_for_popup_browser_view(
            &self,
            _view: Option<&mut BrowserView>,
            _settings: Option<&BrowserSettings>,
            _client: Option<&mut Client>,
            _is_devtools: i32,
        ) -> Option<BrowserViewDelegate> {
            Some(PopupViewDelegate::new())
        }

        fn on_popup_browser_view_created(
            &self,
            _view: Option<&mut BrowserView>,
            popup: Option<&mut BrowserView>,
            _is_devtools: i32,
        ) -> i32 {
            open_popup_window(popup)
        }
    }
}

wrap_browser_view_delegate! {
    struct PopupViewDelegate {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }

        fn chrome_toolbar_type(&self, _view: Option<&mut BrowserView>) -> ChromeToolbarType {
            // L'adresse reste visible : on sait sur quel site on s'authentifie.
            ChromeToolbarType::LOCATION
        }

        fn on_popup_browser_view_created(
            &self,
            _view: Option<&mut BrowserView>,
            popup: Option<&mut BrowserView>,
            _is_devtools: i32,
        ) -> i32 {
            open_popup_window(popup)
        }
    }
}

fn open_popup_window(popup: Option<&mut BrowserView>) -> i32 {
    let Some(popup) = popup else { return 0 };
    let mut delegate = PopupWindowDelegate::new(popup.clone());
    window_create_top_level(Some(&mut delegate));
    1
}

// --- client de la page (onglet) ------------------------------------------------------

wrap_client! {
    struct PageClient {
        tab: Id,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(PageDisplay::new(self.tab))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(PageLoad::new(self.tab))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(PageLifeSpan::new(self.tab))
        }

        fn command_handler(&self) -> Option<CommandHandler> {
            Some(PageCommands::new(self.tab))
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(Shortcuts::new())
        }
    }
}

fn update(tab: Id, update: TabUpdate) {
    bridge::send(Event::Internal(Command::TabUpdated { tab, update }));
}

wrap_display_handler! {
    struct PageDisplay {
        tab: Id,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if frame.is_some_and(|f| f.is_main() != 0) {
                let url = url.map(CefString::to_string);
                update(self.tab, TabUpdate { url, ..Default::default() });
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            let title = title.map(CefString::to_string);
            update(self.tab, TabUpdate { title, ..Default::default() });
        }

        fn on_favicon_urlchange(
            &self,
            _browser: Option<&mut Browser>,
            icon_urls: Option<&mut CefStringList>,
        ) {
            let favicon = icon_urls.and_then(|list| list.clone().into_iter().next());
            if favicon.is_some() {
                update(self.tab, TabUpdate { favicon, ..Default::default() });
            }
        }
    }
}

wrap_load_handler! {
    struct PageLoad {
        tab: Id,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: i32,
            can_go_back: i32,
            can_go_forward: i32,
        ) {
            update(
                self.tab,
                TabUpdate {
                    loading: Some(is_loading != 0),
                    can_go_back: Some(can_go_back != 0),
                    can_go_forward: Some(can_go_forward != 0),
                    ..Default::default()
                },
            );
        }
    }
}

wrap_life_span_handler! {
    struct PageLifeSpan {
        tab: Id,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: i32,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            target_disposition: WindowOpenDisposition,
            _user_gesture: i32,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut i32>,
        ) -> i32 {
            if target_disposition == WindowOpenDisposition::NEW_POPUP {
                // Vraie popup (OAuth…) : fenêtre à part, avec son propre client
                // pour ne pas écraser le titre/l'URL de l'onglet parent.
                if let Some(client) = client {
                    *client = Some(PopupClient::new());
                }
                return 0;
            }
            // target=_blank, clic molette, chrome.tabs.create… → onglet Voile.
            let url = target_url.map(CefString::to_string).unwrap_or_default();
            if !url.is_empty() {
                bridge::send(Event::Internal(Command::PopupRequested { from: self.tab, url }));
            }
            1
        }
    }
}

// Raccourcis Chrome interceptés pour garder le modèle d'Arc : Cmd/Ctrl+T
// ouvre la barre de commande, Cmd/Ctrl+W ferme l'onglet Voile, etc.
wrap_command_handler! {
    struct PageCommands {
        tab: Id,
    }

    impl CommandHandler {
        fn on_chrome_command(
            &self,
            _browser: Option<&mut Browser>,
            command_id: i32,
            _disposition: WindowOpenDisposition,
        ) -> i32 {
            use cef::sys::*;
            let action = match command_id {
                IDC_NEW_TAB | IDC_NEW_WINDOW => UiAction::FocusCommandBar { new_tab: true },
                IDC_FOCUS_LOCATION | IDC_FOCUS_SEARCH => UiAction::FocusCommandBar { new_tab: false },
                IDC_CLOSE_TAB | IDC_CLOSE_WINDOW => {
                    bridge::send(Event::Ui(Command::Close { tab: self.tab }));
                    return 1;
                }
                IDC_SELECT_NEXT_TAB => UiAction::CycleTab { delta: 1 },
                IDC_SELECT_PREVIOUS_TAB => UiAction::CycleTab { delta: -1 },
                IDC_RESTORE_TAB => UiAction::ReopenClosed,
                IDC_EXIT => UiAction::Quit,
                IDC_NEW_INCOGNITO_WINDOW => return 1,
                id if (IDC_SELECT_TAB_0..IDC_SELECT_LAST_TAB).contains(&id) => {
                    UiAction::SelectIndex { index: (id - IDC_SELECT_TAB_0) as usize }
                }
                IDC_SELECT_LAST_TAB => UiAction::SelectIndex { index: usize::MAX },
                _ => return 0,
            };
            bridge::send(Event::Action(action));
            1
        }

        // Les valeurs par défaut des bindings valent 0 (= masqué) : on
        // rétablit le comportement de Chrome.
        fn is_chrome_app_menu_item_visible(&self, _browser: Option<&mut Browser>, command_id: i32) -> i32 {
            use cef::sys::*;
            !matches!(command_id, IDC_NEW_WINDOW | IDC_NEW_INCOGNITO_WINDOW) as i32
        }

        fn is_chrome_app_menu_item_enabled(&self, _browser: Option<&mut Browser>, _command_id: i32) -> i32 {
            1
        }

        fn is_chrome_page_action_icon_visible(&self, _icon_type: ChromePageActionIconType) -> i32 {
            1
        }

        fn is_chrome_toolbar_button_visible(&self, _button_type: ChromeToolbarButtonType) -> i32 {
            1
        }
    }
}

// Raccourcis propres à Voile, valables aussi quand la page a le focus.
wrap_keyboard_handler! {
    struct Shortcuts;

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: OsEvent<'_>,
            _is_keyboard_shortcut: Option<&mut i32>,
        ) -> i32 {
            let Some(event) = event else { return 0 };
            if event.type_ != KeyEventType::RAWKEYDOWN {
                return 0;
            }
            let primary = if cfg!(target_os = "macos") {
                sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0
            } else {
                sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0
            };
            let shift = sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0;
            let mods = event.modifiers & (primary | shift);
            let action = match (event.windows_key_code, mods) {
                // Cmd/Ctrl+S : afficher/masquer la sidebar (comme Arc).
                (0x53, m) if m == primary => UiAction::ToggleSidebar,
                // Cmd/Ctrl+Maj+C : copier l'URL de l'onglet.
                (0x43, m) if m == primary | shift => UiAction::CopyUrl,
                // Cmd/Ctrl+Maj+D : épingler/désépingler l'onglet.
                (0x44, m) if m == primary | shift => UiAction::TogglePin,
                _ => return 0,
            };
            bridge::send(Event::Action(action));
            1
        }
    }
}

// --- clients de l'UI interne et des popups -----------------------------------------------

// Client de la sidebar et de la page d'accueil.
wrap_client! {
    struct UiClient {
        is_sidebar: bool,
    }

    impl Client {
        fn drag_handler(&self) -> Option<DragHandler> {
            self.is_sidebar.then(SidebarDrag::new)
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(UiLifeSpan::new())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(UiConsole::new())
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(Shortcuts::new())
        }
    }
}

wrap_drag_handler! {
    struct SidebarDrag;

    impl DragHandler {
        /// `-webkit-app-region: drag` dans la sidebar → zones de déplacement
        /// de la fenêtre sans bordure. La sidebar est à l'origine de la
        /// fenêtre, les coordonnées n'ont pas à être converties.
        fn on_draggable_regions_changed(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            regions: Option<&[DraggableRegion]>,
        ) {
            let window = browser_view_get_for_browser(browser).and_then(|v| v.window());
            if let Some(window) = window {
                window.set_draggable_regions(regions);
            }
        }
    }
}

wrap_life_span_handler! {
    struct UiLifeSpan;

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: i32,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: i32,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut i32>,
        ) -> i32 {
            let input = target_url.map(CefString::to_string).unwrap_or_default();
            if !input.is_empty() {
                bridge::send(Event::Ui(Command::Open { input, new_tab: true }));
            }
            1
        }
    }
}

wrap_display_handler! {
    struct UiConsole;

    impl DisplayHandler {
        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            source: Option<&CefString>,
            line: i32,
        ) -> i32 {
            tracing::debug!(
                target: "voile::ui",
                "{}:{line} {}",
                source.map(CefString::to_string).unwrap_or_default(),
                message.map(CefString::to_string).unwrap_or_default(),
            );
            0
        }
    }
}

wrap_client! {
    pub struct PopupClient;

    impl Client {}
}
