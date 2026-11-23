# Architecture de Voile

## Vue d'ensemble

```
 sidebar (JS, Alloy)  ──POST voile://app/api/cmd──▶  scheme.rs (thread IO)
        ▲                                                   │ bridge::send
        │ voile.render(state)                               ▼
        └──────────── shell.rs (thread UI) ◀── file d'événements (bridge.rs)
                         │  Workspace::apply(Command) → Vec<Effect>
                         ▼
               frame.rs : fenêtres CEF, navigation, Live Folders (voile-live, tokio)
```

- **`voile-core`** ne connaît pas CEF : chaque action est une `Command`,
  le modèle répond par des `Effect` (créer/montrer/détruire une vue,
  naviguer, synchroniser un Live Folder). Tout le comportement d'Arc
  (épinglés qui reviennent à leur URL, archivage du Today, réutilisation de
  l'onglet d'une PR…) est testé sans navigateur.
- **`voile-app`** exécute ces effets avec CEF et renvoie les événements du
  navigateur (titre, URL, chargement, popups) sous forme de commandes.
- **La sidebar** est une page web servie par `voile://app/`. Elle reçoit
  l'état complet à chaque changement (au plus une fois par lot
  d'événements) et ne garde aucun état métier.

## Une fenêtre par onglet

C'est le choix le plus visible, et il est imposé par CEF.

1. Les extensions Chrome n'existent qu'en **Chrome style** (le style
   *Alloy* a perdu son API d'extensions en M127).
2. La documentation de `cef_runtime_style_t` est explicite : *« a Chrome
   style Window can host at most one Chrome style BrowserView »*. Le code le
   confirme : `CefBrowserViewImpl::AddedToWidget` refuse une seconde vue
   (« Cannot add multiple Chrome style BrowserViews »).
3. Impossible non plus de **déplacer** une BrowserView Chrome style d'une
   fenêtre à l'autre : `ChromeBrowserView::InitBrowser` lie la vue au widget
   une seule fois (`DCHECK(!web_view_)`), il n'y a pas de chemin de
   ré-attachement.
4. Les fenêtres enfants natives en Chrome style marchent sous Windows et
   Linux, mais pas sous macOS (CEF issue #3294).

Voile crée donc, pour chaque onglet **chargé**, une fenêtre sans bordure
contenant la sidebar (BrowserView Alloy, autorisée en plusieurs
exemplaires) et la page (Chrome style). Une seule est visible ; changer
d'onglet montre la fenêtre cible aux mêmes coordonnées (et dans le même
état : maximisée, plein écran) puis masque l'ancienne. Tout passe par l'API
publique de CEF et se comporte pareil sur les trois systèmes.

Conséquences :

- Un onglet déchargé (épinglé jamais ouvert, ou « déchargé » avec ⌘W) n'a
  pas de fenêtre : seul le nombre d'onglets *ouverts* coûte de la mémoire.
- Chaque fenêtre a sa propre sidebar. Seule la sidebar visible reçoit l'état
  en continu ; les autres sont marquées périmées et mises à jour juste avant
  d'être montrées (`FrameWindow::stale`).
- Le réglage « barre Chrome » est figé à la création d'une BrowserView :
  le changer recrée les fenêtres des onglets chargés.

## Fils d'exécution et réentrance

Tous les callbacks CEF (thread UI ou IO) et les tâches tokio des Live
Folders ne font qu'empiler un `bridge::Event`. Une tâche postée sur le thread
UI vide la file et appelle le `Shell`, rangé dans un `thread_local`. Un appel
CEF fait depuis le shell peut déclencher un callback synchrone : il ne fait
qu'empiler, donc aucun double emprunt. Si le shell est malgré tout occupé
(boucle de messages imbriquée d'une boîte de dialogue), l'événement est remis
en tête de file et retenté 16 ms plus tard.

## Pièges des bindings cef-rs

Les méthodes non surchargées d'un handler renvoient `Default::default()`,
soit **0**, et non la valeur par défaut de l'API C++ :

- `WindowDelegate::can_resize`, `can_maximize`, `can_minimize`, `can_close`
  doivent être surchargées, sinon la fenêtre est figée et impossible à fermer ;
- `CommandHandler::is_chrome_app_menu_item_visible` et consorts doivent
  renvoyer 1, sinon le menu et la barre Chrome sont vides.

## Popups et nouveaux onglets

`on_before_popup` :

- `target=_blank`, clic molette, `window.open(url)`, `chrome.tabs.create`
  d'une extension → **onglet Voile** (`Command::PopupRequested`) ;
- `window.open` avec dimensions (`NEW_POPUP`, typiquement OAuth) → vraie
  fenêtre popup, avec un client à part pour ne pas écraser le titre de
  l'onglet parent, et la barre d'adresse visible.

## Raccourcis

Les raccourcis de Chrome passent par `CommandHandler::on_chrome_command` :
`IDC_NEW_TAB` ouvre la barre de commande de Voile, `IDC_CLOSE_TAB` ferme
l'onglet *Voile*, `IDC_SELECT_NEXT_TAB` suit l'ordre de la sidebar, etc.
Ceux propres à Voile (⌘S, ⌘⇧D, ⌘⇧C) sont captés par un `KeyboardHandler`
avant la page. Sur macOS, un menu Édition minimal est indispensable pour que
⌘C/⌘V fonctionnent dans les pages.

## Limites connues

- Le module macOS (`mac.rs`) suit l'exemple `cefsimple` de cef-rs mais n'a
  pas pu être compilé dans l'environnement où il a été écrit (Linux, sans
  cible Apple) : le premier build Mac peut demander de petits ajustements
  d'API objc2.
- Windows : la sandbox impose `bootstrap.exe`, non géré (voir README).
- Une extension qui ouvre une *fenêtre* Chrome (pas un onglet) obtient une
  fenêtre Chrome classique, hors de la sidebar.
- La fermeture de la fenêtre quitte Voile (pas de mode « reste dans le Dock »).
