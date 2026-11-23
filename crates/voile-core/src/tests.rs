use crate::*;

const T0: u64 = 1_800_000_000;

fn open(ws: &mut Workspace, url: &str) -> Id {
    let fx = ws
        .apply(
            Command::Open {
                input: url.into(),
                new_tab: true,
            },
            T0,
        )
        .unwrap();
    match fx[0] {
        Effect::CreateView { tab, .. } => tab,
        ref other => panic!("attendu CreateView, reçu {other:?}"),
    }
}

/// Workspace de départ sans ses Live Folders : `pinned` est vide.
fn bare() -> Workspace {
    let mut ws = Workspace::new();
    ws.spaces[0].pinned.clear();
    ws
}

fn perso(ws: &Workspace) -> Id {
    ws.spaces[0].id
}

#[test]
fn opening_a_tab_puts_it_on_top_of_today_and_shows_it() {
    let mut ws = Workspace::new();
    let a = open(&mut ws, "github.com");
    let b = open(&mut ws, "cheztomy.cc");
    assert_eq!(ws.active_space().today, vec![b, a]);
    assert_eq!(ws.active_tab(), Some(b));
    assert_eq!(ws.tab(a).unwrap().url, "https://github.com/");
}

#[test]
fn open_without_new_tab_navigates_the_current_one() {
    let mut ws = Workspace::new();
    let a = open(&mut ws, "github.com");
    let fx = ws
        .apply(
            Command::Open {
                input: "rust".into(),
                new_tab: false,
            },
            T0,
        )
        .unwrap();
    assert!(
        matches!(&fx[..], [Effect::LoadUrl { tab, url }] if *tab == a && url.contains("q=rust"))
    );
}

#[test]
fn closing_a_today_tab_archives_it_and_activates_the_next() {
    let mut ws = Workspace::new();
    let a = open(&mut ws, "a.com");
    let b = open(&mut ws, "b.com");
    let fx = ws.apply(Command::Close { tab: b }, T0).unwrap();
    assert!(fx.contains(&Effect::DestroyView { tab: b }));
    assert!(fx.contains(&Effect::ShowView { tab: a }));
    assert_eq!(ws.archive[0].url, "https://b.com/");
    assert!(ws.tab(b).is_err());
}

#[test]
fn closing_a_pinned_tab_only_unloads_it_and_resets_its_url() {
    let mut ws = bare();
    let a = open(&mut ws, "github.com");
    ws.apply(Command::Pin { tab: a }, T0).unwrap();
    ws.apply(
        Command::TabUpdated {
            tab: a,
            update: TabUpdate {
                url: Some("https://github.com/pulls".into()),
                ..Default::default()
            },
        },
        T0,
    )
    .unwrap();

    let fx = ws.apply(Command::Close { tab: a }, T0).unwrap();
    assert!(fx.contains(&Effect::DestroyView { tab: a }));
    let tab = ws.tab(a).unwrap();
    assert!(!tab.loaded);
    assert_eq!(tab.url, "https://github.com/");
    assert!(matches!(ws.active_space().pinned[0], Node::Tab { tab } if tab == a));

    // Réactiver recrée la vue.
    let fx = ws.apply(Command::Activate { tab: a }, T0).unwrap();
    assert_eq!(
        fx[0],
        Effect::CreateView {
            tab: a,
            url: "https://github.com/".into()
        }
    );
}

#[test]
fn spaces_keep_their_own_active_tab() {
    let mut ws = Workspace::new();
    let a = open(&mut ws, "a.com");
    let home = perso(&ws);
    let fx = ws
        .apply(
            Command::CreateSpace {
                name: "Dev".into(),
                icon: "🦀".into(),
                theme: Theme::new("#c9dcf7", "#cfe9de"),
            },
            T0,
        )
        .unwrap();
    let dev = ws.active_space;
    assert!(fx.contains(&Effect::ShowEmpty));
    let b = open(&mut ws, "b.com");
    assert_eq!(ws.space(dev).unwrap().today, vec![b]);

    let fx = ws.apply(Command::SwitchSpace { space: home }, T0).unwrap();
    assert!(fx.contains(&Effect::ShowView { tab: a }));
}

#[test]
fn moving_tabs_into_folders_pins_them() {
    let mut ws = bare();
    let a = open(&mut ws, "a.com");
    ws.apply(
        Command::CreateFolder {
            name: "Maison".into(),
        },
        T0,
    )
    .unwrap();
    let Node::Folder(f) = &ws.active_space().pinned[0] else {
        panic!()
    };
    let folder = f.id;
    let space = perso(&ws);

    ws.apply(
        Command::Move {
            node: NodeRef::Tab(a),
            to: Location {
                space,
                container: Container::Folder(folder),
                index: 0,
            },
        },
        T0,
    )
    .unwrap();
    assert!(ws.active_space().today.is_empty());
    let Node::Folder(f) = &ws.active_space().pinned[0] else {
        panic!()
    };
    assert_eq!(f.children, vec![Node::Tab { tab: a }]);
    assert_eq!(
        ws.tab(a).unwrap().pinned_url.as_deref(),
        Some("https://a.com/")
    );
}

