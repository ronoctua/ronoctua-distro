# The belongings — the person's configuration kept, encrypted, in a private
# git repository they name, and put back on a new installation (`LEADER s v`).
#
# WHAT RUNS WHERE
# ---------------
# A door asks the passphrase, writes it into a `0600` runtime file and puts a
# `Work::Belongings` on the transfer queue; the queue runs the belongings' CLI
# as a child (`ronoctua-belongings prepare|apply|abandon|keygen|rekey`). The
# CLI is EXTRACTED from the workspace (`rust.belongings`) because it links
# `age`, and neither the queue nor the footer does: this module hands its store
# path to the queue's unit, which is the one place that starts it.
#
# NOTHING HERE IS THE PERSON'S. The repository's address, the deploy key and
# the token live under `~/.config/ronoctua` (written by a door, or by the
# installer's question); what is kept is `assets/belongings.nix`; what a sync
# left is under `~/.local/state/ronoctua/belongings`. This module declares only
# the program and the tools it runs by store path (`nix/rust.nix` compiles git,
# ssh and ssh-keygen in).
#
# The brain reminds of a sync from a module of its own (a folder under
# `crates/ronoctua-brain/modules/`, sealed: only its folder names it), which
# reads the same files.
{
  config,
  lib,
  inputs,
  ...
}:

let
  cfg = config.ronoctua.belongings;
in
{
  options.ronoctua.belongings = {
    enable = lib.mkEnableOption "the belongings (the configuration kept in a private repository)" // {
      default = true;
    };
    cli = lib.mkOption {
      type = lib.types.package;
      internal = true;
      description = ''
        The belongings' CLI, extracted from the workspace (`rust.belongings`):
        what the transfer queue runs for every sync.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # THE QUEUE STARTS IT, by store path: a user unit's `PATH` is minimal, and
    # the CLI of this generation is the one that must run — never whatever a
    # profile points at after a switch.
    systemd.user.services.ronoctua-transfers = lib.mkIf config.ronoctua.transfers.enable {
      environment.RONOCTUA_BELONGINGS = "${cfg.cli}/bin/ronoctua-belongings";
    };

    # THE 2026-10-05 WAVE: one region per package, after its anchor.
    # ── anchor: L08 ──
    # WHEN THIS DISTRO WAS BUILT (the flake's `lastModified`), for the index's
    # `machines.<name>.built_s` — which of the person's machines runs the
    # newest distro (`fleet.json`). A FILE the CLI reads at run time
    # (`engine::BUILT_FILE`), never a variable compiled in: a new commit
    # changes it without rebuilding the workspace, and a switch without
    # restarting the queue that runs the CLI.
    environment.etc."ronoctua/built-s".text = toString (inputs.self.lastModified or 0);
    # ── anchor: L09 ──
    # ── anchor: L10 ──
    # THE IDENTITY KEPT FOR THE SESSION ENDS WITH IT (`crate::unlock`): the
    # CLI keeps it in the kernel's keyring, never on disk, for at most
    # `unlockHours`, and these forget it first.
    #
    # AT LOGOUT: part of the graphical session, forgetting when it stops —
    # and when it starts, so a key a crash left behind never outlives the
    # session it was typed in. `--everywhere`: every instance's, the
    # development loop's too, which runs inside this session (the queue's
    # plain `forget` takes only its own instance's).
    systemd.user.services.ronoctua-belongings-forget = {
      description = "Forgets the belongings' identity kept for this session when the session ends";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        # `-`: a switch starts it, and forgetting is never a failure of it.
        ExecStart = "-${cfg.cli}/bin/ronoctua-belongings forget --everywhere";
        ExecStop = "${cfg.cli}/bin/ronoctua-belongings forget --everywhere";
      };
    };

    # BEFORE THE MACHINE SLEEPS, the `ronoctua-private-lock` mould: a system
    # unit wanted by `sleep.target` and ordered before it, so the forgetting
    # is done before the machine sleeps — a signal a user process heard could
    # lose that race. It runs
    # AS THE PERSON, with their user keyring linked in (`KeyringMode=shared`):
    # root is never asked anything here, and needs nothing. No settings, no
    # home, no runtime variable: `forget` finds the person's runtime folder by
    # their uid. A oneshot without `RemainAfterExit`, so it runs at every
    # sleep.
    systemd.services.ronoctua-belongings-forget-sleep = {
      description = "Forgets the belongings' identity kept for the session before the machine sleeps";
      wantedBy = [ "sleep.target" ];
      before = [ "sleep.target" ];
      serviceConfig = {
        Type = "oneshot";
        User = config.ronoctua.user;
        KeyringMode = "shared";
        ExecStart = "${cfg.cli}/bin/ronoctua-belongings forget --everywhere";
        TimeoutStartSec = "30s";
      };
    };
  };
}
