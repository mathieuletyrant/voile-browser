//! Ce que la sidebar reçoit : le workspace sérialisé, allégé de l'archive
//! complète (seuls les plus récents servent à la barre de commande), plus
//! quelques infos d'affichage calculées côté Rust.

use crate::workspace::Workspace;
use serde_json::{json, Value};

/// Nombre d'entrées d'archive envoyées pour la recherche de la barre de commande.
const RECENT: usize = 60;

/// Infos de contexte fournies par le shell.
#[derive(Debug, Clone, Default)]
pub struct ViewContext {
    /// `macos`, `windows` ou `linux` (la sidebar réserve la place des feux
    /// tricolores sur macOS et dessine ses propres boutons ailleurs).
    pub platform: &'static str,
    pub sidebar_visible: bool,
    /// Dernière erreur à afficher (commande refusée, etc.).
    pub toast: Option<String>,
}

pub fn ui_state(ws: &Workspace, ctx: &ViewContext) -> Value {
    let mut v = serde_json::to_value(ws).unwrap_or(Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.remove("archive");
        obj.remove("nextId");
        obj.insert(
            "recent".into(),
            serde_json::to_value(&ws.archive[..ws.archive.len().min(RECENT)]).unwrap_or_default(),
        );
        obj.insert("activeTab".into(), json!(ws.active_tab()));
        obj.insert(
            "ui".into(),
            json!({
                "platform": ctx.platform,
                "sidebarVisible": ctx.sidebar_visible,
                "toast": ctx.toast,
            }),
        );
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Workspace};

    #[test]
    fn state_drops_full_archive_and_exposes_active_tab() {
        let mut ws = Workspace::new();
        for i in 0..80 {
            ws.apply(
                Command::Open {
                    input: format!("site{i}.com"),
                    new_tab: true,
                },
                1,
            )
            .unwrap();
        }
        ws.apply(Command::ClearToday, 2).unwrap();
        assert_eq!(ws.archive.len(), 80);

        let v = ui_state(
            &ws,
            &ViewContext {
                platform: "macos",
                sidebar_visible: true,
                toast: None,
            },
        );
        assert!(v.get("archive").is_none());
        assert!(v.get("nextId").is_none());
        assert_eq!(v["recent"].as_array().unwrap().len(), RECENT);
        assert_eq!(v["activeTab"], Value::Null);
        assert_eq!(v["ui"]["platform"], "macos");
        assert!(v["spaces"].as_array().unwrap().len() >= 2);
    }
}
