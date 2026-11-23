//! `voile://app/…` : sert l'interface embarquée (sidebar, page d'accueil) et
//! reçoit les commandes de l'UI.
//!
//! * `GET  voile://app/sidebar.html` … fichiers de `ui/`
//! * `POST voile://app/api/cmd` : une [`voile_core::Command`] en JSON
//! * `POST voile://app/api/ui`  : une [`crate::shell::UiAction`] en JSON
//!
//! Le handler tourne sur le thread IO : il ne fait que décoder et pousser un
//! événement vers le thread UI (voir [`crate::bridge`]).
//!
//! En développement, `VOILE_UI_DIR=/chemin/vers/ui` fait lire les fichiers
//! sur le disque (recharger la sidebar avec Ctrl/Cmd+R suffit).

use crate::bridge::{self, Event};
use crate::schemes::{HOST, SCHEME};
use crate::shell::UiAction;
use cef::wrapper::byte_read_handler::{ByteReadHandler, ByteStream};
use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;
use std::sync::{Arc, Mutex};
use voile_core::Command;

macro_rules! asset {
    ($name:literal, $mime:literal) => {
        (
            $name,
            $mime,
            include_bytes!(concat!("../../../ui/", $name)) as &[u8],
        )
    };
}

const ASSETS: &[(&str, &str, &[u8])] = &[
    asset!("sidebar.html", "text/html"),
    asset!("sidebar.css", "text/css"),
    asset!("sidebar.js", "text/javascript"),
    asset!("home.html", "text/html"),
];

pub fn register_factory() {
    let mut factory = AppSchemeFactory::new();
    register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(HOST)),
        Some(&mut factory),
    );
}

wrap_scheme_handler_factory! {
    struct AppSchemeFactory;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let request = request?;
            let url = CefString::from(&request.url()).to_string();
            let method = CefString::from(&request.method()).to_string();
            let path = url
                .strip_prefix(&format!("{SCHEME}://{HOST}/"))
                .unwrap_or_default()
                .split(['?', '#'])
                .next()
                .unwrap_or_default();

            let (status, mime, body) = match path.strip_prefix("api/") {
                Some(route) if method == "POST" => {
                    let body = read_post_body(request);
                    match handle_api(route, &body) {
                        Ok(()) => (202, "application/json", b"{\"ok\":true}".to_vec()),
                        Err(e) => {
                            tracing::warn!("api/{route} refusée : {e}");
                            let msg = serde_json::json!({ "ok": false, "error": e }).to_string();
                            (400, "application/json", msg.into_bytes())
                        }
                    }
                }
                Some(_) => (405, "text/plain", b"POST attendu".to_vec()),
                None => match asset(path) {
                    Some((mime, bytes)) => (200, mime, bytes),
                    None => (404, "text/plain", format!("{path} introuvable").into_bytes()),
                },
            };
            Some(respond(status, mime, body))
        }
    }
}

fn handle_api(route: &str, body: &[u8]) -> Result<(), String> {
    let event = match route {
        "cmd" => Event::Ui(serde_json::from_slice::<Command>(body).map_err(|e| e.to_string())?),
        "ui" => match serde_json::from_slice::<UiAction>(body).map_err(|e| e.to_string())? {
            UiAction::Ready { frame } => Event::SidebarReady(
                frame
                    .parse()
                    .map_err(|_| format!("cadre inconnu : {frame}"))?,
            ),
            other => Event::Action(other),
        },
        other => return Err(format!("route inconnue : {other}")),
    };
    bridge::send(event);
    Ok(())
}

fn asset(path: &str) -> Option<(&'static str, Vec<u8>)> {
    let path = if path.is_empty() { "home.html" } else { path };
    let (_, mime, bytes) = ASSETS.iter().find(|(name, ..)| *name == path)?;
    if let Ok(dir) = std::env::var("VOILE_UI_DIR") {
        if let Ok(fresh) = std::fs::read(std::path::Path::new(&dir).join(path)) {
            return Some((mime, fresh));
        }
    }
    Some((mime, bytes.to_vec()))
}

fn read_post_body(request: &mut Request) -> Vec<u8> {
    let mut out = Vec::new();
    let Some(post) = request.post_data() else {
        return out;
    };
    let mut elements = Vec::with_capacity(post.element_count());
    post.elements(Some(&mut elements));
    for element in elements.into_iter().flatten() {
        let len = element.bytes_count();
        let start = out.len();
        out.resize(start + len, 0);
        let read = element.bytes(len, out[start..].as_mut_ptr());
        out.truncate(start + read);
    }
    out
}

fn respond(status: i32, mime: &str, body: Vec<u8>) -> ResourceHandler {
    let mut reader = ByteReadHandler::new(Arc::new(Mutex::new(ByteStream::new(body))));
    let stream = stream_reader_create_for_handler(Some(&mut reader));
    let text = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    StreamResourceHandler::new(status, text.to_string(), mime.to_string(), None, stream)
}
