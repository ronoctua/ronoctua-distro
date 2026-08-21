// The one surface this extension has, and it is built from tokens and keys.
//
// NO TEXT AND NO COLOUR IS WRITTEN HERE. Invariants 8 and 9 do not stop at the
// browser's edge: every word comes from `assets/i18n/*.ftl` and every colour
// from `assets/tokens/colors.nix`, both of them through `generated.js`, which
// the flake writes. `popup.html` carries no visible text for the same reason —
// the DOM is built below, from keys.

import { TEXT, COLOUR, SPACE } from './generated.js';

const root = document.getElementById('root');

function element(tag, style, text) {
  const node = document.createElement(tag);
  if (style) Object.assign(node.style, style);
  if (text !== undefined) node.textContent = text;
  return node;
}

// A row that does something. Flat: no card, no shadow, no elevation —
// invariant 15 reaches here too.
function button(label, hint, run) {
  const node = element('button', {
    display: 'block',
    width: '100%',
    textAlign: 'left',
    background: 'transparent',
    color: COLOUR.textPrimary,
    border: 'none',
    borderRadius: '0',
    padding: `${SPACE.tight}px ${SPACE.gap}px`,
    font: 'inherit',
    cursor: 'pointer',
  });
  node.appendChild(element('div', { color: COLOUR.textPrimary }, label));
  if (hint) node.appendChild(element('div', { color: COLOUR.textDim, fontSize: '11px' }, hint));
  node.addEventListener('mouseenter', () => { node.style.background = COLOUR.surfaceSelection; });
  node.addEventListener('mouseleave', () => { node.style.background = 'transparent'; });
  node.addEventListener('click', run);
  return node;
}

function rule() {
  return element('div', {
    height: '1px',
    background: COLOUR.ruleDefault,
    margin: `${SPACE.tight}px 0`,
  });
}

function say(node, text, colour) {
  node.textContent = text;
  node.style.color = colour;
}

async function ask(message) {
  return await chrome.runtime.sendMessage(message);
}

async function draw() {
  root.textContent = '';
  Object.assign(document.body.style, {
    background: COLOUR.surfaceBase,
    color: COLOUR.textPrimary,
    margin: '0',
    minWidth: '260px',
    font: '13px system-ui, sans-serif',
  });

  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const state = await ask({ kind: 'state' });

  const header = element('div', {
    padding: `${SPACE.gap}px`,
    color: COLOUR.stateAccent,
    fontWeight: '600',
  }, TEXT.extTitle);
  root.appendChild(header);

  const status = element('div', { padding: `0 ${SPACE.gap}px ${SPACE.tight}px` });
  say(status,
      state.up ? TEXT.extQueueUp : TEXT.extQueueDown,
      state.up ? COLOUR.stateSuccess : COLOUR.stateError);
  root.appendChild(status);

  // WHO IS ASKING, ABOVE THE BUTTONS IT GOVERNS.
  //
  // It is a switch and not a second row of buttons because interception has no
  // moment at which anybody can be asked — and a switch whose state is not on
  // screen where it is acted on is exactly the invisible cost this whole
  // design avoids. Warning-coloured when it is on, because what it costs is
  // anonymity.
  const mine = state.identity === 'mine';
  const who = button(
    mine ? TEXT.extAsMe : TEXT.extAnonymous,
    null,
    async () => { await ask({ kind: 'toggle-identity' }); await draw(); },
  );
  who.firstChild.style.color = mine ? COLOUR.stateWarning : COLOUR.textSecondary;
  root.appendChild(who);
  root.appendChild(rule());

  const page = tab && tab.url && /^https?:/i.test(tab.url) ? tab.url : null;

  if (page) {
    // THE THREE THE OWNER ASKED FOR. Each one hands yt-dlp the PAGE, which is
    // what it resolves — no injection, and nothing read out of the document.
    //
    // WRITTEN AS `want: '...'`, and that spelling is load-bearing:
    // `downloads-do-not-need-the-extension` reads this file to find out what
    // the extension can ask for, and demands a key for each. Anything written
    // another way is a thing the extension can do and the keyboard cannot,
    // which is the whole failure that check exists to catch.
    for (const item of [
      { label: TEXT.extVideo, want: 'video' },
      { label: TEXT.extAudio, want: 'audio' },
      { label: TEXT.extSubtitles, want: 'subtitles' },
    ]) {
      root.appendChild(button(item.label, null, async () => {
        const out = await ask({ kind: 'media', url: page, want: item.want });
        say(status, out.ok ? TEXT.extSent : TEXT.extFailed,
            out.ok ? COLOUR.stateSuccess : COLOUR.stateError);
      }));
    }
    root.appendChild(rule());

    // THE FALLBACK, and it is a button and not a default: this is the only
    // thing here that runs anything inside the tab, so it happens when it is
    // asked for and never before.
    root.appendChild(button(TEXT.extScan, null, async () => {
      const out = await ask({ kind: 'scan', tabId: tab.id });
      const found = out.media || [];
      if (found.length === 0) {
        say(status, TEXT.extFoundNone, COLOUR.textDim);
        return;
      }
      root.appendChild(rule());
      for (const item of found) {
        const name = item.url.split('/').pop().split('?')[0] || item.url;
        root.appendChild(button(name, item.kind, async () => {
          const sent = await ask({ kind: 'link', url: item.url });
          say(status, sent.ok ? TEXT.extSent : TEXT.extFailed,
              sent.ok ? COLOUR.stateSuccess : COLOUR.stateError);
        }));
      }
    }));
  } else {
    root.appendChild(element('div', {
      padding: `${SPACE.tight}px ${SPACE.gap}px`,
      color: COLOUR.textDim,
    }, TEXT.extPageNotMedia));
  }

  root.appendChild(rule());
  const toggle = button(
    state.intercept ? TEXT.extInterceptOn : TEXT.extInterceptOff,
    null,
    async () => { await ask({ kind: 'toggle' }); await draw(); },
  );
  root.appendChild(toggle);
}

draw();
