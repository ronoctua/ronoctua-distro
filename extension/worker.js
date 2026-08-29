// The extension's whole brain — and it never touches a page.
//
// WHAT MAKES THIS INVISIBLE TO A WEBSITE. An extension is normally detectable
// in five ways, and every one of them is a thing this file and the manifest do
// NOT do:
//
//   1. a web-accessible resource a page can `fetch` from `chrome-extension://`;
//   2. a content script that leaves anything in the DOM;
//   3. `externally_connectable`, which hands a page `chrome.runtime`;
//   4. a standing host permission, which is what makes 2 possible everywhere;
//   5. a `declarativeNetRequest` rule, which a page infers from its own headers.
//
// The manifest declares none of the five, and `extension-has-no-fingerprint`
// fails the build if one ever appears. What is left runs HERE, in the service
// worker, where page JavaScript cannot see it at all.
//
// The one thing a page can still infer is behaviour: a download it started that
// never appears. That is not fixable from inside an extension, and it is said
// out loud rather than papered over.

import { TEXT, HOST } from './generated.js';

// Whether a download the browser starts is handed over. Kept in `storage.sync`
// so it is one answer per person and not per machine.
const SWITCH = 'intercept';

// AND WHETHER WHAT LEAVES CARRIES WHO YOU ARE.
//
// This is the ONE place in the whole system where identity is a setting rather
// than a key, and the reason is interception: a download the browser started
// has no moment at which anybody can be asked. So it is a switch — shown in
// the popup, right above the buttons it governs, so that its state is visible
// at the moment it is acted on rather than remembered.
//
// At the keyboard there is no such compromise: `LEADER t v` and `LEADER t i v`
// are different keys, and the sequence itself says which happened.
const AS_ME = 'identity';

// What we will take over. A `blob:` or `data:` URL exists only inside the tab
// and there is nothing to hand anybody; `ftp:` and the rest are not aria2's
// business here either.
const OURS = /^https?:\/\//i;

async function intercepting() {
  const kept = await chrome.storage.sync.get(SWITCH);
  // On by default: the owner asked for the browser to HAND downloads over, so
  // the surprising state is the one that has to be chosen.
  return kept[SWITCH] !== false;
}

// OFF by default, and that direction is not symmetric with the one above:
// anonymous is what costs nothing and can always be repeated as oneself. The
// reverse is not true — a request that already carried who was asking cannot
// be taken back.
async function identity() {
  const kept = await chrome.storage.sync.get(AS_ME);
  return kept[AS_ME] === true ? 'mine' : 'anonymous';
}

// Says something to the queue, and reports whether it took it.
//
// `sendNativeMessage` and not a long-lived port: each message starts the host,
// which adds a row and exits. A port would keep a process alive for the life of
// the browser to do nothing between clicks.
async function tell(message) {
  try {
    const answer = await chrome.runtime.sendNativeMessage(HOST, message);
    return answer && answer.ok === true;
  } catch (e) {
    console.warn('[ronoctua] the queue did not answer', e);
    return false;
  }
}

// THE INTERCEPTION.
//
// `onCreated` is the earliest point at which the URL is known and the download
// can still be stopped. Cancelling leaves a cancelled entry in the browser's
// own list, so it is erased too — otherwise every handover leaves a red line
// behind that looks like a failure.
chrome.downloads.onCreated.addListener(async (item) => {
  if (!item || !OURS.test(item.url || '')) return;
  if (!(await intercepting())) return;

  const taken = await tell({ kind: 'fetch', url: item.url, identity: await identity() });
  if (!taken) {
    // THE QUEUE IS NOT UP, so the browser keeps the download. Cancelling first
    // and asking afterwards would lose the file whenever the unit is down,
    // which is the one outcome worse than not intercepting at all.
    note(TEXT.extQueueDown);
    return;
  }
  try {
    await chrome.downloads.cancel(item.id);
    await chrome.downloads.erase({ id: item.id });
  } catch (e) {
    console.warn('[ronoctua] could not clear the browser download', e);
  }
  note(TEXT.extSent);
});

