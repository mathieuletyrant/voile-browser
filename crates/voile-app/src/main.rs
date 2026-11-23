//! Voile : navigateur à la Arc, en Rust sur Chromium (CEF, Chrome style).

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod bridge;
mod frame;
#[cfg(target_os = "macos")]
mod mac;
mod paths;
mod scheme;
mod schemes;
mod shell;

use cef::*;
use std::process::ExitCode;

#[cfg(all(feature = "sandbox", target_os = "windows"))]
fn main() -> ExitCode {
    eprintln!(
        "Sous Windows, compiler avec --no-default-features (la sandbox impose bootstrap.exe)."
    );
    ExitCode::FAILURE
}

#[cfg(not(all(feature = "sandbox", target_os = "windows")))]
fn main() -> ExitCode {
    #[cfg(target_os = "macos")]
    let _loader = {
        let exe = std::env::current_exe().expect("chemin de l'exécutable");
        let loader = library_loader::LibraryLoader::new(&exe, false);
        assert!(loader.load(), "chargement de Chromium Embedded Framework");
        loader
    };
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    #[cfg(target_os = "macos")]
    mac::setup_application();

    let args = args::Args::new();
    let Some(cmd_line) = args.as_cmd_line() else {
        eprintln!("ligne de commande illisible");
        return ExitCode::FAILURE;
    };
    let is_browser = cmd_line.has_switch(Some(&CefString::from("type"))) != 1;

    // Les sous-processus (rendu, GPU…) sortent ici. L'App est passée pour que
    // le schéma voile:// soit déclaré partout.
    let mut app = app::VoileApp::create(None);
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if !is_browser || code >= 0 {
        return ExitCode::from(code.clamp(0, 255) as u8);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("VOILE_LOG")
                .unwrap_or_else(|_| "voile=info,voile_live=info".into()),
        )
        .init();

    let dirs = paths::Dirs::resolve();
    if let Err(e) = std::fs::create_dir_all(&dirs.profile) {
        eprintln!("création de {} : {e}", dirs.profile.display());
        return ExitCode::FAILURE;
    }
    tracing::info!("données : {}", dirs.data.display());

    let settings = Settings {
        no_sandbox: !cfg!(feature = "sandbox") as _,
        root_cache_path: CefString::from(dirs.chromium.to_string_lossy().as_ref()),
        cache_path: CefString::from(dirs.profile.to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        // Fond des vues avant le premier rendu : évite le flash blanc.
        background_color: 0xff1c_1b22,
        ..Default::default()
    };
    let mut app = app::VoileApp::create(Some(dirs));
    if initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    ) != 1
    {
        eprintln!("initialisation de CEF impossible (Voile est peut-être déjà ouvert)");
        return ExitCode::FAILURE;
    }

    #[cfg(target_os = "macos")]
    let _delegate = mac::setup_app_delegate();

    run_message_loop();
    shell::shutdown();
    shutdown();
    ExitCode::SUCCESS
}
