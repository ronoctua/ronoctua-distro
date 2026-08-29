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
# WHAT IS NOT PROMISED. Whether a given Chrome build honours a `file://` update
# manifest is not something this file can assert. `tests/extension-vm.nix`
# checks what is checkable — the key exists, the crx is signed, the id in the
# policy is the id of the crx, and the native host's allow-list names it — and
# `docs/PENDING.md` records that the last step is the owner's to confirm in a
# real browser.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.browser;

  # WHERE THE MACHINE-LOCAL HALF LIVES. Root-owned and `0700`: the private key
  # is in it, and the policy that trusts the crx is only as good as the key.
  home = "/var/lib/ronoctua-browser";

  host = "br.ronoctua.transfers";

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

      install -d -m700 "$home"

      # ONCE, AND NEVER AGAIN. Regenerating the key would change the id, and a
      # changed id is a different extension: the old one stays force-installed
      # by a policy nothing writes any more, and the person's switch and
      # history go with it.
      if [ ! -f "$key" ]; then
        umask 077
        openssl genrsa -out "$key" 2048
      fi
      chmod 600 "$key"

      python3 ${./../scripts/pack-crx.py} \
        --source ${ronoctua.extension}/share/ronoctua-extension \
        --key "$key" \
        --out "$crx" \
        --id-file "$idf"
      id=$(cat "$idf")
      chmod 644 "$crx"

      # The update manifest Chrome fetches to decide whether to install. A
      # `file://` URL, because there is no server here and there should not be:
      # this extension is one person's and is never published.
      cat > "$home/update.xml" <<XML
      <?xml version="1.0" encoding="UTF-8"?>
      <gupdate xmlns="http://www.google.com/update2/response" protocol="2.0">
        <app appid="$id">
          <updatecheck codebase="file://$crx" version="${ronoctua.extension.version}" />
        </app>
      </gupdate>
      XML
      chmod 644 "$home/update.xml"

      # ---- THE TWO FILES CHROME READS ----
      #
      # Both need the id, which is only known now — which is precisely why they
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

      # THE ALLOW-LIST IS THE WHOLE SECURITY OF NATIVE MESSAGING. Chrome starts
      # this program for an extension named here and for no other, so an
      # extension somebody else installs cannot reach the queue.
      cat > /etc/opt/chrome/native-messaging-hosts/${host}.json <<JSON
      {
        "name": "${host}",
        "description": "ronoctua's transfer queue",
        "path": "${ronoctua.transfers}/bin/ronoctua-transfers",
        "type": "stdio",
        "allowed_origins": ["chrome-extension://$id/"]
      }
      JSON
      chmod 644 /etc/opt/chrome/native-messaging-hosts/${host}.json
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

    # `path` in the native-messaging manifest is a store path, so the binary it
    # names has to be a runtime dependency of something — otherwise the garbage
    # collector takes it away and Chrome starts nothing. The unit's script cites
    # it, which is what keeps it alive.
    #
    # THE HOST BINARY IS THE QUEUE'S OWN, in its `browser` shape: one binary,
    # one closure, and the same model on both sides of the pipe.
    systemd.services.ronoctua-browser-extension.restartTriggers = [ build ];
  };
}
