'use strict';
// Sidebar de Voile. Le shell Rust pousse l'état avec `voile.render(state)` ;
// la sidebar renvoie des commandes par `POST voile://app/api/{cmd,ui}`.

(() => {
  const FRAME = new URLSearchParams(location.search).get('frame') || '';
  const $ = (sel, el = document) => el.querySelector(sel);
  const $$ = (sel, el = document) => [...el.querySelectorAll(sel)];

  // --- API ------------------------------------------------------------------------
  const post = (route, body) =>
    fetch(`voile://app/api/${route}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    })
      .then((r) => (r.ok ? null : r.json().then((j) => toast(j.error || 'Commande refusée'))))
      .catch(() => {});
  const cmd = (type, args = {}) => post('cmd', { type, ...args });
  const ui = (type, args = {}) => post('ui', { type, ...args });

  // --- icônes (traits 24×24) ----------------------------------------------------------
  const PATHS = {
    sidebar: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M9 4v16"/>',
    back: '<path d="M15 18l-6-6 6-6"/>',
    forward: '<path d="M9 18l6-6-6-6"/>',
    reload: '<path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 4v7h-7"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    minus: '<path d="M6 12h12"/>',
    x: '<path d="M6 6l12 12M18 6L6 18"/>',
    close: '<path d="M6 6l12 12M18 6L6 18"/>',
    more: '<circle cx="5" cy="12" r="1.3"/><circle cx="12" cy="12" r="1.3"/><circle cx="19" cy="12" r="1.3"/>',
    minimize: '<path d="M5 12h14"/>',
    maximize: '<rect x="5" y="5" width="14" height="14" rx="2"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/>',
    globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
    chevron: '<path d="M9 6l6 6-6 6"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    folderOpen: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v1H7.5a2 2 0 0 0-1.9 1.4L3 19z"/><path d="M3 19l2.6-7.6A2 2 0 0 1 7.5 10H21l-2.7 7.6a2 2 0 0 1-1.9 1.4H3"/>',
    pr: '<circle cx="6" cy="6" r="2.5"/><circle cx="6" cy="18" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M6 8.5v7M18 15.5V9a3 3 0 0 0-3-3h-4"/><path d="M13 3.5L10.5 6 13 8.5"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    clock: '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l2.5 2.5"/>',
    approved: '<circle cx="12" cy="12" r="9"/><path d="M8 12.5l3 3 5-6"/>',
    changes: '<circle cx="12" cy="12" r="9"/><path d="M12 7.5v5.5M12 16.5h.01"/>',
    eye: '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    draft: '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    conflict: '<path d="M12 3l9.5 17h-19z"/><path d="M12 10v4M12 17h.01"/>',
    alert: '<path d="M12 3l9.5 17h-19z"/><path d="M12 10v4M12 17h.01"/>',
    reset: '<path d="M9 14L4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 0 10h-3"/>',
    pin: '<path d="M9 4h6l-1 6 3 3H7l3-3z"/><path d="M12 16v5"/>',
    star: '<path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8L3.5 9.7l5.9-.9z"/>',
    trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>',
    edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
    puzzle: '<path d="M10 4a2 2 0 1 1 4 0v2h4v4h-2a2 2 0 1 0 0 4h2v4h-4v-2a2 2 0 1 0-4 0v2H6v-4H4a2 2 0 1 1 0-4h2V6h4z"/>',
    store: '<path d="M4 9l1.5-5h13L20 9"/><path d="M4 9h16v11H4z"/><path d="M9 20v-6h6v6"/>',
    toolbar: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 9h18"/>',
    code: '<path d="M8 8l-4 4 4 4M16 8l4 4-4 4"/>',
    quit: '<path d="M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4"/><path d="M15 16l4-4-4-4M19 12H9"/>',
    space: '<rect x="4" y="4" width="16" height="16" rx="4"/><path d="M4 10h16"/>',
    history: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5M12 8v4l3 2"/>',
    download: '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
    key: '<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M17 6l3 3"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1.2l2-1.6-2-3.4-2.4 1a7 7 0 0 0-2-1.2L14 3h-4l-.5 2.6a7 7 0 0 0-2 1.2l-2.4-1-2 3.4 2 1.6A7 7 0 0 0 5 12c0 .4 0 .8.1 1.2l-2 1.6 2 3.4 2.4-1a7 7 0 0 0 2 1.2L10 21h4l.5-2.6a7 7 0 0 0 2-1.2l2.4 1 2-3.4-2-1.6c.1-.4.1-.8.1-1.2z"/>',
    github: '<path d="M9 19c-4.3 1.4-4.3-2.5-6-3m12 5v-3.5c0-1 .1-1.4-.5-2 2.8-.3 5.5-1.4 5.5-6a4.6 4.6 0 0 0-1.3-3.2 4.2 4.2 0 0 0-.1-3.2s-1.1-.3-3.5 1.3a12.3 12.3 0 0 0-6.2 0C6.5 2.8 5.4 3.1 5.4 3.1a4.2 4.2 0 0 0-.1 3.2A4.6 4.6 0 0 0 4 9.5c0 4.6 2.7 5.7 5.5 6-.6.6-.6 1.2-.5 2V21"/>',
  };
  const icon = (name) =>
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${PATHS[name] || ''}</svg>`;
  const hydrateIcons = (root = document) =>
    $$('[data-icon]', root).forEach((el) => {
      if (!el.firstChild) el.innerHTML = icon(el.dataset.icon);
    });

  // --- utilitaires ------------------------------------------------------------------------
  const esc = (s) =>
    String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
  const isMac = () => (S ? S.ui.platform === 'macos' : /Mac/.test(navigator.platform));
  const modKey = (e) => (isMac() ? e.metaKey : e.ctrlKey);
  const kbd = (k) => (isMac() ? `⌘${k}` : `Ctrl+${k}`);

  function displayUrl(url) {
    try {
      const u = new URL(url);
      if (u.protocol === 'http:' || u.protocol === 'https:') {
        const path = u.pathname === '/' ? '' : u.pathname;
        return u.host.replace(/^www\./, '') + path;
      }
      return url;
    } catch {
      return url || '';
    }
  }
  function hostOf(url) {
    try {
      return new URL(url).host.replace(/^www\./, '');
    } catch {
      return '';
    }
  }
  function hue(str) {
    let h = 0;
    for (const c of str) h = (h * 31 + c.charCodeAt(0)) % 360;
    return h;
  }
  const looksLikeUrl = (q) =>
    /^[a-z][a-z0-9+.-]*:\S*$/i.test(q) ||
    (!/\s/.test(q) && /^(localhost|\d{1,3}(\.\d{1,3}){3}|[\w-]+(\.[\w-]+)*\.[a-z]{2,})(:\d+)?([/?#].*)?$/i.test(q));

  // --- état ------------------------------------------------------------------------------------
  let S = null;
  let held = 0; // rendu suspendu pendant un glisser-déposer ou une saisie
  let pending = null;
  const hold = () => held++;
  const release = () => {
    held = Math.max(0, held - 1);
    if (!held && pending) {
      const p = pending;
      pending = null;
      render(p);
    }
  };

  const space = () => S.spaces.find((s) => s.id === S.activeSpace) || S.spaces[0];
  // Recherche d'un Live Folder restreinte aux dépôts de l'espace (même règle que LiveSource::scoped).
  const scopedQuery = (l) =>
    [l.source.query, ...(space().githubRepos || []).map((r) => `${r.includes('/') ? 'repo' : 'user'}:${r}`)].join(' ');
  const tabOf = (id) => (id == null ? null : S.tabs[id]);
  const activeTab = () => tabOf(S.activeTab);
  const allLive = () => {
    const out = [];
    const walk = (nodes, sp) =>
      nodes.forEach((n) => {
        if (n.kind === 'live') out.push({ ...n, space: sp });
        if (n.kind === 'folder') walk(n.children, sp);
      });
    S.spaces.forEach((sp) => walk(sp.pinned, sp));
    return out;
  };
  const inPinned = (tabId) => {
    let hit = false;
    const walk = (nodes) => nodes.forEach((n) => {
      if (n.kind === 'tab' && n.tab === tabId) hit = true;
      if (n.kind === 'folder') walk(n.children);
    });
    S.spaces.forEach((sp) => walk(sp.pinned));
    return hit;
  };

  // --- rendu -------------------------------------------------------------------------------------
  function luminance(hex) {
    const m = /^#?([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(hex || '');
    if (!m) return 1;
    const [r, g, b] = m.slice(1).map((x) => {
      const c = parseInt(x, 16) / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  }
  function applyTheme(theme) {
    const root = document.documentElement;
    root.style.setProperty('--from', theme.from);
    root.style.setProperty('--to', theme.to);
    root.classList.toggle('dark', (luminance(theme.from) + luminance(theme.to)) / 2 < 0.32);
  }

  function favicon(t, big = false) {
    if (t.loading && !big) return '<span class="spin"></span>';
    if (t.favicon) return `<span class="fav"><img src="${esc(t.favicon)}" alt="" data-letter="${esc(letter(t))}"></span>`;
    return letterFav(t);
  }
  const letter = (t) => (hostOf(t.url) || t.title || '?').charAt(0).toUpperCase();
  const letterFav = (t) =>
    `<span class="fav letter" style="--c:hsl(${hue(hostOf(t.url) || t.url)} 45% 55%)">${esc(letter(t))}</span>`;

  function tabRow(t, where) {
    const active = t.id === S.activeTab;
    const pinned = where === 'pinned';
    const moved = pinned && t.loaded && t.pinnedUrl && t.pinnedUrl !== t.url;
    const cls = ['row', 'tab', active && 'active', !t.loaded && 'unloaded'].filter(Boolean).join(' ');
    return `<div class="${cls}" draggable="true" data-item="tab:${t.id}" data-tab="${t.id}" data-where="${where}"
        title="${esc(t.title)}&#10;${esc(t.url)}">
      ${favicon(t)}
      <span class="label">${esc(t.title || displayUrl(t.url))}</span>
      ${moved ? `<button class="reset shown" data-act="resetPinned" title="Revenir à ${esc(displayUrl(t.pinnedUrl))}">${icon('reset')}</button>` : ''}
      ${pinned && !t.loaded ? '' : `<button class="close" data-act="close" title="${pinned ? 'Décharger' : 'Fermer'} (${kbd('W')})">${icon(pinned ? 'minus' : 'x')}</button>`}
    </div>`;
  }

  const BADGES = {
    draft: ['muted', 'draft', 'Brouillon'],
    checksPassing: ['ok', 'check', 'Checks réussis'],
    checksFailing: ['bad', 'x', 'Checks en échec'],
    checksPending: ['warn', 'clock', 'Checks en cours'],
    approved: ['ok', 'approved', 'Approuvée'],
    changesRequested: ['bad', 'changes', 'Modifications demandées'],
    reviewRequired: ['info', 'eye', 'Review requise'],
    conflicts: ['bad', 'conflict', 'Conflits à résoudre'],
  };
  const badge = (b) => {
    const [cls, ic, label] = BADGES[b] || ['muted', 'clock', b];
    return `<span class="badge ${cls}" title="${label}">${icon(ic)}</span>`;
  };

  function liveItem(l, i) {
    const tab = Object.values(S.tabs).find((t) => t.origin && t.origin.folder === l.id && t.origin.key === i.key);
    const active = tab && tab.id === S.activeTab;
    return `<div class="row live-item${i.unread ? ' unread' : ''}${active ? ' active' : ''}" data-live-item="${l.id}" data-key="${esc(i.key)}"
        title="${esc(i.title)}&#10;${esc(i.subtitle)}">
      ${i.avatar ? `<img class="avatar" src="${esc(i.avatar)}" alt="">` : '<span class="avatar"></span>'}
      <span class="label">${esc(i.title)}<span class="sub">${esc(i.subtitle)}</span></span>
      <span class="badges">${(i.badges || []).map(badge).join('')}</span>
    </div>`;
  }

  function liveNode(l) {
    const st = l.status || {};
    const unread = l.items.filter((i) => i.unread).length;
    const empty = !l.items.length;
    const connected = S.ui.githubToken;
    const msg = !connected
      ? '<button class="live-connect" data-connect>Connecter GitHub</button>'
      : st.error
      ? `<div class="live-error">${esc(st.error)}</div>`
      : empty
        ? `<div class="live-empty">${st.syncing || !st.lastSync ? 'Synchronisation…' : 'Rien en attente 🎉'}</div>`
        : '';
    return `<div class="folder live${l.open ? ' open' : ''}" data-item="live:${l.id}">
      <div class="row" draggable="true" data-node="live:${l.id}" data-toggle title="${esc(scopedQuery(l))}">
        <span class="chev">${icon('chevron')}</span>
        <span class="fav">${st.syncing && empty ? '<span class="spin"></span>' : icon('pr')}</span>
        <span class="label">${esc(l.name)}</span>
        ${connected && st.error ? `<span class="badge bad" title="${esc(st.error)}">${icon('alert')}</span>` : ''}
        <span class="count${unread ? ' unread' : ''}" title="${unread ? `${unread} nouveau(x)` : `${l.items.length} élément(s)`}">${unread || l.items.length}</span>
      </div>
      <div class="children">${msg}${l.items.map((i) => liveItem(l, i)).join('')}</div>
    </div>`;
  }

  function node(n) {
    if (n.kind === 'tab') {
      const t = tabOf(n.tab);
      return t ? tabRow(t, 'pinned') : '';
    }
    if (n.kind === 'live') return liveNode(n);
    return `<div class="folder${n.open ? ' open' : ''}" data-item="folder:${n.id}">
      <div class="row" draggable="true" data-node="folder:${n.id}" data-toggle data-drop-into="folder:${n.id}">
        <span class="chev">${icon('chevron')}</span>
        <span class="fav">${icon(n.open ? 'folderOpen' : 'folder')}</span>
        <span class="label">${esc(n.name)}</span>
      </div>
      <div class="children" data-container="folder:${n.id}">${n.children.map(node).join('')}</div>
    </div>`;
  }

  function render(state) {
    if (held) {
      pending = state;
      return;
    }
    S = state;
    const sp = space();
    applyTheme(sp.theme);
    document.documentElement.classList.toggle('mac', S.ui.platform === 'macos');

    const t = activeTab();
    $('#btn-back').disabled = !t || !t.canGoBack;
    $('#btn-forward').disabled = !t || !t.canGoForward;
    $('#btn-reload').disabled = !t;
    $('#address-text').textContent = t ? displayUrl(t.url) : 'Rechercher ou saisir une adresse';
    $('#address-lock').innerHTML = icon(t && t.url.startsWith('https:') ? 'globe' : 'search');

    $('#favorites').innerHTML = S.favorites
      .map(tabOf)
      .filter(Boolean)
      .map((f) => `<div class="tile${f.id === S.activeTab ? ' active' : ''}${f.loaded ? '' : ' unloaded'}${f.loading ? ' loading' : ''}"
          draggable="true" data-item="tab:${f.id}" data-tab="${f.id}" data-where="favorites" title="${esc(f.title)}&#10;${esc(f.url)}">${favicon(f, true)}</div>`)
      .join('');
    $('#pinned').innerHTML = sp.pinned.map(node).join('');
    $('#today').innerHTML = sp.today.map(tabOf).filter(Boolean).map((x) => tabRow(x, 'today')).join('');
    $('#btn-clear').hidden = !sp.today.length;
    $('#spaces').innerHTML = S.spaces
      .map((s) => `<button class="space${s.id === sp.id ? ' active' : ''}" data-space="${s.id}" title="${esc(s.name)}">${esc(s.icon)}</button>`)
      .join('');

    $('#new-tab-kbd').textContent = kbd('T');
    if (S.ui.toast) toast(S.ui.toast);
    if (commandOpen) updatePalette();
  }

  // Favicon cassée → lettre.
  document.addEventListener(
    'error',
    (e) => {
      const img = e.target;
      if (img.tagName !== 'IMG') return;
      if (img.dataset.letter) {
        const span = img.parentElement;
        span.classList.add('letter');
        span.style.setProperty('--c', `hsl(${hue(img.dataset.letter)} 45% 55%)`);
        span.textContent = img.dataset.letter;
      } else {
        img.style.visibility = 'hidden';
      }
    },
    true,
  );

  // --- toast ----------------------------------------------------------------------------------------
  let toastTimer;
  function toast(msg) {
    const el = $('#toast');
    el.textContent = msg;
    el.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (el.hidden = true), 4000);
  }

  // --- barre de commande -------------------------------------------------------------------------------
  const input = $('#command-input');
  const palette = $('#palette');
  let commandOpen = false;
  let commandNewTab = false;
  let results = [];
  let selected = 0;

  function focusCommand(newTab) {
    if (!S) return;
    closeMenus();
    commandOpen = true;
    commandNewTab = !!newTab || !activeTab();
    $('#address').hidden = true;
    input.hidden = false;
    input.placeholder = commandNewTab ? 'Nouvel onglet : rechercher ou saisir une adresse' : 'Rechercher ou saisir une adresse';
    input.value = commandNewTab ? '' : activeTab().url;
    input.focus();
    input.select();
    updatePalette();
  }

  function closeCommand(focusPage) {
    if (!commandOpen) return;
    commandOpen = false;
    input.hidden = true;
    palette.hidden = true;
    $('#address').hidden = false;
    if (focusPage) ui('focusPage');
  }

  const ACTIONS = () => [
    { label: 'Nouveau dossier', icon: 'folder', run: () => folderDialog() },
    { label: 'Nouveau Live Folder GitHub', icon: 'pr', keys: 'pr pull request github live', run: () => liveDialog() },
    { label: 'Nouvel espace', icon: 'space', run: () => spaceDialog() },
    { label: 'Token GitHub', icon: 'key', keys: 'github token pat connecter live pr', run: () => tokenDialog() },
    { label: 'Extensions', icon: 'puzzle', keys: 'extensions modules', run: () => cmd('open', { input: 'chrome://extensions', newTab: true }) },
    { label: 'Chrome Web Store', icon: 'store', keys: 'extensions store boutique', run: () => cmd('open', { input: 'https://chromewebstore.google.com', newTab: true }) },
    {
      label: S.settings.showToolbar ? 'Masquer la barre Chrome' : 'Afficher la barre Chrome',
      icon: 'toolbar',
      keys: 'toolbar barre omnibox extensions',
      run: () => cmd('setShowToolbar', { show: !S.settings.showToolbar }),
    },
    { label: 'Copier l\'URL', icon: 'copy', keys: 'url lien', run: () => ui('copyUrl') },
    { label: 'Effacer Aujourd\'hui', icon: 'trash', keys: 'clear fermer onglets', run: () => cmd('clearToday') },
    { label: 'Historique', icon: 'history', run: () => cmd('open', { input: 'chrome://history', newTab: true }) },
    { label: 'Téléchargements', icon: 'download', run: () => cmd('open', { input: 'chrome://downloads', newTab: true }) },
    { label: 'Mots de passe', icon: 'key', run: () => cmd('open', { input: 'chrome://password-manager', newTab: true }) },
    { label: 'Paramètres Chrome', icon: 'gear', keys: 'settings réglages', run: () => cmd('open', { input: 'chrome://settings', newTab: true }) },
    { label: 'Masquer la sidebar', icon: 'sidebar', run: () => ui('toggleSidebar') },
    { label: 'Outils de développement', icon: 'code', keys: 'devtools inspecter', run: () => ui('devTools', { sidebar: false }) },
    { label: 'Outils de développement de la sidebar', icon: 'code', keys: 'devtools', run: () => ui('devTools', { sidebar: true }) },
    { label: 'Quitter Voile', icon: 'quit', run: () => ui('quit') },
  ];

  const matches = (q, ...fields) => {
    if (!q) return true;
    const hay = fields.join(' ').toLowerCase();
    return q.toLowerCase().split(/\s+/).every((w) => hay.includes(w));
  };

  function updatePalette() {
    const q = input.value.trim();
    const current = activeTab();
    const typed = !commandNewTab && current && q === current.url ? '' : q;
    results = [];
    if (typed) {
      const url = looksLikeUrl(typed);
      results.push({
        icon: url ? 'globe' : 'search',
        label: url ? `Aller à ${typed}` : `Rechercher « ${typed} »`,
        run: () => cmd('open', { input: typed, newTab: commandNewTab }),
      });
    }
    const tabs = Object.values(S.tabs)
      .filter((t) => t.id !== S.activeTab && matches(typed, t.title, t.url))
      .sort((a, b) => (b.lastActive || 0) - (a.lastActive || 0))
      .slice(0, typed ? 6 : 5)
      .map((t) => ({ group: 'Onglets', tab: t, label: t.title || displayUrl(t.url), sub: displayUrl(t.url), run: () => cmd('activate', { tab: t.id }) }));
    const live = typed
      ? allLive()
          .flatMap((l) => l.items.map((i) => ({ l, i })))
          .filter(({ i }) => matches(typed, i.title, i.subtitle, i.key))
          .slice(0, 6)
          .map(({ l, i }) => ({ group: l.name, icon: 'pr', label: i.title, sub: i.subtitle, run: () => cmd('openLiveItem', { folder: l.id, key: i.key }) }))
      : [];
    const recent = (S.recent || [])
      .filter((r) => matches(typed, r.title, r.url))
      .slice(0, typed ? 4 : 3)
      .map((r) => ({ group: 'Récemment fermés', icon: 'history', label: r.title || displayUrl(r.url), sub: displayUrl(r.url), run: () => cmd('open', { input: r.url, newTab: true }) }));
    const actions = ACTIONS()
      .filter((a) => (typed ? matches(typed, a.label, a.keys || '') : commandNewTab))
      .slice(0, typed ? 4 : 6)
      .map((a) => ({ ...a, group: 'Actions' }));
    results.push(...tabs, ...live, ...recent, ...actions);
    selected = Math.min(selected, Math.max(0, results.length - 1));
    if (typed && results.length) selected = Math.min(selected, results.length - 1);

    let group = null;
    palette.innerHTML = results
      .map((r, idx) => {
        let head = '';
        if (r.group && r.group !== group) head = `<div class="group">${esc(r.group)}</div>`;
        group = r.group || group;
        const fav = r.tab ? favicon(r.tab) : `<span class="fav">${icon(r.icon)}</span>`;
        return `${head}<div class="row${idx === selected ? ' selected' : ''}" data-result="${idx}">${fav}
          <span class="label">${esc(r.label)}${r.sub ? `<span class="sub">${esc(r.sub)}</span>` : ''}</span></div>`;
      })
      .join('');
    palette.hidden = !results.length;
    const sel = $('.row.selected', palette);
    if (sel) sel.scrollIntoView({ block: 'nearest' });
  }

  function runResult(idx) {
    const r = results[idx];
    closeCommand(false);
    if (r) r.run();
  }

  input.addEventListener('input', () => {
    selected = 0;
    updatePalette();
  });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || (e.key === 'n' && e.ctrlKey)) {
      selected = Math.min(results.length - 1, selected + 1);
      updatePalette();
      e.preventDefault();
    } else if (e.key === 'ArrowUp' || (e.key === 'p' && e.ctrlKey)) {
      selected = Math.max(0, selected - 1);
      updatePalette();
      e.preventDefault();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const q = input.value.trim();
      if (results.length) runResult(selected);
      else if (q) {
        closeCommand(false);
        cmd('open', { input: q, newTab: commandNewTab });
      } else closeCommand(true);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeCommand(true);
    }
  });
  input.addEventListener('blur', () =>
    setTimeout(() => {
      if (document.activeElement !== input) closeCommand(false);
    }, 150),
  );
  palette.addEventListener('mousedown', (e) => {
    e.preventDefault(); // garder le focus dans la saisie
    const row = e.target.closest('[data-result]');
    if (row) runResult(+row.dataset.result);
  });
  $('#address').addEventListener('click', () => focusCommand(false));
  $('#address').addEventListener('keydown', (e) => e.key === 'Enter' && focusCommand(false));

  // --- clics -------------------------------------------------------------------------------------------
  $('#btn-sidebar').addEventListener('click', () => ui('toggleSidebar'));
  $('#btn-back').addEventListener('click', () => S.activeTab != null && cmd('back', { tab: S.activeTab }));
  $('#btn-forward').addEventListener('click', () => S.activeTab != null && cmd('forward', { tab: S.activeTab }));
  $('#btn-reload').addEventListener('click', () => S.activeTab != null && cmd('reload', { tab: S.activeTab }));
  $('#btn-clear').addEventListener('click', () => cmd('clearToday'));
  $('#new-tab').addEventListener('click', () => focusCommand(true));
  $('#btn-space').addEventListener('click', () => spaceDialog());
  $('#btn-menu').addEventListener('click', (e) => {
    const r = e.currentTarget.getBoundingClientRect();
    const items = ACTIONS().filter((a) => !/^Outils de développement de la sidebar/.test(a.label));
    openMenu(items.slice(0, 7).concat([null], items.slice(7)), r.left, r.top, true);
  });
  $$('[data-win]').forEach((b) => b.addEventListener('click', () => ui(b.dataset.win)));

  $('#scroll').addEventListener('click', (e) => {
    const act = e.target.closest('[data-act]');
    const tabEl = e.target.closest('[data-tab]');
    if (act && tabEl) {
      e.stopPropagation();
      const tab = +tabEl.dataset.tab;
      if (act.dataset.act === 'close') cmd('close', { tab });
      if (act.dataset.act === 'resetPinned') cmd('resetPinned', { tab }).then(() => cmd('activate', { tab }));
      return;
    }
    if (e.target.closest('[data-connect]')) return void tokenDialog();
    if (tabEl) return void cmd('activate', { tab: +tabEl.dataset.tab });
    const item = e.target.closest('[data-live-item]');
    if (item) return void cmd('openLiveItem', { folder: +item.dataset.liveItem, key: item.dataset.key });
    const toggle = e.target.closest('[data-toggle]');
    if (toggle) {
      const [kind, id] = toggle.dataset.node.split(':');
      cmd('toggleOpen', { node: { kind, id: +id } });
    }
  });
  $('#scroll').addEventListener('auxclick', (e) => {
    const tabEl = e.target.closest('[data-tab]');
    if (e.button === 1 && tabEl) cmd('close', { tab: +tabEl.dataset.tab });
  });
  $('#scroll').addEventListener('dblclick', (e) => {
    const row = e.target.closest('[data-node]');
    if (row && !row.dataset.tab) {
      const [kind, id] = row.dataset.node.split(':');
      rename(row, { kind, id: +id });
    }
  });
  $('#spaces').addEventListener('click', (e) => {
    const b = e.target.closest('[data-space]');
    if (b) cmd('switchSpace', { space: +b.dataset.space });
  });

  // --- renommage en place ----------------------------------------------------------------------------------
  function rename(row, nodeRef) {
    const label = $('.label', row);
    const field = document.createElement('input');
    field.className = 'rename';
    field.value = label.textContent;
    row.classList.add('editing');
    label.after(field);
    hold();
    field.focus();
    field.select();
    let done = false;
    const finish = (save) => {
      if (done) return;
      done = true;
      const name = field.value.trim();
      field.remove();
      row.classList.remove('editing');
      if (save && name && name !== label.textContent) {
        label.textContent = name;
        cmd('rename', { node: nodeRef, name });
      }
      release();
    };
    field.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') finish(true);
      if (e.key === 'Escape') finish(false);
      e.stopPropagation();
    });
    field.addEventListener('blur', () => finish(true));
    field.addEventListener('click', (e) => e.stopPropagation());
  }

  // --- menus -------------------------------------------------------------------------------------------------
  const menu = $('#menu');
  function openMenu(items, x, y, above = false) {
    closeMenus();
    menu.innerHTML = items
      .map((it, i) =>
        it === null
          ? '<hr>'
          : `<button class="item${it.danger ? ' danger' : ''}" data-i="${i}">${icon(it.icon || 'chevron')}<span>${esc(it.label)}</span>${it.checked ? `<span class="check">${icon('check')}</span>` : ''}</button>`,
      )
      .join('');
    menu.hidden = false;
    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    menu.style.left = `${Math.max(6, Math.min(x, innerWidth - w - 6))}px`;
    menu.style.top = `${Math.max(6, Math.min(above ? y - h - 6 : y, innerHeight - h - 6))}px`;
    menu.onclick = (e) => {
      const b = e.target.closest('[data-i]');
      if (!b) return;
      closeMenus();
      items[+b.dataset.i].run();
    };
  }
  function closeMenus() {
    menu.hidden = true;
  }
  // Un clic dans la page n'arrive pas jusqu'ici : la perte du focus ferme les menus.
  window.addEventListener('blur', () => closeMenus());
  document.addEventListener('mousedown', (e) => {
    if (!menu.hidden && !menu.contains(e.target)) closeMenus();
  });

  const otherSpaces = () => S.spaces.filter((s) => s.id !== S.activeSpace);

  function tabMenu(t, where) {
    const node = { kind: 'tab', id: t.id };
    const items = [];
    if (where === 'favorites') items.push({ label: 'Retirer des favoris', icon: 'star', run: () => cmd('unfavorite', { tab: t.id }) });
    else items.push({ label: 'Ajouter aux favoris', icon: 'star', run: () => cmd('favorite', { tab: t.id }) });
    if (where === 'pinned') items.push({ label: 'Désépingler', icon: 'pin', run: () => cmd('unpin', { tab: t.id }) });
    if (where === 'today') items.push({ label: 'Épingler', icon: 'pin', run: () => cmd('pin', { tab: t.id }) });
    if (t.pinnedUrl && t.pinnedUrl !== t.url) items.push({ label: 'Revenir à l\'URL épinglée', icon: 'reset', run: () => cmd('resetPinned', { tab: t.id }) });
    items.push(
      { label: 'Copier l\'URL', icon: 'copy', run: () => copy(t.url) },
      { label: 'Dupliquer', icon: 'plus', run: () => cmd('open', { input: t.url, newTab: true }) },
    );
    for (const s of otherSpaces()) {
      items.push({
        label: `Déplacer vers ${s.icon} ${s.name}`,
        icon: 'space',
        run: () => cmd('move', { node, to: { space: s.id, container: { kind: 'today' }, index: 0 } }),
      });
    }
    items.push(null);
    items.push({ label: where === 'today' ? 'Fermer' : 'Décharger', icon: 'x', danger: where === 'today', run: () => cmd('close', { tab: t.id }) });
    if (where !== 'today') items.push({ label: 'Supprimer', icon: 'trash', danger: true, run: () => cmd('delete', { node }) });
    return items;
  }

  function folderMenu(row, id) {
    const node = { kind: 'folder', id };
    return [
      { label: 'Renommer', icon: 'edit', run: () => rename(row, node) },
      ...otherSpaces().map((s) => ({
        label: `Déplacer vers ${s.icon} ${s.name}`,
        icon: 'space',
        run: () => cmd('move', { node, to: { space: s.id, container: { kind: 'pinned' }, index: 9999 } }),
      })),
      null,
      { label: 'Supprimer le dossier et ses onglets', icon: 'trash', danger: true, run: () => cmd('delete', { node }) },
    ];
  }

  function liveMenu(row, id) {
    const l = allLive().find((x) => x.id === id);
    const node = { kind: 'live', id };
    return [
      { label: 'Rafraîchir', icon: 'reload', run: () => cmd('refreshLive', { folder: id }) },
      { label: 'Tout marquer comme lu', icon: 'check', run: () => cmd('markLiveRead', { folder: id }) },
      {
        label: 'Ouvrir la recherche sur GitHub',
        icon: 'github',
        run: () => cmd('open', { input: `https://github.com/pulls?q=${encodeURIComponent(scopedQuery(l))}`, newTab: true }),
      },
      { label: 'Renommer', icon: 'edit', run: () => rename(row, node) },
      null,
      { label: 'Supprimer le Live Folder', icon: 'trash', danger: true, run: () => cmd('delete', { node }) },
    ];
  }

  $('#app').addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if (!S) return;
    const tabEl = e.target.closest('[data-tab]');
    const nodeEl = e.target.closest('[data-node]');
    const spaceEl = e.target.closest('[data-space]');
    let items;
    if (tabEl) items = tabMenu(tabOf(+tabEl.dataset.tab), tabEl.dataset.where);
    else if (nodeEl) {
      const [kind, id] = nodeEl.dataset.node.split(':');
      items = kind === 'live' ? liveMenu(nodeEl, +id) : folderMenu(nodeEl, +id);
    } else if (spaceEl) {
      const s = S.spaces.find((x) => x.id === +spaceEl.dataset.space);
      items = [{ label: 'Modifier l\'espace…', icon: 'edit', run: () => spaceDialog(s) }];
    } else if (e.target.closest('#scroll')) {
      items = [
        { label: 'Nouveau dossier', icon: 'folder', run: () => folderDialog() },
        { label: 'Nouveau Live Folder GitHub', icon: 'pr', run: () => liveDialog() },
        { label: 'Nouvel onglet', icon: 'plus', run: () => focusCommand(true) },
      ];
    }
    if (items) openMenu(items, e.clientX, e.clientY);
  });

  // --- dialogues ---------------------------------------------------------------------------------------------
  const dialog = $('#dialog');
  // `form.name` est l'attribut du formulaire, pas le champ : passer par elements.
  const val = (form, field) => (form.elements.namedItem(field)?.value || '').trim();
  function openDialog(html, onSubmit, setup) {
    closeMenus();
    dialog.innerHTML = `<form>${html}<div class="actions"><button type="button" data-cancel>Annuler</button><button class="primary">OK</button></div></form>`;
    dialog.hidden = false;
    hold();
    const form = $('form', dialog);
    const close = () => {
      dialog.hidden = true;
      release();
    };
    $('[data-cancel]', dialog).onclick = close;
    form.onsubmit = (e) => {
      e.preventDefault();
      if (onSubmit(form) !== false) close();
    };
    form.onkeydown = (e) => e.key === 'Escape' && close();
    if (setup) setup(form);
    const first = $('input', form);
    if (first) {
      first.focus();
      first.select();
    }
  }

  const TOKEN_URL = 'https://github.com/settings/tokens/new?scopes=repo,read:org&description=Voile';
  function tokenDialog() {
    openDialog(
      `<h3>Token GitHub</h3>
       <label>Personal access token, droits <code>repo</code> et <code>read:org</code>${S.ui.githubToken ? ' (vide : supprimer le token actuel)' : ''}</label>
       <input name="token" type="password" autocomplete="off" placeholder="ghp_…">
       <div class="presets" style="margin-top:6px"><button type="button" class="preset" data-new-token>Créer un token sur GitHub<small>github.com/settings/tokens</small></button></div>`,
      (f) => {
        const token = val(f, 'token');
        if (!token && !S.ui.githubToken) return false;
        ui('setGithubToken', { token });
      },
      (f) => ($('[data-new-token]', f).onclick = () => cmd('open', { input: TOKEN_URL, newTab: true })),
    );
  }

  function folderDialog() {
    openDialog('<h3>Nouveau dossier</h3><input name="name" value="Nouveau dossier">', (f) => {
      const name = val(f, 'name');
      if (!name) return false;
      cmd('createFolder', { name });
    });
  }

  const LIVE_PRESETS = [
    { name: 'À relire', query: 'is:pr is:open review-requested:@me archived:false', label: 'PR qui attendent ma review' },
    { name: 'Mes PR', query: 'is:pr is:open author:@me archived:false', label: 'Mes PR ouvertes' },
    { name: 'Mentions', query: 'is:open mentions:@me archived:false', label: 'Issues et PR qui me mentionnent' },
    { name: 'Assignées', query: 'is:open assignee:@me archived:false', label: 'Issues et PR qui me sont assignées' },
  ];
  function liveDialog() {
    const presets = LIVE_PRESETS.map(
      (p, i) => `<button type="button" class="preset" data-p="${i}">${esc(p.label)}<small>${esc(p.query)}</small></button>`,
    ).join('');
    openDialog(
      `<h3>Nouveau Live Folder GitHub</h3>
       <div class="presets">${presets}</div>
       <label>Nom</label><input name="name" placeholder="À relire">
       <label>Recherche GitHub (dépôt : <code>is:pr is:open repo:owner/nom</code>)</label>
       <input name="query" placeholder="is:pr is:open review-requested:@me">`,
      (f) => {
        const query = val(f, 'query');
        if (!query) return false;
        const name = val(f, 'name') || query.replace(/is:\S+|archived:\S+/g, '').trim() || 'GitHub';
        cmd('addLiveFolder', { name, source: { type: 'githubSearch', query } });
      },
      (f) =>
        $$('.preset', f).forEach((b) =>
          b.addEventListener('click', () => {
            const p = LIVE_PRESETS[+b.dataset.p];
            $$('.preset', f).forEach((x) => x.classList.toggle('on', x === b));
            f.elements.namedItem('name').value = p.name;
            f.elements.namedItem('query').value = p.query;
          }),
        ),
    );
  }

  const THEMES = [
    ['#f6d5c4', '#d9c6f2'], ['#c9dcf7', '#cfe9de'], ['#fde2a8', '#f7b9a3'], ['#cfe8c1', '#9fd4c9'],
    ['#e7d3f8', '#b7c4f5'], ['#f5c2d6', '#f7e1c8'], ['#2b2a3d', '#43325a'], ['#1f2b2a', '#2d4a43'],
  ];
  function spaceDialog(existing) {
    let theme = existing ? existing.theme : { from: THEMES[2][0], to: THEMES[2][1] };
    const swatches = THEMES.map(
      ([a, b], i) => `<button type="button" class="swatch${theme.from === a ? ' on' : ''}" data-t="${i}" style="background:linear-gradient(135deg,${a},${b})"></button>`,
    ).join('');
    openDialog(
      `<h3>${existing ? 'Modifier l\'espace' : 'Nouvel espace'}</h3>
       <label>Nom et icône</label>
       <div class="inline"><input name="icon" class="emoji" maxlength="4" value="${esc(existing ? existing.icon : '✨')}"><input name="name" value="${esc(existing ? existing.name : 'Nouvel espace')}"></div>
       <label>Couleurs</label><div class="swatches">${swatches}</div>
       <label>Dépôts GitHub des Live Folders (<code>owner/nom</code> ou <code>owner</code>, séparés par des espaces ; vide : tous)</label>
       <input name="repos" spellcheck="false" placeholder="getprimo mathieuletyrant/voile-browser" value="${esc(((existing && existing.githubRepos) || []).join(' '))}">`,
      (f) => {
        const name = val(f, 'name');
        const ic = val(f, 'icon') || '✨';
        if (!name) return false;
        const repos = val(f, 'repos').split(/[\s,]+/).filter(Boolean);
        // createSpace active le nouvel espace : setSpaceRepos sans `space` le vise.
        (existing ? cmd('updateSpace', { space: existing.id, name, icon: ic, theme }) : cmd('createSpace', { name, icon: ic, theme })).then(() =>
          cmd('setSpaceRepos', { space: existing ? existing.id : null, repos }),
        );
      },
      (f) =>
        $$('.swatch', f).forEach((b) =>
          b.addEventListener('click', () => {
            const [from, to] = THEMES[+b.dataset.t];
            theme = { from, to };
            $$('.swatch', f).forEach((x) => x.classList.toggle('on', x === b));
            applyTheme(theme);
          }),
        ),
    );
    // Le nom d'abord : c'est ce qu'on modifie le plus souvent.
    const name = $('input[name=name]', dialog);
    name.focus();
    name.select();
  }

  // --- glisser-déposer --------------------------------------------------------------------------------------------
  let drag = null; // { key: 'tab:3', kind, id }
  let dropTarget = null; // { location, el, cls }

  $('#app').addEventListener('dragstart', (e) => {
    const el = e.target.closest('[draggable="true"]');
    if (!el || !S) return;
    const key = el.dataset.item || el.dataset.node;
    const [kind, id] = key.split(':');
    drag = { key, kind, id: +id, el };
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', kind === 'tab' ? tabOf(+id)?.url || key : key);
    hold();
    requestAnimationFrame(() => (el.closest('[data-item]') || el).classList.add('dragging'));
  });

  function clearDrop() {
    if (dropTarget) dropTarget.el.classList.remove(dropTarget.cls);
    dropTarget = null;
  }

  function containerRef(el) {
    if (el.id === 'favorites') return { kind: 'favorites' };
    if (el.id === 'pinned') return { kind: 'pinned' };
    if (el.id === 'today') return { kind: 'today' };
    const c = el.dataset.container;
    if (c && c.startsWith('folder:')) return { kind: 'folder', id: +c.slice(7) };
    return null;
  }

  // Index d'insertion parmi les éléments du conteneur, sans l'élément déplacé.
  function indexIn(container, before) {
    const items = [...container.children].filter((c) => c.dataset.item && c.dataset.item !== drag.key);
    if (!before) return items.length;
    const i = items.indexOf(before);
    return i < 0 ? items.length : i;
  }

  function computeDrop(e) {
    const sp = e.target.closest('[data-space]');
    if (sp) {
      const container = drag.kind === 'tab' ? { kind: 'today' } : { kind: 'pinned' };
      return { location: { space: +sp.dataset.space, container, index: drag.kind === 'tab' ? 0 : 9999 }, el: sp, cls: 'drop-into' };
    }
    if (e.target.closest('#new-tab') && drag.kind === 'tab') {
      return { location: { space: S.activeSpace, container: { kind: 'today' }, index: 0 }, el: $('#new-tab'), cls: 'drop-after' };
    }
    // Déposer *dans* un dossier fermé ou sur le milieu de son en-tête.
    const head = e.target.closest('[data-drop-into]');
    if (head && head.dataset.dropInto !== drag.key) {
      const r = head.getBoundingClientRect();
      const y = (e.clientY - r.top) / r.height;
      if (y > 0.25 && y < 0.75) {
        const id = +head.dataset.dropInto.split(':')[1];
        if (!$(`[data-item="${drag.key}"] [data-container="folder:${id}"]`)) {
          return { location: { space: S.activeSpace, container: { kind: 'folder', id }, index: 9999 }, el: head, cls: 'drop-into' };
        }
      }
    }
    const item = e.target.closest('[data-item]');
    const container = (item ? item.parentElement : e.target).closest('#favorites, #pinned, #today, [data-container]');
    if (!container) return null;
    const ref = containerRef(container);
    if (!ref) return null;
    if ((ref.kind === 'favorites' || ref.kind === 'today') && drag.kind !== 'tab') return null;
    if (item && item.dataset.item === drag.key) return null;
    if (item && item.parentElement === container) {
      const r = item.getBoundingClientRect();
      const horizontal = ref.kind === 'favorites';
      const after = horizontal ? e.clientX > r.left + r.width / 2 : e.clientY > r.top + r.height / 2;
      const items = [...container.children].filter((c) => c.dataset.item && c.dataset.item !== drag.key);
      const next = after ? items[items.indexOf(item) + 1] : item;
      return {
        location: { space: S.activeSpace, container: ref, index: indexIn(container, next) },
        el: item.matches('.folder') ? item.firstElementChild : item,
        cls: after ? 'drop-after' : 'drop-before',
      };
    }
    container.classList.add('drop-zone');
    return { location: { space: S.activeSpace, container: ref, index: indexIn(container, null) }, el: container, cls: 'drop-into' };
  }

  $('#app').addEventListener('dragover', (e) => {
    if (!drag) return;
    const t = computeDrop(e);
    if (!t) {
      clearDrop();
      return;
    }
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    if (!dropTarget || dropTarget.el !== t.el || dropTarget.cls !== t.cls) {
      clearDrop();
      t.el.classList.add(t.cls);
    }
    dropTarget = t;
  });

  $('#app').addEventListener('drop', (e) => {
    if (!drag || !dropTarget) return;
    e.preventDefault();
    const node = { kind: drag.kind, id: drag.id };
    cmd('move', { node, to: dropTarget.location });
  });

  $('#app').addEventListener('dragend', () => {
    clearDrop();
    $$('.dragging, .drop-zone').forEach((x) => x.classList.remove('dragging', 'drop-zone'));
    drag = null;
    release();
  });

  // --- redimensionnement -----------------------------------------------------------------------------------------------
  const handle = $('#resize');
  handle.addEventListener('pointerdown', (e) => {
    handle.setPointerCapture(e.pointerId);
    handle.classList.add('on');
    let frame = 0;
    const move = (ev) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => ui('setSidebarWidth', { width: Math.round(ev.clientX + 3) }));
    };
    const up = () => {
      handle.classList.remove('on');
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
  });

  // --- raccourcis (quand la sidebar a le focus) -------------------------------------------------------------------------
  document.addEventListener('keydown', (e) => {
    if (!S) return;
    const typing = e.target.matches('input, textarea');
    const mod = modKey(e);
    const tab = S.activeTab;
    const k = e.key.toLowerCase();
    let handled = true;
    if (mod && !e.shiftKey && k === 't') focusCommand(true);
    else if (mod && !e.shiftKey && k === 'l') focusCommand(false);
    else if (mod && !e.shiftKey && k === 'w' && tab != null) cmd('close', { tab });
    else if (mod && !e.shiftKey && k === 'r' && tab != null) cmd('reload', { tab });
    else if (mod && k === '[' && tab != null) cmd('back', { tab });
    else if (mod && k === ']' && tab != null) cmd('forward', { tab });
    else if (mod && /^[1-9]$/.test(e.key)) ui('selectIndex', { index: e.key === '9' ? 1e9 : +e.key - 1 });
    else if (e.ctrlKey && e.key === 'Tab') ui('cycleTab', { delta: e.shiftKey ? -1 : 1 });
    else if (mod && e.shiftKey && k === 't') ui('reopenClosed');
    else if (mod && e.shiftKey && k === 'c') ui('copyUrl');
    else if (mod && e.shiftKey && k === 'd') ui('togglePin');
    else if (e.key === 'Escape' && !typing && !commandOpen) {
      closeMenus();
      ui('focusPage');
    } else handled = false;
    if (handled) e.preventDefault();
  });

  // --- presse-papiers ----------------------------------------------------------------------------------------------------
  function copy(text) {
    const done = () => toast('URL copiée');
    if (navigator.clipboard) {
      navigator.clipboard.writeText(text).then(done, () => legacyCopy(text) && done());
    } else if (legacyCopy(text)) done();
  }
  function legacyCopy(text) {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.append(ta);
    ta.select();
    const ok = document.execCommand('copy');
    ta.remove();
    return ok;
  }

  // --- point d'entrée -----------------------------------------------------------------------------------------------------
  window.voile = { render, focusCommand, copy };
  hydrateIcons();
  $('#new-tab-kbd').textContent = kbd('T');
  ui('ready', { frame: FRAME });
})();
