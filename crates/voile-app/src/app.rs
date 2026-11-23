use crate::bridge::{self, Event};
use crate::paths::Dirs;
use crate::shell::UiAction;
use cef::*;
use std::sync::{Arc, Mutex};

wrap_app! {
    pub struct VoileApp {
        dirs: Arc<Mutex<Option<Dirs>>>,
    }

    impl App {
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            crate::schemes::register(registrar);
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(VoileBrowserProcess::new(self.dirs.clone()))
        }
    }
}

impl VoileApp {
    pub fn create(dirs: Option<Dirs>) -> App {
        Self::new(Arc::new(Mutex::new(dirs)))
    }
}

wrap_browser_process_handler! {
    struct VoileBrowserProcess {
        dirs: Arc<Mutex<Option<Dirs>>>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            crate::scheme::register_factory();
            let dirs = self.dirs.lock().ok().and_then(|mut d| d.take()).unwrap_or_else(Dirs::resolve);
            crate::shell::start(dirs);
        }

        /// Un second lancement de Voile (même profil) : on ramène la fenêtre.
        fn on_already_running_app_relaunch(
            &self,
            _command_line: Option<&mut CommandLine>,
            _current_directory: Option<&CefString>,
        ) -> i32 {
            bridge::send(Event::Action(UiAction::Raise));
            1
        }

        /// Client des fenêtres que Chrome crée lui-même (rares avec Voile) :
        /// sans client, CEF bloquerait l'arrêt de l'application.
        fn default_client(&self) -> Option<Client> {
            Some(crate::frame::PopupClient::new())
        }
    }
}
