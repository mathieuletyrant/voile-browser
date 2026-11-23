//! Déclaration du schéma `voile://`. Doit être faite dans *tous* les
//! processus (navigateur, rendu…), d'où ce module sans dépendance partagé
//! avec l'exécutable helper de macOS.

use cef::*;

pub const SCHEME: &str = "voile";
#[allow(dead_code)] // inutilisé par le helper
pub const HOST: &str = "app";

pub fn register(registrar: Option<&mut SchemeRegistrar>) {
    let Some(registrar) = registrar else {
        return;
    };
    // Standard + sécurisé : origine propre, `fetch()` et `navigator.clipboard`
    // disponibles dans la sidebar.
    let options = SchemeOptions::STANDARD.get_raw()
        | SchemeOptions::SECURE.get_raw()
        | SchemeOptions::CORS_ENABLED.get_raw()
        | SchemeOptions::FETCH_ENABLED.get_raw();
    registrar.add_custom_scheme(Some(&CefString::from(SCHEME)), options as _);
}
