# Every Atuin setting this system offers, declared once.
#
# The same generator as Tor's and Syncthing's — see
# `assets/syncthing-options.nix` for what each field means.
#
# `key` IS THE TOML KEY, spelled as Atuin spells it, and `within` is the
# SECTION it lives under: `[sync]`, `[keys]`, `[preview]`, `[daemon]`. Nothing
# in Atuin's file nests deeper than that, which is what lets
# `ronoctua_atuin::config` edit one LINE instead of reserialising the file and
# losing the person's own comments.
#
# WHAT IS DELIBERATELY NOT HERE
# ------------------------------
# Atuin has about seventy settings and this offers forty. What is left out is
# left out for a reason, not for room:
#
#   * `db_path`, `key_path`, `socket_path`, `data_dir` — where things are is
#     the distro's, and a row that moved the database would leave the old one
#     behind with no way back to it.
#   * `[theme]` — colour is invariant 9's, and Atuin's TUI draws in this
#     system's terminal theme already.
#   * `dialect`, `timezone` — `LEADER s l` owns those, for the whole machine.
#     Two screens answering "what time is it" is two answers.
#   * `sync_address` — it is set by LOGGING IN, which is the only moment the
#     address is known to be one that works. A row that typed it independently
#     would let the two disagree.
{
  # ---------------------------------------------------------------------
  # WHAT LEAVES THIS MACHINE — the section this whole feature turns on.
  # ---------------------------------------------------------------------
  leaving = [
    {
      id = "auto-sync";
      key = "auto_sync";
      kind = "switch";
    }
    {
      id = "sync-frequency";
      key = "sync_frequency";
      kind = "text";
    }
    # THE ONE THAT ANSWERS THE OBJECTION. `nix/modules/home.nix` refused sync
    # in writing because "a command with a token in argv" would leave the
    # machine. This is what stops that command being RECORDED at all, so it
    # cannot leave — and it is drawn rather than defaulted because a safeguard
    # nobody reads is not one.
    {
      id = "secrets-filter";
      key = "secrets_filter";
      kind = "switch";
    }
    {
      id = "history-filter";
      key = "history_filter";
      kind = "text";
    }
    {
      id = "cwd-filter";
      key = "cwd_filter";
      kind = "text";
    }
    {
      id = "store-failed";
      key = "store_failed";
      kind = "switch";
    }
    {
      id = "sync-records";
      key = "records";
      within = "sync";
      kind = "switch";
    }
  ];

  # ---------------------------------------------------------------------
  # THE SEARCH — what `Ctrl+R` does.
  # ---------------------------------------------------------------------
  search = [
    {
      id = "search-mode";
      key = "search_mode";
      kind = "ring";
      ring = [
        "fuzzy"
        "prefix"
        "fulltext"
        "skim"
      ];
    }
    {
      id = "filter-mode";
      key = "filter_mode";
      kind = "ring";
      ring = [
        "global"
        "host"
        "session"
        "directory"
        "workspace"
      ];
    }
    {
      id = "up-key-filter";
      key = "filter_mode_shell_up_key_binding";
      kind = "ring";
      ring = [
        "global"
        "host"
        "session"
        "directory"
      ];
    }
    {
      id = "search-style";
      key = "style";
      kind = "ring";
      ring = [
        "auto"
        "full"
        "compact"
      ];
    }
    {
      id = "inline-height";
      key = "inline_height";
      kind = "number";
      least = 0;
      most = 60;
      unit = "lines";
    }
    {
      id = "enter-accept";
      key = "enter_accept";
      kind = "switch";
    }
    {
      id = "show-preview";
      key = "show_preview";
      kind = "switch";
    }
    {
      id = "show-help";
      key = "show_help";
      kind = "switch";
    }
    {
      id = "show-tabs";
      key = "show_tabs";
      kind = "switch";
    }
    {
      id = "invert";
      key = "invert";
      kind = "switch";
    }
    {
      id = "exit-mode";
      key = "exit_mode";
      kind = "ring";
      ring = [
        "return-original"
        "return-query"
      ];
    }
  ];

  # ---------------------------------------------------------------------
  # THE KEYS — how the search behaves under the fingers.
  # ---------------------------------------------------------------------
  keys = [
    {
      id = "scroll-exits";
      key = "scroll_exits";
      within = "keys";
      kind = "switch";
    }
    {
      id = "accept-past-line-end";
      key = "accept_past_line_end";
      within = "keys";
      kind = "switch";
      advanced = true;
    }
    {
      id = "ctrl-n-shortcuts";
      key = "ctrl_n_shortcuts";
      within = "keys";
      kind = "switch";
      advanced = true;
    }
    {
      id = "prefix-key";
      key = "prefix";
      within = "keys";
      kind = "text";
      advanced = true;
    }
  ];

  # ---------------------------------------------------------------------
  # THE DRAWER — real settings, rarely touched.
  # ---------------------------------------------------------------------
  advanced = [
    {
      id = "workspaces";
      key = "workspaces";
      kind = "switch";
      advanced = true;
    }
    {
      id = "common-prefix";
      key = "common_prefix";
      kind = "text";
      advanced = true;
    }
    {
      id = "common-subcommands";
      key = "common_subcommands";
      kind = "text";
      advanced = true;
    }
    {
      id = "ignored-commands";
      key = "ignored_commands";
      kind = "text";
      advanced = true;
    }
    {
      id = "command-chaining";
      key = "command_chaining";
      kind = "switch";
      advanced = true;
    }
    {
      id = "strip-trailing-whitespace";
      key = "strip_trailing_whitespace";
      kind = "switch";
      advanced = true;
    }
    {
      id = "network-timeout";
      key = "network_timeout";
      kind = "number";
      least = 1;
      most = 600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "network-connect-timeout";
      key = "network_connect_timeout";
      kind = "number";
      least = 1;
      most = 120;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "local-timeout";
      key = "local_timeout";
      kind = "number";
      least = 1;
      most = 120;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "max-preview-height";
      key = "max_preview_height";
      kind = "number";
      least = 1;
      most = 40;
      unit = "lines";
      advanced = true;
    }
    {
      id = "scroll-context-lines";
      key = "scroll_context_lines";
      kind = "number";
      least = 0;
      most = 20;
      unit = "lines";
      advanced = true;
    }
    {
      id = "word-jump-mode";
      key = "word_jump_mode";
      kind = "ring";
      ring = [
        "emacs"
        "subl"
      ];
      advanced = true;
    }
    {
      id = "word-chars";
      key = "word_chars";
      kind = "text";
      advanced = true;
    }
    {
      id = "keymap-mode";
      key = "keymap_mode";
      kind = "ring";
      ring = [
        "emacs"
        "vim-normal"
        "vim-insert"
        "auto"
      ];
      advanced = true;
    }
    {
      id = "prefers-reduced-motion";
      key = "prefers_reduced_motion";
      kind = "switch";
      advanced = true;
    }
    {
      id = "daemon-enabled";
      key = "enabled";
      within = "daemon";
      kind = "switch";
      advanced = true;
    }
    {
      id = "daemon-sync-frequency";
      key = "sync_frequency";
      within = "daemon";
      kind = "number";
      least = 60;
      most = 86400;
      unit = "seconds";
      advanced = true;
    }
    # `update_check` IS THE FLAKE'S. The version is pinned, and a binary that
    # phones home to offer one it cannot install is a request nobody asked for.
    {
      id = "update-check";
      key = "update_check";
      kind = "switch";
      fixed = true;
    }
  ];
}
