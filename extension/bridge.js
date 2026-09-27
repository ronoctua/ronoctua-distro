// The kiosk's bridge: what the footer sees of a kiosk window, and what it
// may ask of one.
//
// WHAT A KIOSK IS. An ordinary window of the person's Chrome whose tab strip
// and toolbar the COMPOSITOR hides (it tells the window it is fullscreen
// while its tile keeps its size). The footer then draws the tabs, the
// extensions and the zoom in the modeline — and everything it draws comes
// from here, and everything it does goes through here.
//
// HOW ONE IS BORN. The footer runs Chrome with `--new-window` and our
// `kiosk.html`, a nonce in its fragment. The page puts the nonce in its title
// (the compositor sees it) and says `kiosk-born` to this worker (the footer
// hears it, through the host). The footer pairs the two halves, hides the
// bars, and orders `go`; only then does the page leave for its address. With
// no footer to pair it, the page leaves at once: a kiosk is never a blank
// window waiting for somebody who is not there.
//
// THE PORT EXISTS ONLY WHILE A KIOSK DOES. `connectNative` starts the host
// (`ronoctua-browser`), which connects to the footer's socket; an open port
// keeps this worker alive, so it is opened for the first kiosk and closed
// with the last. The `tabs.*` and `windows.*` listeners are added on connect
// and removed on disconnect, so ordinary browsing never wakes the worker.
//
// WHOLE STATE, NEVER A DELTA — the rule every daemon of this system keeps. A
// burst of events is ONE message: a flag and a microtask, never a timer.
//
// NARROW ON PURPOSE. Nothing here reads a page, runs script in one, or turns
// an extension on, off or away: `management` is asked for the list and
// nothing else.

import { BROWSER_HOST } from './generated.js';
import { NEW_TAB, allowed, extensionPage } from './address.js';

// The wire's version (`ronoctua_browser::wire::VERSION`): the footer draws a
// refusal instead of guessing when the two differ.
const VERSION = 1;

// Where the kiosks are kept between two lives of this worker (an extension
// update restarts it). `storage.local` and NOT `storage.session`, which is
// wiped by every update — the moment a kiosk most needs to be remembered.
const KEPT = 'kiosks';

// How long a kiosk page waits for the footer's `go` before it leaves on its
// own: the footer's own timer is the first answer, this is the one for a page
// nobody launched (a bookmark, another extension). Armed only while a page
// waits.
const PATIENCE_MS = 30000;

// A HOST THAT ENDED while kiosks are still open (it crashed, or refused a
// frame) is started again — after a pause that doubles each time, from the
// first to the last of these, so a host that dies at once is not restarted
// in a loop. A port that lived longer than the last pause resets it. The
// last stays under the half-minute an idle worker is given: a timer past it
// may never ring, and the next wake reconnects anyway (`ready`).
const FIRST_RETRY_MS = 1000;
const LAST_RETRY_MS = 16000;

// windowId -> { nonce, profile, born, restored }
const kiosks = new Map();
// nonce -> { reply, window, tab, url, timer }
const waiting = new Map();

let port = null;
// What the host last said about the footer: `true`, `false`, or `null` while
// it has not said (or no port is open).
let footerUp = null;
// When the port was opened, the last pause before opening it again, and the
// timer of the next try — armed only while kiosks wait for a port.
let openedAt = 0;
let retryMs = 0;
let retryTimer = null;

// ---- remembered kiosks ----------------------------------------------------

function keep() {
  const out = {};
  for (const [window, k] of kiosks) out[window] = { nonce: k.nonce, profile: k.profile };
  return chrome.storage.local.set({ [KEPT]: out });
}

// A worker that starts with kiosks remembered (an update restarted it while
// one was open) opens the port again, and the footer sees them without
// anybody asking.
const ready = (async () => {
  const got = await chrome.storage.local.get(KEPT);
  for (const [window, k] of Object.entries(got[KEPT] || {})) {
    kiosks.set(Number(window), { nonce: k.nonce, profile: k.profile, born: false, restored: true });
  }
  if (kiosks.size) connect();
})();

// A NEW BROWSER: a window id of the last session names some other window
// now. Only what THIS life of the worker remembered from storage is dropped —
// a kiosk page that announced itself before this event arrived is real.
chrome.runtime.onStartup.addListener(async () => {
  await ready;
  for (const [window, k] of kiosks) if (k.restored) kiosks.delete(window);
  await keep();
  if (!kiosks.size && !waiting.size) hangUp();
});

// ---- the state -----------------------------------------------------------