#[test]
fn a_folder_cannot_be_moved_into_itself() {
    let mut ws = bare();
    ws.apply(Command::CreateFolder { name: "A".into() }, T0)
        .unwrap();
    ws.apply(Command::CreateFolder { name: "B".into() }, T0)
        .unwrap();
    let ids: Vec<Id> = ws
        .active_space()
        .pinned
        .iter()
        .map(|n| match n {
            Node::Folder(f) => f.id,
            _ => unreachable!(),
        })
        .collect();
    let space = perso(&ws);
    let mv = |node, target| Command::Move {
        node: NodeRef::Folder(node),
        to: Location {
            space,
            container: Container::Folder(target),
            index: 0,
        },
    };
    ws.apply(mv(ids[1], ids[0]), T0).unwrap(); // B dans A
    assert!(ws.apply(mv(ids[0], ids[1]), T0).is_err()); // A dans B : cycle
    assert!(ws.apply(mv(ids[0], ids[0]), T0).is_err());
}

#[test]
fn stale_today_tabs_are_archived_but_not_the_active_one() {
    let mut ws = Workspace::new();
    let old = open(&mut ws, "old.com");
    let current = open(&mut ws, "current.com");
    let later = T0 + 13 * 3600;
    let fx = ws.apply(Command::Tick { now: later }, later).unwrap();
    assert_eq!(fx, vec![Effect::DestroyView { tab: old }]);
    assert_eq!(ws.active_space().today, vec![current]);
}

#[test]
fn live_results_mark_new_items_and_opening_reuses_the_tab() {
    let mut ws = Workspace::new();
    let folder = ws.live_folders()[0].id;
    let pr = LiveItem {
        key: "tauri-apps/cef-rs#42".into(),
        title: "Add views example".into(),
        url: "https://github.com/tauri-apps/cef-rs/pull/42".into(),
        subtitle: "tauri-apps/cef-rs · @someone".into(),
        updated_at: "2026-11-20T10:00:00Z".into(),
        badges: vec![Badge::ChecksPassing],
        avatar: None,
        unread: false,
    };
    ws.apply(
        Command::LiveResult {
            folder,
            result: Ok(vec![pr.clone()]),
        },
        T0,
    )
    .unwrap();
    assert!(ws.live_folders()[0].items[0].unread);

    let open_pr = || Command::OpenLiveItem {
        folder,
        key: pr.key.clone(),
    };
    let fx = ws.apply(open_pr(), T0).unwrap();
    let Effect::CreateView { tab, .. } = fx[0] else {
        panic!("{fx:?}")
    };
    assert!(!ws.live_folders()[0].items[0].unread);

    // Même PR, pas d'update : reste lue. Deuxième ouverture : même onglet.
    ws.apply(
        Command::LiveResult {
            folder,
            result: Ok(vec![pr.clone()]),
        },
        T0,
    )
    .unwrap();
    assert!(!ws.live_folders()[0].items[0].unread);
    let fx = ws.apply(open_pr(), T0).unwrap();
    assert!(fx.contains(&Effect::ShowView { tab }));
    assert!(!fx.iter().any(|e| matches!(e, Effect::CreateView { .. })));

    // Erreur réseau : on garde les éléments et on affiche l'erreur.
    ws.apply(
        Command::LiveResult {
            folder,
            result: Err("401".into()),
        },
        T0,
    )
    .unwrap();
    assert_eq!(ws.live_folders()[0].items.len(), 1);
    assert_eq!(ws.live_folders()[0].status.error.as_deref(), Some("401"));
}

#[test]
fn ui_commands_deserialize_from_camel_case_json() {
    let cmd: Command = serde_json::from_str(
        r#"{"type":"move","node":{"kind":"tab","id":3},"to":{"space":1,"container":{"kind":"folder","id":7},"index":2}}"#,
    )
    .unwrap();
    assert_eq!(
        cmd,
        Command::Move {
            node: NodeRef::Tab(3),
            to: Location {
                space: 1,
                container: Container::Folder(7),
                index: 2
            }
        }
    );
    let cmd: Command =
        serde_json::from_str(r#"{"type":"addLiveFolder","name":"cef-rs","source":{"type":"githubSearch","query":"repo:tauri-apps/cef-rs is:pr"}}"#)
            .unwrap();
    assert!(matches!(cmd, Command::AddLiveFolder { .. }));
}

#[test]
fn store_roundtrip_resets_runtime_state() {
    let dir = std::env::temp_dir().join(format!("voile-test-{}", std::process::id()));
    let store = Store::new(&dir);
    let mut ws = Workspace::new();
    let a = open(&mut ws, "a.com");
    assert!(ws.tab(a).unwrap().loaded);
    store.save(&ws).unwrap();

    let loaded = store.load().unwrap();
    assert!(!loaded.tab(a).unwrap().loaded);
    assert_eq!(loaded.active_space().today, vec![a]);

    std::fs::write(store.path(), b"{ pas du json").unwrap();
    let fresh = store.load().unwrap();
    assert!(fresh.tabs.is_empty());
    assert!(dir.join("workspace.json.broken").exists());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn tab_order_follows_the_sidebar() {
    let mut ws = Workspace::new();
    let a = open(&mut ws, "a.com");
    let b = open(&mut ws, "b.com");
    let c = open(&mut ws, "c.com");
    ws.apply(Command::Favorite { tab: c }, T0).unwrap();
    ws.apply(Command::Pin { tab: a }, T0).unwrap();
    assert_eq!(ws.tab_order(), vec![c, a, b]);
    assert!(ws.is_pinned(a) && ws.is_pinned(c) && !ws.is_pinned(b));
}

#[test]
fn live_folders_carry_their_space_repos() {
    let mut ws = Workspace::new();
    let fx = ws
        .apply(
            Command::SetSpaceRepos {
                space: None,
                repos: vec![" getprimo ".into(), "getprimo".into(), "".into()],
            },
            T0,
        )
        .unwrap();
    assert_eq!(fx, vec![Effect::LiveConfigChanged]);
    assert_eq!(ws.live_sources()[0].2, vec!["getprimo".to_string()]);
}
