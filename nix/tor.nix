# WHERE TOR LISTENS AND WHERE ITS FRAGMENT LIVES — in ONE place.
#
# WHY A FILE OF ITS OWN
# ---------------------
# Because two different kinds of thing read it, and only one of them is a
# NixOS module:
#
#   - `nix/modules/tor.nix` turns `socks` into `services.tor.client.
#     socksListenAddress`, which is what actually makes Tor listen, and
#     `fragmentDirectory` into the `%include` and the tmpfiles rule;
#   - `nix/rust.nix` bakes both into the shell as `RONOCTUA_TOR_SOCKS` and
#     `RONOCTUA_TOR_FRAGMENT`, because the view NAMES the address on screen and
#     WRITES into that directory.
#
# THIS FILE EXISTS BECAUSE THE TWO SIDES USED TO AGREE BY LUCK.
# `ronoctua_tunnel::tor::SOCKS` was the literal `"127.0.0.1:9050"`, and the
# module declared no `SOCKSPort` at all — so the number on screen matched the
# number Tor listened on only because the nixpkgs default happened to be the
# same. Nothing would have said otherwise the day either moved: the row would
# have named a port nothing listened on, and a browser pointed at it would
# have failed with a connection refused that blames the network.
#
# It is the same shape as `nix/user-dirs.nix` and `nix/keyboard-xkb.nix`: one
# file, several readers, and a check that compares them.
{
  # THE LOOPBACK, and changing the address puts an open proxy on the network.
  socks = {
    address = "127.0.0.1";
    port = 9050;
  };

  # The directory the generated torrc `%include`s and the view writes into.
  #
  # Under `/var/lib/ronoctua` and not in the person's home: it is read by TOR,
  # which runs as its own user and cannot enter a home directory. The group is
  # what lets the owner write there — the arrangement `/etc/keyd` already has.
  fragmentDirectory = "/var/lib/ronoctua/tor";
}
