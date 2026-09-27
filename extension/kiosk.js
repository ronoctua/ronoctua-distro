// The page a kiosk window is born on — blank, and only for as long as the
// footer takes to find it.
//
// ITS TITLE IS THE NONCE. The footer put a nonce in this page's fragment and
// watches the compositor's windows for a title that carries it; the worker
// tells the footer the same nonce with Chrome's window id. The two halves
// make the pair — never timing, and never a guess about which new window is
// which.
//
// IT NEVER STAYS. The worker answers `{go}` when the footer has paired the
// window (or when there is no footer to do it); a page whose worker cannot be
// reached leaves on its own. The address is checked here too, with the same
// rules the worker and the host use.
//
// NO TEXT AND NO COLOUR OF ITS OWN (`extension-has-no-literal`): the ground is
// the palette's, and there is nothing to read.

import { COLOUR } from './generated.js';
import { allowed } from './address.js';

document.documentElement.style.background = COLOUR.surfaceBase;
document.body.style.background = COLOUR.surfaceBase;

const fields = new URLSearchParams(location.hash.slice(1));
const nonce = fields.get('n') || '';
const address = fields.get('u') || '';
document.title = nonce;

function leave(to) {
  if (to && allowed(to)) location.replace(to);
}

// A page that was NAVIGATED to is a birth. One that came back — a reload, the
// history, a restored session — was paired or abandoned long ago, and waiting
// for a footer that is not looking for it would be a blank window for ever.
const how = (performance.getEntriesByType('navigation')[0] || {}).type || 'navigate';
if (how !== 'navigate') {
  leave(address);
} else {
  chrome.runtime
    .sendMessage({ kind: 'kiosk-born', nonce, profile: fields.get('p') || '', url: address })
    .then((answer) => leave(answer && answer.go))
    .catch(() => leave(address));
}
