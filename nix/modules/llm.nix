# The LLM accounts' command line (`ronoctua-llm`), on the system's PATH.
#
# WHY THE SYSTEM'S PATH, AND NOT A STORE PATH
# -------------------------------------------
# Claude Code runs an account's status line from that account's
# `settings.json`, and ours is `/run/current-system/sw/bin/ronoctua-llm
# statusline --profile <id>` (`ronoctua_llm::statusline::PROGRAM`). The file is
# the person's and outlives every generation: a store path written there would
# point at a program the next collection deletes, and the person's own status
# line — chained behind ours — would go blank with it. The system's PATH is the
# one name that always means "this generation's".
#
# WHY ALWAYS, AND NOT ONLY WITH THE AGENTS
# ----------------------------------------
# A wrapped `settings.json` keeps naming the program after `ronoctua.dev.agents`
# is turned off, and a Claude Code installed some other way still runs it. The
# program is small (TLS, serde, a zstd reader) and runs nothing by itself:
# `refresh` is started by the usage unit, `statusline` by Claude Code.
{
  config,
  lib,
  ...
}:

let
  cfg = config.ronoctua.llm;
in
{
  options.ronoctua.llm = {
    cli = lib.mkOption {
      type = lib.types.package;
      internal = true;
      description = ''
        The LLM accounts' CLI, extracted from the workspace (`rust.llm`):
        Claude Code's status line and the usage unit's `refresh`.
      '';
    };
  };

  config = {
    environment.systemPackages = [ cfg.cli ];
  };
}
