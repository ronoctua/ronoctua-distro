# THE NATIVE-MESSAGING HOSTS' NAMES — one place.
#
# Chrome finds a host by this name in `/etc/opt/chrome/native-messaging-hosts/
# <name>.json`, and the extension asks for it by the same name; the two used
# to be written in two files, which is one rename away from a host nobody
# answers. `nix/extension.nix` bakes these into the extension, and
# `nix/modules/browser-extension.nix` writes the manifests.
{
  # The queue's door: a download or a page's media handed to the transfers.
  transfers = "br.ronoctua.transfers";
  # The kiosk's bridge: the extension's state and the footer's orders, through
  # `ronoctua-browser` (started by Chrome while a kiosk exists).
  browser = "br.ronoctua.browser";
}
