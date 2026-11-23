//! Écrit sur stdout un état de sidebar réaliste, produit par le vrai modèle.
//! Pratique pour travailler sur `ui/` dans un navigateur ordinaire :
//!
//! ```sh
//! cargo run -q -p voile-core --example demo_state > /tmp/state.json
//! ```

use voile_core::view::{ui_state, ViewContext};
use voile_core::*;

fn main() {
    let t = 1_800_000_000;
    let mut ws = Workspace::new();
    let open = |ws: &mut Workspace, url: &str| -> Id {
        match ws
            .apply(
                Command::Open {
                    input: url.into(),
                    new_tab: true,
                },
                t,
            )
            .unwrap()[0]
        {
            Effect::CreateView { tab, .. } => tab,
            _ => unreachable!(),
        }
    };
    let titled = |ws: &mut Workspace, tab: Id, title: &str, favicon: Option<&str>| {
        let update = TabUpdate {
            title: Some(title.into()),
            favicon: favicon.map(Into::into),
            ..Default::default()
        };
        ws.apply(Command::TabUpdated { tab, update }, t).unwrap();
    };

    // Espace « Dev » : Live Folders + un dossier.
    let dev = ws.spaces[1].id;
    ws.apply(Command::SwitchSpace { space: dev }, t).unwrap();
    let gh = open(&mut ws, "github.com/mathieuletyrant/cozytouch");
    titled(
        &mut ws,
        gh,
        "mathieuletyrant/cozytouch",
        Some("https://github.githubassets.com/favicons/favicon.svg"),
    );
    ws.apply(Command::Pin { tab: gh }, t).unwrap();
    ws.apply(
        Command::CreateFolder {
            name: "Docs".into(),
        },
        t,
    )
    .unwrap();
    let docs = open(&mut ws, "docs.rs/cef");
    titled(&mut ws, docs, "cef - Rust", None);
    let folder = ws.active_space().pinned.iter().find_map(|n| match n {
        Node::Folder(f) => Some(f.id),
        _ => None,
    });
    let to = Location {
        space: dev,
        container: Container::Folder(folder.unwrap()),
        index: 0,
    };
    ws.apply(
        Command::Move {
            node: NodeRef::Tab(docs),
            to,
        },
        t,
    )
    .unwrap();

    let review = ws.live_folders()[0].id;
    let item = |n: u64, title: &str, author: &str, badges: Vec<Badge>| LiveItem {
        key: format!("mathieuletyrant/cozytouch#{n}"),
        title: title.into(),
        url: format!("https://github.com/mathieuletyrant/cozytouch/pull/{n}"),
        subtitle: format!("mathieuletyrant/cozytouch #{n} · @{author}"),
        updated_at: format!("2026-11-2{n}T10:00:00Z"),
        badges,
        avatar: None,
        unread: false,
    };
    let items = vec![
        item(
            1,
            "Ajoute le mode absence aux clims",
            "julie-dev",
            vec![Badge::ChecksPassing, Badge::ReviewRequired],
        ),
        item(
            2,
            "Corrige la reconnexion après expiration du token",
            "bardesss",
            vec![Badge::Draft, Badge::ChecksFailing, Badge::Conflicts],
        ),
        item(
            3,
            "Capteurs de consommation par appareil",
            "renovate",
            vec![Badge::ChecksPending, Badge::Approved],
        ),
    ];
    ws.apply(
        Command::LiveResult {
            folder: review,
            result: Ok(items),
        },
        t,
    )
    .unwrap();
    let fx = ws
        .apply(
            Command::OpenLiveItem {
                folder: review,
                key: "mathieuletyrant/cozytouch#1".into(),
            },
            t,
        )
        .unwrap();
    let Effect::CreateView { tab: pr, .. } = fx[0] else {
        unreachable!()
    };
    let mine = ws.live_folders()[1].id;
    ws.apply(
        Command::LiveResult {
            folder: mine,
            result: Err(
                "token GitHub introuvable : définis GITHUB_TOKEN ou lance `gh auth login`".into(),
            ),
        },
        t,
    )
    .unwrap();

    let crates = open(&mut ws, "crates.io/crates/cef");
    titled(
        &mut ws,
        crates,
        "cef - crates.io: Rust Package Registry",
        None,
    );
    titled(
        &mut ws,
        pr,
        "Ajoute le mode absence aux clims by julie-dev · Pull Request #1",
        Some("https://github.githubassets.com/favicons/favicon.svg"),
    );
    ws.apply(Command::Activate { tab: pr }, t).unwrap();
    let ha = open(&mut ws, "homeassistant.local:8123");
    titled(&mut ws, ha, "Accueil – Home Assistant", None);
    ws.apply(Command::Favorite { tab: ha }, t).unwrap();
    ws.apply(Command::Activate { tab: pr }, t).unwrap();

    let ctx = ViewContext {
        platform: "macos",
        sidebar_visible: true,
        toast: None,
    };
    println!("{}", ui_state(&ws, &ctx));
}