async function tabsOf(window) {
  const tabs = await chrome.tabs.query({ windowId: window });
  return Promise.all(tabs.map(async (t) => {
    let zoom = 1;
    try {
      zoom = await chrome.tabs.getZoom(t.id);
    } catch (e) {
      // A tab that went away between the query and this line.
    }
    return {
      id: t.id,
      index: t.index,
      active: t.active === true,
      pinned: t.pinned === true,
      title: t.title || '',
      url: t.url || t.pendingUrl || '',
      loading: t.status === 'loading',
      audible: t.audible === true,
      muted: (t.mutedInfo && t.mutedInfo.muted) === true,
      zoom_percent: Math.round(zoom * 100),
    };
  }));
}

async function snapshot() {
  const out = [];
  const gone = [];
  for (const [window, k] of kiosks) {
    let state = '';
    try {
      state = (await chrome.windows.get(window)).state || '';
    } catch (e) {
      // THE WINDOW IS GONE, and nobody said so: it closed while no port was
      // open, and `windows.onRemoved` is armed only with one. Forgotten here,
      // or a dead id would keep the port — the host and this worker — alive
      // until Chrome restarts.
      gone.push(window);
      continue;
    }
    out.push({
      window,
      nonce: k.nonce,
      profile: k.profile,
      born: k.born === true,
      state,
      tabs: await tabsOf(window),
    });
  }
  // Every extension of this profile, ours included (the footer leaves ours
  // out). The list, and nothing done with it.
  const all = await chrome.management.getAll();
  const extensions = all
    .filter((e) => e.type === 'extension')
    .map((e) => ({
      id: e.id,
      name: e.name,
      version: e.version,
      enabled: e.enabled === true,
      options_url: e.optionsUrl || null,
    }));
  let focused = null;
  try {
    const last = await chrome.windows.getLastFocused();
    if (last && last.focused) focused = last.id;
  } catch (e) {
    // No window at all.
  }
  if (gone.length) {
    for (const window of gone) kiosks.delete(window);
    await keep();
  }
  const first = kiosks.values().next().value;
  return {
    said: 'state',
    v: VERSION,
    profile: first ? first.profile : null,
    kiosks: out,
    extensions,
    focused_window: focused,
  };
}

// ONE MESSAGE PER BURST. What arrives while a state is being gathered sets
// `again`, and the loop sends once more with everything — never one message
// per event, and never a timer.
let sending = false;
let again = false;
function touch() {
  if (!port) return;
  if (sending) {
    again = true;
    return;
  }
  sending = true;
  queueMicrotask(async () => {
    try {
      do {
        again = false;
        const state = await snapshot();
        if (port) port.postMessage(state);
      } while (again && port);
      // WHAT THE SNAPSHOT FOUND GONE may have been the last kiosk: the port
      // goes with it, after the state that says so.
      if (!kiosks.size && !waiting.size) hangUp();
    } catch (e) {
      console.warn('[ronoctua] could not gather the kiosk state', e);
    } finally {
      sending = false;
    }
  });
}

// ---- the events, only while the port is open ------------------------------

const isKiosk = (window) => kiosks.has(window);
const onWindow = (window) => { if (isKiosk(window)) touch(); };

const LISTENERS = [
  [chrome.tabs.onCreated, (tab) => onWindow(tab.windowId)],
  [chrome.tabs.onRemoved, (_id, info) => onWindow(info.windowId)],
  [chrome.tabs.onUpdated, (_id, _change, tab) => onWindow(tab.windowId)],
  [chrome.tabs.onMoved, (_id, info) => onWindow(info.windowId)],
  [chrome.tabs.onActivated, (info) => onWindow(info.windowId)],
  [chrome.tabs.onAttached, (_id, info) => onWindow(info.newWindowId)],
  [chrome.tabs.onDetached, (_id, info) => onWindow(info.oldWindowId)],
  [chrome.tabs.onReplaced, () => touch()],
  [chrome.tabs.onZoomChange, () => touch()],
  [chrome.windows.onFocusChanged, () => touch()],
  [chrome.windows.onBoundsChanged, (window) => onWindow(window.id)],
  [chrome.windows.onRemoved, (window) => forget(window)],
  [chrome.management.onEnabled, () => touch()],
  [chrome.management.onDisabled, () => touch()],
  [chrome.management.onInstalled, () => touch()],
  [chrome.management.onUninstalled, () => touch()],
];

