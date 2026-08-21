// Reads the media elements of ONE tab, once, because a button was pressed.
//
// It runs in the isolated world, so nothing it does is visible to the page's
// own JavaScript, and it MUTATES NOTHING: no element, no attribute, no style,
// no event listener. A content script that left a marker behind would be the
// cheapest way in the world for a site to recognise this browser, and the whole
// design of this extension is built around not giving one.
//
// It is also not in the manifest. It is injected by `chrome.scripting` under
// `activeTab`, which means it exists on no page until the person asks.
(() => {
  const out = [];
  const seen = new Set();
  const keep = (url, kind) => {
    if (!url || !/^https?:/i.test(url) || seen.has(url)) return;
    seen.add(url);
    out.push({ url, kind });
  };
  for (const node of document.querySelectorAll('video, audio')) {
    keep(node.currentSrc || node.src, node.tagName.toLowerCase());
    for (const source of node.querySelectorAll('source')) keep(source.src, node.tagName.toLowerCase());
  }
  // A `blob:` source means the page is feeding the element from JavaScript, and
  // there is no address anybody else can fetch. The page's own URL is what
  // yt-dlp wants in that case, and the popup already offers it.
  return out.slice(0, 20);
})();
