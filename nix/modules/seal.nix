# Encrypt anything — the file manager's `E`/`U` and `LEADER s e c`.
#
# WHAT RUNS WHERE
# ---------------
# A door asks for whom (the person's post-quantum key, a passphrase or
# someone's `age1pq1…`), writes a passphrase into a `0600` runtime file and
# puts a `Work::Seal` on the transfer queue; the queue runs `ronoctua-seal` as
# a child, which seals or opens the file (a folder as the tar bsdtar streams)
# into a name that is not there, and never deletes the original. The program
# is EXTRACTED from the workspace (`rust.seal`) because it links `age` and the
# X-Wing pieces, and neither the queue nor the footer does
# (`transfers-never-link-age`): this module hands its store path to the
# queue's unit, which is the one place that starts it.
#
# Nothing here is the person's: the passphrase lives in the runtime folder for
# the life of the job, and the belongings' `key.age` it may be opened with is
# read where the belongings keep it, never copied.
{
  config,
  lib,
  ...
}:

let
  cfg = config.ronoctua.seal;
in
{
  options.ronoctua.seal.cli = lib.mkOption {
    type = lib.types.package;
    internal = true;
    description = ''
      `ronoctua-seal`, extracted from the workspace (`rust.seal`): what the
      transfer queue runs for every `Work::Seal`.
    '';
  };

  # THE QUEUE STARTS IT, by store path: a user unit's `PATH` is minimal, and
  # the program of this generation is the one that must run — never whatever
  # a profile points at after a switch.
  config = lib.mkIf config.ronoctua.transfers.enable {
    systemd.user.services.ronoctua-transfers.environment.RONOCTUA_SEAL = "${cfg.cli}/bin/ronoctua-seal";
  };
}
