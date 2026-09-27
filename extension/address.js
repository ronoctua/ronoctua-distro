// Which addresses a kiosk may be sent to — the SAME rules as the host's
// `crates/ronoctua-browser/src/url.rs`, checked again on this side.
//
// WHY TWICE. The footer is not the only program that could reach the host's
// socket in principle, and a `javascript:` address given to `tabs.update`
// runs script in the page. So the extension refuses on its own authority
// before it acts, and the host refuses before it relays.
//
// A module of its own, with no side effect, because two very different
// places import it: the service worker, and the kiosk page — which must not
// pull the worker's listeners into a tab.

// Chrome's own new-tab page, the one `chrome://` address a kiosk may open.
export const NEW_TAB = 'chrome://newtab/';

// An extension id: 32 letters from `a` to `p`.
export function isExtensionId(id) {
  return typeof id === 'string' && /^[a-p]{32}$/.test(id);
}

// http or https with a host and no credentials, or the new-tab page.
export function allowed(address) {
  if (typeof address !== 'string') return false;
  if (address === NEW_TAB) return true;
  const at = address.indexOf('://');
  if (at < 0) return false;
  const scheme = address.slice(0, at).toLowerCase();
  if (scheme !== 'http' && scheme !== 'https') return false;
  const authority = address.slice(at + 3).split(/[/?#]/)[0];
  return authority.length > 0 && !authority.includes('@') && !/\s/.test(authority);
}

// A page inside an extension, or its entry in Chrome's extension manager.
export function extensionPage(address) {
  if (typeof address !== 'string') return false;
  const manager = 'chrome://extensions/?id=';
  if (address.startsWith(manager)) return isExtensionId(address.slice(manager.length));
  const inside = 'chrome-extension://';
  if (!address.startsWith(inside)) return false;
  const rest = address.slice(inside.length);
  const slash = rest.indexOf('/');
  const id = slash < 0 ? rest : rest.slice(0, slash);
  const path = slash < 0 ? '' : rest.slice(slash + 1);
  return isExtensionId(id) && !path.includes('..');
}
