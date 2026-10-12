# The LLM accounts' module's NixOS half: the usage unit the brain starts.
#
# WHAT IT RUNS
# ------------
# `ronoctua-llm refresh` (`crates/ronoctua-llm`, `refresh::run`): every
# account's transcripts counted from where the last count stopped, and — for a
# Claude Code account whose host the person let in `consent.json` — the usage
# endpoint asked, the answer or its refusal kept beside the last numbers. It
# asks nobody anything: the pass is minted from the person's consent at the
# moment of asking, and a host never let sends nothing.
#
# WHO STARTS IT, AND WHEN
# -----------------------
# The brain alone (`module.rs`): after an account was used, and no sooner than
# a few minutes after its last start — never on a clock, so there is no timer
# and no `wantedBy` here. A person who switches the module off
# (`ronoctua.brain.modules.agent-accounts.enable = false`) takes the unit away
# with it (the gate `nix/brain-modules.nix` puts around this file); the CLI
# stays on the PATH (`nix/modules/llm.nix`), because Claude Code runs its
# status line from there.
#
# BY STORE PATH, like every program a unit runs: the extracted CLI the
# accounts' module puts on the PATH (`ronoctua.llm.cli`), whose closure is
# TLS, serde and bash — never the desktop.
#
# THE NAME IS THE MANIFEST'S (`units.run`), written once there and read here.
{
  config,
  lib,
  ...
}:

let
  manifest = import ./manifest.nix;
  service = unit: lib.removeSuffix ".service" unit;
in
{
  systemd.user.services.${service manifest.units.run} = {
    description = "ronoctua — count the LLM accounts' tokens and ask their usage";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${config.ronoctua.llm.cli}/bin/ronoctua-llm refresh";
      # A COUNT, NOT A TASK OF THE PERSON'S: the transcripts of a month can be
      # a few hundred megabytes the first time, and the machine is in use.
      Nice = 10;
      IOSchedulingClass = "idle";
      NoNewPrivileges = true;
      # NO FILESYSTEM SANDBOX: a user unit's `ProtectSystem=` needs user
      # namespaces, and a `ReadWritePaths=` over a folder not made yet stops
      # it before it runs (226/NAMESPACE) — failures only a VM shows. What it
      # writes is the crate's to say: the person's state and cache, never an
      # agent's folder.
    };
  };
}
