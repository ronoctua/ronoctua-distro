# The extension, installed OUTSIDE the Web Store — and declaratively.
#
# THE PROBLEM. Chrome will not load an extension from a directory unless
# somebody turns on developer mode and clicks, which is imperative and which
# invariant 6 refuses. What it WILL do without anybody clicking is honour an
# enterprise policy in `/etc/opt/chrome/policies/managed/`, which is a plain
# JSON file and perfectly declarable — but a force-installed extension needs a
# signed `.crx` and a stable id, and the id comes from the signing key.
#
# THE KEY THEREFORE CANNOT BE IN THE REPOSITORY. `nix/modules/secrets.nix` is
# explicit: a private key may not live here, not even encrypted. So this module
# declares a unit that generates the key ON THE MACHINE at first boot, packs the
# extension the flake built, derives the id from that key, and writes the two
# files Chrome reads — the policy and the native-messaging host manifest, whose
# `allowed_origins` needs the same id, which is why one unit writes both.
#
# It is the same arrangement as the Wi-Fi secret: the unit is declared, its
# output is machine-local state, and nothing personal is in git (invariant 13).
#
# WHAT IS PROVED WHERE. `tests/extension-vm.nix` checks what needs no browser
# — the key exists, the crx is signed, the id in the policy is the id of the
# crx, and both hosts' allow-lists name it. `tests/kiosk-vm.nix` runs the real
# Chrome, offline, and proves the rest: that it installs the extension from
# the `file://` update manifest, and starts both hosts.
#
# THE VERSION GOES UP HERE. Chrome replaces a force-installed extension only
# with a higher version, so the unit keeps a counter beside the key and adds
# it as the fourth number whenever the extension it packs is another build —
# see `nix/extension-version.nix`.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.browser;

  # WHERE THE MACHINE-LOCAL HALF LIVES. Root-owned; the private key in it is
  # `0600`, and the policy that trusts the crx is only as good as the key.
  #
  # THE DIRECTORY IS `0755`, AND IT HAS TO BE. Chrome runs as the person and
  # reads `update.xml` and the crx from here through the policy's `file://`
  # addresses; a `0700` directory (what this was) is one it cannot enter, so
  # the force-install waited for ever with nothing logged anywhere a person
  # looks. Measured in `tests/kiosk-vm.nix`, the real Chrome, offline. The key
  # is protected by its own mode, which a listing does not reveal.
  home = "/var/lib/ronoctua-browser";

  # THE HOSTS' NAMES, from the one table the extension is built from too.
  hosts = import ../browser-hosts.nix;

  # One native-messaging host manifest. THE ALLOW-LIST IS THE WHOLE SECURITY
  # OF NATIVE MESSAGING: Chrome starts `path` for an extension named here and
  # for no other, so an extension somebody else installs reaches neither the
  # queue nor the footer. `path` has no arguments — Chrome passes the caller's
  # origin as the first one, and both binaries accept exactly that.
  hostManifest = name: description: path: ''
    cat > /etc/opt/chrome/native-messaging-hosts/${name}.json <<JSON
    {
      "name": "${name}",
      "description": "${description}",
      "path": "${path}",
      "type": "stdio",
      "allowed_origins": ["chrome-extension://$id/"]
    }
    JSON
    chmod 644 /etc/opt/chrome/native-messaging-hosts/${name}.json
  '';

  build = pkgs.writeShellApplication {
    name = "ronoctua-browser-extension-setup";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.openssl
      pkgs.python3
    ];
    text = ''
      home=${lib.escapeShellArg home}
      key="$home/key.pem"
      crx="$home/ronoctua.crx"
      idf="$home/id"
      built="$home/built"
      source=${ronoctua.extension}/share/ronoctua-extension

      install -d -m755 "$home"
      chmod 755 "$home"

      # ONCE, AND NEVER AGAIN. Regenerating the key would change the id, and a
      # changed id is a different extension: the old one stays force-installed
      # by a policy nothing writes any more, and the person's switch and
      # history go with it.
      if [ ! -f "$key" ]; then
        umask 077
        openssl genrsa -out "$key" 2048
      fi
      chmod 600 "$key"

      # THE FOURTH NUMBER: the same build keeps its number, another build
      # takes the next one. `built` holds the store path last packed and the
      # counter, and lives and dies with the key.
      last=""
      count=0
      if [ -f "$built" ]; then
        read -r last count < "$built" || true
      fi
      case "$count" in
        ""|*[!0-9]*) count=0 ;;
      esac
      if [ "$last" != "$source" ]; then
        count=$((count + 1))
      fi
      version="${ronoctua.extension.version}.$count"

      # The manifest with that version, in a copy: the store's is read-only,
      # and the signature has to cover the number Chrome compares.
      work=$(mktemp -d)
      trap 'rm -rf "$work"' EXIT
      cp -r "$source" "$work/extension"
      chmod -R u+w "$work/extension"
      python3 - "$work/extension/manifest.json" "$version" <<'PY'
      import json, pathlib, sys
      path = pathlib.Path(sys.argv[1])
      manifest = json.loads(path.read_text())
      manifest["version"] = sys.argv[2]
      path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
      PY

      python3 ${./../scripts/pack-crx.py} \
        --source "$work/extension" \
        --key "$key" \
        --out "$crx" \
        --id-file "$idf"
      id=$(cat "$idf")
      chmod 644 "$crx"
      printf '%s %s\n' "$source" "$count" > "$built.tmp"
      mv "$built.tmp" "$built"

      # The update manifest Chrome fetches to decide whether to install. A
      # `file://` URL, because there is no server here and there should not be:
      # this extension is one person's and is never published.
      cat > "$home/update.xml" <<XML
      <?xml version="1.0" encoding="UTF-8"?>
      <gupdate xmlns="http://www.google.com/update2/response" protocol="2.0">
        <app appid="$id">
          <updatecheck codebase="file://$crx" version="$version" />
        </app>
      </gupdate>
      XML
      chmod 644 "$home/update.xml"

      # ---- THE FILES CHROME READS: the policy and the two hosts ----
      #
      # All need the id, which is only known now — which is precisely why they
      # are written here rather than by `environment.etc`.
      install -d -m755 /etc/opt/chrome/policies/managed
      install -d -m755 /etc/opt/chrome/native-messaging-hosts

      cat > /etc/opt/chrome/policies/managed/ronoctua.json <<JSON
      {
        "ExtensionInstallSources": ["file:///*"],
        "ExtensionSettings": {
          "$id": {
            "installation_mode": "force_installed",
            "update_url": "file://$home/update.xml",
            "toolbar_pin": "force_pinned"
          }
        }
      }
      JSON
      chmod 644 /etc/opt/chrome/policies/managed/ronoctua.json

      # The queue's door: a download handed over by the extension.
      ${hostManifest hosts.transfers "ronoctua's transfer queue"
        "${ronoctua.transfers}/bin/ronoctua-transfers"
      }
      # The kiosk's bridge: started while a kiosk window exists, and the one
      # that connects to the footer.
      ${hostManifest hosts.browser "ronoctua's kiosk bridge" "${ronoctua.browser}/bin/ronoctua-browser"}
    '';
  };
