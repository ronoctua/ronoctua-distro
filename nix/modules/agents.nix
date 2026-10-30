# The LLM accounts' view (`LEADER m a`, `crate::agents` in the shell): the
# agents' programs, handed to the footer by store path.
#
# WHY THE FOOTER NEEDS THEM
# -------------------------
# The view opens a session of an account, and its login, in our terminal:
# `env CLAUDE_CONFIG_DIR=… <claude>` or `env CODEX_HOME=… <codex>`, a command
# line `ronoctua-llm` builds (`launch_arguments`, `login_arguments`) from the
# program it is handed in `RONOCTUA_CLAUDE` / `RONOCTUA_CODEX`. The footer's
# unit has the session's PATH and not the person's shell, so the program
# comes by store path — and the window of the view is the footer's child,
# and inherits it.
#
# WHY HERE, AND NEVER COMPILED IN
# -------------------------------
# Claude Code is unfree: a store path baked into a binary would carry it into
# every closure that links the crate (`one-llm-boundary` refuses an
# `option_env!` for either). So each follows its PROGRAM in the registry
# (`ronoctua.distroApps.installed`), as the lifeline's "ask an AI" does: a
# person who declined Claude Code is not offered a session of it, and the
# view says why (`agents-refused-not-installed`). A system without the
# registry (a VM test's) hands over neither.
#
# The default account is NOT set here: it is the person's, written by the
# crate into `llm-default.env`, which the shell sources.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  installed = id: config.ronoctua.distroApps.installed.${id} or false;
  # Claude Code is a part (`nix/modules/dev.nix`): its stable path.
  parts = config.ronoctua ? parts;
in
{
  # PART-ONLY (owner amendment 2026-10-30, reverting the native-install
  # attempt): the footer launches the store PART at its stable path. A native
  # `~/.local/bin/claude` auto-updater downloads a generic-linux binary that
  # NixOS cannot run without nix-ld, so there is no bootstrap and no $HOME
  # preference — the PART (patched, pinned, `DISABLE_AUTOUPDATER`) is the one
  # that runs. The pin moves the version (`nix/claude-code.nix`).
  config.systemd.user.services.ronoctua-footer.environment = lib.mkMerge [
    (lib.mkIf (installed "claude-code" && parts) {
      RONOCTUA_CLAUDE = "${config.ronoctua.parts.claude-code.path}/bin/claude";
    })
    (lib.mkIf (installed "codex") {
      RONOCTUA_CODEX = "${pkgs.codex}/bin/codex";
    })
  ];
}
