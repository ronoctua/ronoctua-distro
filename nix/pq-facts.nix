# WHAT THIS GENERATION SHIPS, for the security centre's post-quantum section
# (`LEADER s e q`, `ronoctua_security::quantum::Built`).
#
# Whether a channel holds against a quantum computer is, for most of them, a
# fact of the VERSION that runs: OpenSSL offers X25519MLKEM768 first from 3.5,
# Chrome agrees it by default from 131, Go's `crypto/tls` from 1.24. Those
# versions are known here, at build, and nowhere at run time without running
# the programs — so they are written down once, as JSON, and the shell bakes
# this file's path (`RONOCTUA_PQ_FACTS`, in `generated`).
#
# AND ONE FACT OF A SOURCE: Go's default only reaches Syncthing if Syncthing
# leaves `tls.Config.CurvePreferences` alone (setting it narrows the exchange
# to the curves it names) and its `go.mod` says a Go of 1.24 or later (Go's
# defaults follow the version a module is written for). Both are read from
# the pinned source, so the day a release pins its curves the section says
# so without anybody remembering to look.
#
# AND, FOR ONE SYSTEM, WHICH OPTIONAL CHANNELS IT TURNED ON (`ships`): the
# workspace — one build every configuration shares — cannot know whether the
# machine it lands on has Chrome, Syncthing, the tailnet, Tor or Android, so
# its copy has no `ships` and the section draws every channel. The system's
# own copy is written by `nix/modules/security.nix`, from the options that
# turned each one on, and the footer's unit names it — so a minimal machine is
# not told what a browser it does not have agrees.
#
# The JSON names no store path: a binary that bakes this file's path carries
# a few hundred bytes, never Syncthing's source or Chrome.
{
  pkgs,
  # `{ chrome; syncthing; tailscale; tor; android; }`, each a bool — or null
  # for the workspace's copy.
  ships ? null,
}:

pkgs.runCommand "ronoctua-pq-facts.json"
  {
    src = pkgs.syncthing.src;
    openssh = pkgs.openssh.version;
    openssl = pkgs.openssl.version;
    # Unfree, and only its version is read: nothing of Chrome is built here.
    chrome = pkgs.google-chrome.version;
    syncthingVersion = pkgs.syncthing.version;
    syncthingGo = pkgs.syncthing.go.version;
    tor = pkgs.tor.version;
    tailscale = pkgs.tailscale.version;
    # A JSON member appended after `tailscale`, comma first — or nothing.
    ships = if ships == null then "" else ",\"ships\":${builtins.toJSON ships}";
  }
  ''
    # THE LAYOUT FIRST: a source with no `lib/` would grep nothing and say
    # "does not narrow" about a tree it never read.
    sources=$( (find "$src/lib" "$src/cmd" -name '*.go' 2> /dev/null || true) | wc -l)
    if [ "$sources" -lt 100 ]; then
      echo "pq-facts: only $sources Go files under Syncthing's lib/ and cmd/ — has its layout moved?" >&2
      exit 1
    fi
    # A FILE OF THE PRODUCT that sets the curves, its tests left out.
    pins=false
    if grep -rl --include='*.go' 'CurvePreferences' "$src/lib" "$src/cmd" 2>/dev/null \
      | grep -v '_test\.go$' | grep -q .; then
      pins=true
    fi
    goMod=$(sed -n 's/^go \([0-9][0-9.]*\).*/\1/p' "$src/go.mod" | head -n 1)
    if [ -z "$goMod" ]; then
      echo "pq-facts: no go line in Syncthing's go.mod" >&2
      exit 1
    fi
    printf '{"openssh":"%s","openssl":"%s","chrome":"%s","syncthing":{"version":"%s","go":"%s","go-mod":"%s","pins-curves":%s},"tor":"%s","tailscale":"%s"%s}\n' \
      "$openssh" "$openssl" "$chrome" "$syncthingVersion" "$syncthingGo" "$goMod" "$pins" "$tor" "$tailscale" "$ships" > "$out"
  ''