in
{
  config = lib.mkIf cfg.enable {
    systemd.services.ronoctua-browser-extension = {
      description = "ronoctua — packages and declares the browser extension";
      wantedBy = [ "multi-user.target" ];
      # BEFORE ANYBODY LOGS IN: Chrome reads the policy at start-up, so a policy
      # written after the browser is already running takes effect on the next
      # one. Ordering it here makes the first login the first one that works.
      before = [ "display-manager.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${build}/bin/ronoctua-browser-extension-setup";
      };
    };

    # `path` in the native-messaging manifests is a store path, so the binary
    # each names has to be a runtime dependency of something — otherwise the
    # garbage collector takes it away and Chrome starts nothing. The unit's
    # script cites both, which is what keeps them alive.
    #
    # THE QUEUE'S HOST IS THE QUEUE'S OWN BINARY, in its browser shape: one
    # binary, one closure, and the same model on both sides of the pipe. The
    # kiosk's is `ronoctua-browser`, extracted from the workspace so its
    # closure is a JSON parser and a socket.
    #
    # AND A NEW BUILD IS PACKED AGAIN: the script cites the extension and both
    # hosts, so any of the three changing restarts the unit on a switch — the
    # crx is re-signed with the next version and the manifests point at the
    # new binaries.
    systemd.services.ronoctua-browser-extension.restartTriggers = [ build ];
  };
}
