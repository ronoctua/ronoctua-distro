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
  };
}
