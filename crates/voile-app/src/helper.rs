//! Exécutable des sous-processus sur macOS (« Voile Helper (Renderer).app »…).
//! Il déclare le schéma voile:// comme le processus principal.

mod schemes;

use cef::*;

wrap_app! {
    struct HelperApp;

    impl App {
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            schemes::register(registrar);
        }
    }
}

fn main() {
    let args = args::Args::new();

    #[cfg(all(target_os = "macos", feature = "sandbox"))]
    let _sandbox = {
        let mut sandbox = cef::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    };

    #[cfg(target_os = "macos")]
    let _loader = {
        let exe = std::env::current_exe().expect("chemin du helper");
        let loader = library_loader::LibraryLoader::new(&exe, true);
        assert!(loader.load(), "chargement de Chromium Embedded Framework");
        loader
    };

    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let mut app = HelperApp::new();
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    std::process::exit(code.max(0));
}
