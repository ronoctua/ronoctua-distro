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
# `refresh` is started by the usage unit, `statusline` and `hook` by Claude
# Code.
#
# THE MANAGED HOOK
# ----------------
# What each Claude Code session is doing — working, waiting for the person,
# done — reaches the modeline's account sets through `ronoctua-llm hook`,
# which keeps one record per session (`ronoctua_llm::sessions`). It is
# subscribed here, in Claude Code's MANAGED settings, and not in any
# account's `settings.json`: Claude Code 2.1.314 reads `/etc/claude-code`
# on Linux whatever `CLAUDE_CONFIG_DIR` says, so one drop-in covers every
# account, and the person's file keeps its one field of ours (`statusLine`).
#
#   - ONLY `hooks`. Managed hooks are merged with the person's own (they
#     survive the person's `disableAllHooks`); `allowManagedHooksOnly` would
#     silence theirs and their plugins', and `disableAllHooks` here would
#     silence everybody's — neither is ever set (`claude-hook-is-managed`).
#   - A LINK INTO THE STORE, as `environment.etc` makes it. Claude Code takes
#     a drop-in that is a file or a link, and its ownership check — asked only
#     when the person's organization asks for it — distrusts a file the user
#     could write: root's `/etc` folders and the read-only store pass it.
#   - `exec`: a hook with no `args` runs through `/bin/sh -c`, and `exec`
#     makes the program the shell, so its parent is Claude Code itself (and
#     Claude Code says its pid too, `CLAUDE_PID`). The system's PATH and not
#     a store path, for the status line's reason: a session outlives a
#     generation.
#   - `async`, `timeout` 5 s: nothing of a session waits on it. An async
#     hook's output joins the conversation and exit code 2 blocks, so the
#     program never prints and always answers 0 (`claude-hook-is-quiet`).
#   - The events are the ones the program maps (`ronoctua_llm::hook::EVENTS`),
#     no matcher (every tool). A subagent's events carry its `agent_id` and
#     are ignored there, so `SubagentStop` is not subscribed.
{
  config,
  lib,
  ...
}:

let
  cfg = config.ronoctua.llm;
  hook = {
    type = "command";
    command = "exec /run/current-system/sw/bin/ronoctua-llm hook";
    async = true;
    timeout = 5;
  };
  events = [
    "SessionStart"
    "SessionEnd"
    "UserPromptSubmit"
    "PreToolUse"
    "PostToolUse"
    "PostToolUseFailure"
    "PermissionRequest"
    "PermissionDenied"
    "Notification"
    "Stop"
    "StopFailure"
  ];
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
    environment.etc."claude-code/managed-settings.d/ronoctua.json".text = builtins.toJSON {
      hooks = lib.genAttrs events (_: [ { hooks = [ hook ]; } ]);
    };
  };
}