function connect() {
  if (port) return;
  let opened;
  try {
    opened = chrome.runtime.connectNative(BROWSER_HOST);
  } catch (e) {
    console.warn('[ronoctua] the browser bridge is not installed', e);
    unavailable();
    return;
  }
  port = opened;
  footerUp = null;
  openedAt = Date.now();
  opened.onMessage.addListener(heard);
  opened.onDisconnect.addListener(() => {
    // The host went away (not installed, or it ended): nobody can pair a
    // waiting page any more. Only THIS port's end: a hang-up of ours never
    // fires here, and a later port is not this one.
    console.warn('[ronoctua] the browser bridge closed', chrome.runtime.lastError);
    if (port !== opened) return;
    unplug();
    unavailable();
    reconnectLater();
  });
  for (const [event, listener] of LISTENERS) event.addListener(listener);
  touch();
}

// The kiosks that are left are given a port again, after a pause — see
// `FIRST_RETRY_MS`. Nothing is armed when no kiosk is left to need one.
function reconnectLater() {
  if (retryTimer !== null || !kiosks.size) return;
  const lived = Date.now() - openedAt;
  retryMs = lived > LAST_RETRY_MS
    ? FIRST_RETRY_MS
    : Math.min(Math.max(retryMs * 2, FIRST_RETRY_MS), LAST_RETRY_MS);
  retryTimer = setTimeout(() => {
    retryTimer = null;
    if (kiosks.size && !port) connect();
  }, retryMs);
}

// Removes the listeners and forgets the port, on either side's hang-up.
function unplug() {
  for (const [event, listener] of LISTENERS) event.removeListener(listener);
  port = null;
  footerUp = null;
}

// Our side ends the port: the last kiosk is gone. The host sees its stdin
// end, tells the footer, and exits.
function hangUp() {
  if (!port) return;
  const closing = port;
  unplug();
  closing.disconnect();
}

// Every waiting page leaves at once: there is nobody to pair it.
function unavailable() {
  for (const nonce of [...waiting.keys()]) go(nonce, false);
}

// The window stops being a kiosk: closed, or released by the footer. With
// `close`, the port goes with the last one — after the state that says so,
// which the host relays before it sees the end of its input.
async function forget(window, close = true) {
  if (!kiosks.delete(window)) return;
  for (const [nonce, w] of waiting) {
    if (w.window !== window) continue;
    clearTimeout(w.timer);
    waiting.delete(nonce);
  }
  await keep();
  if (!port) return;
  if (kiosks.size || waiting.size || !close) {
    touch();
    return;
  }
  try {
    port.postMessage(await snapshot());
  } catch (e) {
    console.warn('[ronoctua] could not say goodbye to the footer', e);
  }
  if (!kiosks.size && !waiting.size) hangUp();
}

// ---- a kiosk page is born ------------------------------------------------

// The page's `kiosk-born`, answered only when it may leave: `{go: url}`, or
// nothing at all when it is replaced by a new-tab page.
export async function kioskBorn(message, sender, reply) {
  const tab = sender && sender.tab;
  const nonce = String((message && message.nonce) || '');
  if (!tab || !/^[0-9a-f]{32}$/.test(nonce) || sender.url.split('#')[0] !== chrome.runtime.getURL('kiosk.html')) {
    reply({});
    return;
  }
  await ready;
  const url = allowed(message.url) ? message.url : '';
  const known = [...kiosks.values()].find((k) => k.nonce === nonce);
  if (known && !waiting.has(nonce)) {
    // Announced again after it was paired (a reload): it goes, and is not
    // paired twice.
    reply(url ? { go: url } : {});
    return;
  }
  const previous = waiting.get(nonce);
  if (previous) clearTimeout(previous.timer);
  const timer = setTimeout(() => go(nonce, false), PATIENCE_MS);
  waiting.set(nonce, { reply, window: tab.windowId, tab: tab.id, url, timer });
  kiosks.set(tab.windowId, {
    nonce,
    profile: String(message.profile || ''),
    born: true,
    restored: false,
  });
  await keep();
  if (footerUp === false) {
    go(nonce, false);
    return;
  }
  connect();
  touch();
}

// The page leaves: for its address, or — with none — the window gets
// Chrome's new-tab page and the kiosk tab closes. `paired` keeps the window
// as a kiosk; an unpaired one becomes an ordinary window and is forgotten.
async function go(nonce, paired) {
  const w = waiting.get(nonce);
  if (!w) return false;
  waiting.delete(nonce);
  clearTimeout(w.timer);
  const k = kiosks.get(w.window);
  if (paired && k) {
    k.born = false;
  } else {
    kiosks.delete(w.window);
  }
  await keep();
  try {
    if (w.url) {
      w.reply({ go: w.url });
    } else {
      await chrome.tabs.create({ windowId: w.window, active: true });
      await chrome.tabs.remove(w.tab);
    }
  } catch (e) {
    console.warn('[ronoctua] the kiosk page could not leave', e);
  }
  if (port) {
    if (kiosks.size || waiting.size) touch();
    else hangUp();
  }
  return true;
}