// A short word on the toolbar button, which is the only surface this extension
// has. Not a notification: invariant 14 has no floating popups, and the badge
// is the browser's own chrome rather than ours.
let clearing = null;
function note(text) {
  chrome.action.setTitle({ title: `${TEXT.extTitle} — ${text}` });
  if (clearing) clearTimeout(clearing);
  clearing = setTimeout(() => chrome.action.setTitle({ title: TEXT.extTitle }), 4000);
}

// THE CONTEXT MENU. Its titles SAY which of the two they will do, because a
// right-click has no room for a choice and a menu that did not say would be the
// worst place in the system to be silent about it.
function buildMenu(who) {
  const mark = who === 'mine' ? ` — ${TEXT.extAsMe}` : '';
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: 'send-link',
      title: TEXT.extSendLink + mark,
      contexts: ['link'],
    });
    chrome.contextMenus.create({
      id: 'send-media',
      title: TEXT.extSendMedia + mark,
      contexts: ['page', 'video', 'audio'],
    });
  });
}

chrome.runtime.onInstalled.addListener(async () => {
  buildMenu(await identity());
  chrome.action.setTitle({ title: TEXT.extTitle });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === 'send-link' && info.linkUrl) {
    const sent = await tell({ kind: 'fetch', url: info.linkUrl, identity: await identity() });
    note(sent ? TEXT.extSent : TEXT.extFailed);
    return;
  }
  if (info.menuItemId === 'send-media') {
    // THE PAGE'S ADDRESS, and nothing read out of the page. yt-dlp resolves a
    // page into its media itself, for about two thousand sites — which is what
    // lets the common case need no injection at all.
    const url = (info.srcUrl && /^https?:/i.test(info.srcUrl)) ? info.srcUrl : (tab && tab.url);
    if (!url) return;
    const sent = await tell({ kind: 'media', url, want: 'video', identity: await identity() });
    note(sent ? TEXT.extSent : TEXT.extFailed);
  }
});

// What the popup asks for. The popup is part of the extension, so this is not
// a door onto the outside: `externally_connectable` is absent, and no page can
// reach it.
chrome.runtime.onMessage.addListener((message, _sender, reply) => {
  (async () => {
    switch (message && message.kind) {
      case 'state':
        reply({
          up: await tell({ kind: 'ping' }),
          intercept: await intercepting(),
          identity: await identity(),
        });
        return;
      case 'toggle': {
        const now = !(await intercepting());
        await chrome.storage.sync.set({ [SWITCH]: now });
        reply({ intercept: now });
        return;
      }
      case 'toggle-identity': {
        const now = (await identity()) !== 'mine';
        await chrome.storage.sync.set({ [AS_ME]: now });
        // The context menu says which of the two it will do, so it is rebuilt
        // whenever the answer changes — a menu entry that lied about this
        // would be the worst place in the system to lie.
        buildMenu(now ? 'mine' : 'anonymous');
        reply({ identity: now ? 'mine' : 'anonymous' });
        return;
      }
      case 'media':
        reply({
          ok: await tell({
            kind: 'media',
            url: message.url,
            want: message.want,
            identity: await identity(),
          }),
        });
        return;
      case 'link':
        reply({ ok: await tell({ kind: 'fetch', url: message.url, identity: await identity() }) });
        return;
      case 'scan': {
        // THE ONLY INJECTION IN THIS EXTENSION, and it happens because the
        // person pressed a button on this tab. `activeTab` grants access to
        // this one tab until it navigates, and nothing is left behind: the
        // function reads and returns.
        try {
          const [found] = await chrome.scripting.executeScript({
            target: { tabId: message.tabId },
            files: ['find-media.js'],
          });
          reply({ media: (found && found.result) || [] });
        } catch (e) {
          console.warn('[ronoctua] could not look at the tab', e);
          reply({ media: [] });
        }
        return;
      }
      default:
        reply({});
    }
  })();
  // `true`: the answer comes later. Without it the channel closes and every
  // reply above is lost — silently, which is the worst way to lose one.
  return true;
});
