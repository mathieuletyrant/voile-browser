# Voile

Un navigateur façon [Arc](https://arc.net), écrit en Rust sur Chromium
([CEF](https://bitbucket.org/chromiumembedded/cef) via
[cef-rs](https://github.com/tauri-apps/cef-rs)), avec :

- **la sidebar d'Arc** : espaces avec leurs couleurs, favoris en tuiles,
  onglets épinglés (qui reviennent à leur URL d'origine), dossiers, section
  « Aujourd'hui » archivée automatiquement après 12 h, barre de commande ;
- **les extensions Chrome** : les pages tournent en *Chrome style*, donc Web
  Store, `chrome://extensions`, icônes d'extensions dans la barre Chrome ;
- **des Live Folders GitHub** : dossiers qui se remplissent tout seuls avec
  une recherche GitHub (PR à relire, mes PR, un dépôt…), avec pastille
  « non lu » et badges de statut (checks, review, brouillon, conflits).

## Lancer

Prérequis : Rust ≥ 1.85, CMake et Ninja (pour `libcef_dll_wrapper`). Les
binaires CEF (~300 Mo) sont téléchargés au premier build.

```sh
# Facultatif mais conseillé : un seul téléchargement de CEF pour tous les builds
cargo install export-cef-dir@156.7.0   # même version que le crate cef
export-cef-dir --force ~/.local/share/cef
export CEF_PATH=~/.local/share/cef
# Linux : export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$CEF_PATH"

cargo run -p voile-bundle -- --release   # assemble target/bundle/
```

| OS | Résultat | Lancer |
| --- | --- | --- |
| macOS | `target/bundle/voile.app` (l'outil de cef-rs compile toujours en debug sur macOS) | `open target/bundle/voile.app` |
| Linux | `target/bundle/voile` + ressources Chromium à côté | `./target/bundle/voile` |
| Windows | non testé, voir ci-dessous | |

Windows : la sandbox Chromium y impose le lanceur `bootstrap.exe`, que Voile
ne gère pas encore. Il faut compiler sans (`cargo build -p voile-app --release
--no-default-features`) puis copier à la main les fichiers de `$CEF_PATH` à
côté de `voile.exe` ; `voile-bundle` ne sait pas encore transmettre cette option.

### Token GitHub

Les Live Folders utilisent, dans l'ordre : `VOILE_GITHUB_TOKEN`,
`GITHUB_TOKEN`, `GH_TOKEN`, puis `gh auth token`. Le plus simple : `gh auth
login`. Il faut les droits `repo` (dépôts privés) et `read:org`.

## Utiliser

| Raccourci | Action |
| --- | --- |
| ⌘T / Ctrl+T | Barre de commande, nouvel onglet (recherche aussi dans les onglets, les PR et les onglets fermés) |
| ⌘L / Ctrl+L | Barre de commande sur l'onglet courant |
| ⌘W / Ctrl+W | Fermer l'onglet (un épinglé est seulement déchargé) |
| ⌘S / Ctrl+S | Afficher/masquer la sidebar |
| ⌘⇧D | Épingler/désépingler |
| ⌘⇧C | Copier l'URL |
| Ctrl+Tab, ⌘1…9 | Onglet suivant, n-ième onglet |
| ⌘⇧T | Rouvrir le dernier onglet fermé |

- **Glisser-déposer** : onglets vers les favoris, les épinglés, un dossier
  (sur le milieu de son en-tête) ou l'icône d'un autre espace.
- **Clic droit** partout : épingler, déplacer vers un espace, renommer
  (ou double-clic), rafraîchir un Live Folder, tout marquer comme lu…
- **Menu ⋯** (bas de la sidebar) : extensions, Web Store, barre Chrome,
  historique, téléchargements, mots de passe.
- **Live Folder** : clic droit dans la sidebar → « Nouveau Live Folder
  GitHub », puis un préréglage ou n'importe quelle recherche GitHub
  (`is:pr is:open repo:owner/nom`, `is:open assignee:@me`…). Rafraîchi
  toutes les 60 s ; cliquer sur une PR l'ouvre (ou réactive son onglet).

Les extensions s'installent depuis le Chrome Web Store comme dans Chrome.
Leurs icônes apparaissent dans la barre Chrome au-dessus de la page
(affichée par défaut ; masquable depuis le menu ⋯).

## Données

`workspace.json` (sidebar), `window.json` (géométrie) et le profil Chromium
(`chromium/profile` : cookies, extensions, mots de passe) sont dans :

- macOS : `~/Library/Application Support/cc.cheztomy.Voile/`
- Linux : `~/.local/share/voile/`
- Windows : `%APPDATA%\cheztomy\Voile\data\`

`VOILE_DATA_DIR=/tmp/voile-test` donne un profil jetable.
`VOILE_LOG=debug` affiche les journaux (y compris la console de la sidebar).

## Développer

```sh
cargo test -p voile-core -p voile-live        # modèle et source GitHub, sans CEF
cargo check -p voile-app -F voile-app/dox     # vérifie le shell sans binaires CEF
VOILE_UI_DIR=$PWD/ui cargo run -p voile-app   # sidebar lue depuis ui/ : ⌘R pour recharger
cargo run -q -p voile-core --example demo_state > /tmp/state.json  # état de démo pour ui/
```

| Crate | Rôle |
| --- | --- |
| `voile-core` | Modèle d'Arc (espaces, onglets, dossiers, Live Folders) : `Command` → `Effect`, persistance. Aucune dépendance à CEF, entièrement testé. |
| `voile-live` | Synchro des Live Folders (une requête GraphQL GitHub par dossier) sur un runtime tokio à part. |
| `voile-app` | Shell CEF : fenêtres, sidebar, schéma `voile://`, raccourcis. |
| `voile-bundle` | Assemble l'application avec les binaires CEF. |
| `ui/` | Sidebar et page d'accueil (HTML/CSS/JS sans dépendance, embarqués dans le binaire). |

Le pourquoi des choix (une fenêtre par onglet, etc.) est dans
[docs/architecture.md](docs/architecture.md).