// ---- what the host says --------------------------------------------------

// The tail of the orders' line: each order waits for the one before it.
let obeying = Promise.resolve();

function heard(message) {
  if (!message || typeof message !== 'object') return;
  if (message.said === 'footer') {
    footerUp = message.up === true;
    if (!footerUp) unavailable();
    return;
  }
  if (typeof message.serial === 'number') {
    // ONE ORDER AT A TIME, in the order the footer sent them. Each order
    // awaits Chrome, and two run side by side would finish in whatever
    // order Chrome answers: `activate` asks for its tab before it acts
    // while `create` acts at once, so [activate X, create] left X in front.
    obeying = obeying
      .then(() => obey(message))
      .then(
        (why) => answer(message.serial, why),
        (e) => answer(message.serial, (e && e.message) || String(e)),
      )
      // A release of the last kiosk is answered BEFORE the port closes.
      .then(() => {
        if (!kiosks.size && !waiting.size) hangUp();
      })
      // AND THE LINE GOES ON: an order that could not even be answered does
      // not stop the ones behind it.
      .catch((e) => console.warn('[ronoctua] an order of the footer was lost', e));
  }
}

function answer(serial, why) {
  if (!port) return;
  const done = { said: 'done', serial, ok: why === null };
  if (why !== null) done.why = why;
  port.postMessage(done);
}

// The window a tab lives in, when it is a kiosk's; `null` otherwise — no
// order reaches a tab outside a kiosk.
async function kioskTab(id) {
  try {
    const tab = await chrome.tabs.get(id);
    return isKiosk(tab.windowId) ? tab : null;
  } catch (e) {
    return null;
  }
}

// One order; `null` when done, or the reason it was not.
async function obey(m) {
  switch (m.order) {
    case 'go':
      return (await go(String(m.nonce || ''), true)) ? null : 'no-tab';
    case 'look':
      touch();
      return null;
    case 'release':
      if (!isKiosk(m.window)) return 'no-tab';
      await forget(m.window, false);
      return null;
    case 'create': {
      if (!isKiosk(m.window)) return 'no-tab';
      const url = m.url == null ? NEW_TAB : m.url;
      if (!allowed(url)) return 'refused-url';
      await chrome.tabs.create({ windowId: m.window, url, active: true });
      return null;
    }
    case 'open-extension':
      if (!isKiosk(m.window)) return 'no-tab';
      if (!extensionPage(m.url)) return 'refused-url';
      await chrome.tabs.create({ windowId: m.window, url: m.url, active: true });
      return null;
    default:
      break;
  }
  // Every other order names a tab, and the tab must be a kiosk's.
  const tab = await kioskTab(m.tab);
  if (!tab) return 'no-tab';
  switch (m.order) {
    case 'activate':
      await chrome.tabs.update(tab.id, { active: true });
      return null;
    case 'close':
      await chrome.tabs.remove(tab.id);
      return null;
    case 'move':
      await chrome.tabs.move(tab.id, { index: Number(m.index) });
      return null;
    case 'navigate':
      if (!allowed(m.url)) return 'refused-url';
      await chrome.tabs.update(tab.id, { url: m.url });
      return null;
    case 'search': {
      // In a new tab of the SAME window, made first: `search.query` with a
      // disposition opens where the focus is, which is a guess.
      const where = m.new_tab
        ? (await chrome.tabs.create({ windowId: tab.windowId, active: true })).id
        : tab.id;
      await chrome.search.query({ text: String(m.text || ''), tabId: where });
      return null;
    }
    case 'back':
      try {
        await chrome.tabs.goBack(tab.id);
      } catch (e) {
        return 'no-history';
      }
      return null;
    case 'forward':
      try {
        await chrome.tabs.goForward(tab.id);
      } catch (e) {
        return 'no-future';
      }
      return null;
    case 'reload':
      await chrome.tabs.reload(tab.id);
      return null;
    case 'zoom':
      await chrome.tabs.setZoom(tab.id, Number(m.percent) / 100);
      return null;
    default:
      return 'refused-order';
  }
}
