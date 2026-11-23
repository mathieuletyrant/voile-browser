# Architecture

## Overview

```
 sidebar (JS, Alloy)  ──POST voile://app/api/cmd──▶  scheme.rs (IO thread)
        ▲                                                  │ bridge::send
        │ voile.render(state)                              ▼
        └──────────── shell.rs (UI thread) ◀── event queue (bridge.rs)
                         │  Workspace::apply(Command) → Vec<Effect>
                         ▼
               frame.rs: CEF windows, navigation, Live Folders (voile-live, tokio)
```

- **`voile-core`** knows nothing about CEF. Every action is a `Command`; the
  model answers with `Effect`s (create/show/destroy a view, navigate, sync a
  Live Folder). All of Arc's behaviour (pinned tabs returning to their URL,
  Today archiving, a PR reusing its tab…) is tested without a browser.
- **`voile-app`** carries the effects out with CEF and turns browser events
  (title, URL, loading, popups) back into commands.
- **The sidebar** is a web page served from `voile://app/`. It receives the
  whole state after each change (at most once per batch of events) and keeps
  no state of its own.

## One window per tab

The most visible choice, and CEF forces it.

1. Chrome extensions only exist in **Chrome style** (Alloy style lost its
   extension API in M127).
2. The `cef_runtime_style_t` documentation says *"a Chrome style Window can
   host at most one Chrome style BrowserView"*, and the code agrees:
   `CefBrowserViewImpl::AddedToWidget` refuses a second one ("Cannot add
   multiple Chrome style BrowserViews").
3. A Chrome style BrowserView cannot **move** to another window either:
   `ChromeBrowserView::InitBrowser` binds the view to its widget once
   (`DCHECK(!web_view_)`), with no way to re-attach it.
4. Chrome style native child windows work on Windows and Linux, not on macOS
   (CEF issue #3294).

So for each **loaded** tab Voile creates a frameless window holding the
sidebar (an Alloy BrowserView, allowed several times) and the page (Chrome
style). Only one is visible; switching tabs shows the target window at the
same position and in the same state (maximized, full screen), then hides the
previous one. Everything goes through CEF's public API and behaves the same
on the three systems.

Consequences:

- An unloaded tab (a pinned tab never opened, or unloaded with ⌘W) has no
  window: only *open* tabs cost memory.
- Every window has its own sidebar. Only the visible one gets the state
  continuously; the others are marked stale and refreshed right before being
  shown (`FrameWindow::stale`).
- The Chrome toolbar setting is fixed when a BrowserView is created: changing
  it recreates the windows of loaded tabs.

## Threads and re-entrancy

Every CEF callback (UI or IO thread) and every Live Folder tokio task only
pushes a `bridge::Event`. A task posted to the UI thread drains the queue
and calls the `Shell`, kept in a `thread_local`. A CEF call made from the
shell can trigger a synchronous callback: it only pushes, so nothing is
borrowed twice. If the shell is busy anyway (nested message loop of a modal
dialog), the event goes back to the front of the queue and is retried 16 ms
later.

## cef-rs binding traps

Handler methods you don't override return `Default::default()`, that is
**0**, not the C++ API's default:

- `WindowDelegate::can_resize`, `can_maximize`, `can_minimize`, `can_close`
  must be overridden, otherwise the window is frozen and cannot be closed;
- `CommandHandler::is_chrome_app_menu_item_visible` and friends must return
  1, otherwise the Chrome menu and toolbar are empty.

## Popups and new tabs

`on_before_popup`:

- `target=_blank`, middle click, `window.open(url)`, an extension's
  `chrome.tabs.create` → a **Voile tab** (`Command::PopupRequested`);
- `window.open` with a size (`NEW_POPUP`, typically OAuth) → a real popup
  window, with its own client so it doesn't overwrite the opener's title, and
  its address bar visible.

## Shortcuts

Chrome's shortcuts go through `CommandHandler::on_chrome_command`:
`IDC_NEW_TAB` opens Voile's command bar, `IDC_CLOSE_TAB` closes the *Voile*
tab, `IDC_SELECT_NEXT_TAB` follows the sidebar order, and so on. Voile's own
(⌘S, ⌘⇧D, ⌘⇧C) are caught by a `KeyboardHandler` before the page. On macOS
a minimal Edit menu is required for ⌘C/⌘V to work in pages.

## Known limits

- Windows: the Chromium sandbox requires `bootstrap.exe`, not handled yet;
  build with `--no-default-features`.
- An extension that opens a Chrome *window* (not a tab) gets a plain Chrome
  window, outside the sidebar.
- Closing the window quits Voile (no "stay in the Dock" mode).
