# aria2, as the queue's engine for a plain link.
#
# WHY A UNIT AND NOT A CHILD OF THE QUEUE. The same argument that gave the queue
# its own unit: `nix run .#dev` restarts the footer on every file edit, and the
# queue's daemon is restarted whenever it is rebuilt. aria2 holds the open
# connections and the partial files; killing it to rebuild a Rust binary would
# be a download of four gibibytes thrown away because a comment changed.
#
# THE SECRET IS BORN ON THE MACHINE, and never appears in the repository
# (invariant 13). `nix/modules/secrets.nix` is explicit that a token may not
# live here even encrypted. So the unit generates one into `XDG_RUNTIME_DIR` —
# which is `0700` and the person's own, and which is emptied at logout, so the
# token lives exactly as long as the session does.
#
# AND IT IS NOT ON THE COMMAND LINE. `--rpc-secret=…` on `argv` is readable by
# every process on the machine through `/proc/*/cmdline`. `--conf-path` points
# at a file in a directory nobody else may enter, which is the difference
# between a secret and a secret-shaped string.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.transfers;

  # The configuration, minus the one line that cannot be written down here.
  #
  # `rpc-listen-all=false` binds the loopback and nothing else: an RPC that can
  # write anywhere the person can write is an administrative port, and
  # `nix/modules/syncthing.nix` already records why one of those does not go on
  # the LAN.
  settings = pkgs.writeText "aria2-base.conf" ''
    enable-rpc=true
    rpc-listen-all=false
    rpc-listen-port=6800
    # Resume rather than restart, which is the whole reason this program is
    # here rather than a loop around `curl`.
    continue=true
    max-connection-per-server=8
    split=8
    min-split-size=4M
    max-concurrent-downloads=4
    # NONE, and it is a btrfs decision: `falloc` on a copy-on-write filesystem
    # writes the whole length before a byte arrives, and `prealloc` does it
    # slowly. Neither buys anything where extents are allocated on write.
    file-allocation=none
    auto-file-renaming=true
    allow-overwrite=false
    summary-interval=0
    console-log-level=warn
  '';

  # Writes the session's configuration, secret and all.
  #
  # `install -m600` and not a redirect: a redirect creates the file with the
  # umask's permissions and only then narrows them, and the token is already in
  # it by that point.
  prepare = pkgs.writeShellApplication {
    name = "ronoctua-aria2-prepare";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.openssl
    ];
    text = ''
      conf="$XDG_RUNTIME_DIR/ronoctua-aria2.conf"
      state="''${XDG_STATE_HOME:-$HOME/.local/state}/ronoctua"
      mkdir -p "$state"
      touch "$state/aria2.session"

      # ONE TOKEN PER SESSION. `XDG_RUNTIME_DIR` is emptied at logout, so a new
      # login gets a new one for free — and nothing has to decide when to
      # rotate it.
      secret=$(openssl rand -hex 32)

      umask 077
      tmp=$(mktemp "$XDG_RUNTIME_DIR/ronoctua-aria2.XXXXXX")
      cat ${settings} > "$tmp"
      {
        echo "rpc-secret=$secret"
        echo "save-session=$state/aria2.session"
        echo "input-file=$state/aria2.session"
        echo "save-session-interval=30"
      } >> "$tmp"
      install -m600 "$tmp" "$conf"
      rm -f "$tmp"
    '';
  };
in
{
  config = lib.mkIf cfg.enable {
    systemd.user.services.ronoctua-aria2 = {
      description = "ronoctua — o motor de links da fila";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      serviceConfig = {
        Type = "simple";
        # The secret is written before aria2 reads it, and by a separate
        # program: `ExecStartPre` failing stops the unit, where a shell `&&`
        # inside `ExecStart` would start aria2 with no configuration at all.
        ExecStartPre = "${prepare}/bin/ronoctua-aria2-prepare";
        ExecStart = "${pkgs.aria2}/bin/aria2c --conf-path=%t/ronoctua-aria2.conf";
        Restart = "on-failure";
        RestartSec = 2;
        # The same slice as the queue: what the person is doing, not an app.
        Slice = "session.slice";
        # A download interrupted by the OOM killer resumes; one that takes the
        # desktop with it does not.
        OOMPolicy = "stop";
        TimeoutStopSec = 10;
        # It writes only where it is told to.
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
  };
}
