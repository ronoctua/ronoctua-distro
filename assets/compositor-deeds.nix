# WHAT THE COMPOSITOR CAN BE TOLD TO DO — declared here, by us.
#
# # Why this file exists at all
#
# The compositor's commands are free text. There is no list to extract from a
# binary, and the four tables in its source are C arrays of every directive it
# accepts — including forty that only make sense inside a configuration file.
#
# So the vocabulary is OURS. That is not a concession: it is the shape the rest
# of this repository already has (`assets/notifications.nix`,
# `assets/syntax.nix`, `assets/updates.nix`), and it means the list is chosen
# rather than mirrored. What was mirrored before was a hundred and forty-one
# rows of somebody else's `--help`, most of which nobody could find and nobody
# had written a sentence about.
#
# # The rule for being in here
#
# A deed is in this table if BOTH hold:
#
#   1. it is meaningful with no criteria and no runtime knowledge — `focus
#      left` is, `[con_id=7] focus` is not;
#   2. somebody can write one sentence about it, in every declared language.
#
# The second is not a formality. `cmd-wm-<id>` must exist in every `.ftl` or
# the build fails, and a row nobody can describe is a row nobody can find in
# the minibuffer either.
#
# # The shape
#
#   <id> = { text = "<what to say>"; argument = null | "number" | "word"; };
#
# `text` is the command. `{n}` in it is where the argument goes — a
# PLACEHOLDER and not an append, because moving a window to a workspace and
# following it names the number twice.
#
# `argument` is what may be typed after it, and it is enforced at the wire:
# commands are separated by `;` and `,`, so an argument is text landing in the
# middle of a sentence the compositor will parse. `wm-workspace-go` with
# `3; exit` would end the session. See `spoken()` in `ronoctua-scroll`.
{
  # --- moving the focus ------------------------------------------------
  # The four directions are the ones a hand reaches for, and they are what the
  # chords are bound to. `beginning`/`end` are the ends of the scroll.
  focus-left = {
    text = "focus left";
  };
  focus-right = {
    text = "focus right";
  };
  focus-up = {
    text = "focus up";
  };
  focus-down = {
    text = "focus down";
  };
  focus-beginning = {
    text = "focus beginning";
  };
  focus-end = {
    text = "focus end";
  };
  focus-parent = {
    text = "focus parent";
  };
  focus-child = {
    text = "focus child";
  };
  focus-tiling = {
    text = "focus tiling";
  };
  focus-floating = {
    text = "focus floating";
  };
  focus-either = {
    text = "focus mode_toggle";
  };
  focus-screen-left = {
    text = "focus output left";
  };
  focus-screen-right = {
    text = "focus output right";
  };
  focus-screen-up = {
    text = "focus output up";
  };
  focus-screen-down = {
    text = "focus output down";
  };

  # --- moving the window -----------------------------------------------
  # `nomode` moves the WINDOW and leaves its column where it is; without it
  # the unit that moves is whatever the current mode says, which for a window
  # sharing a column with others means it cannot be taken out of it.
  move-left = {
    text = "move left";
  };
  move-right = {
    text = "move right";
  };
  move-up = {
    text = "move up";
  };
  move-down = {
    text = "move down";
  };
  move-beginning = {
    text = "move beginning";
  };
  move-end = {
    text = "move end";
  };
  move-out-left = {
    text = "move left nomode";
  };
  move-out-right = {
    text = "move right nomode";
  };
  move-out-up = {
    text = "move up nomode";
  };
  move-out-down = {
    text = "move down nomode";
  };
  move-screen-left = {
    text = "move container to output left";
  };
  move-screen-right = {
    text = "move container to output right";
  };

  # --- the window itself -----------------------------------------------
  close = {
    text = "kill";
  };
  close-others = {
    text = "kill unfocused";
  };
  full-screen = {
    text = "fullscreen";
  };
  full-layout = {
    text = "fullscreen layout";
  };
  full-application = {
    text = "fullscreen application";
  };
  full-global = {
    text = "fullscreen global";
  };
  float = {
    text = "floating toggle";
  };
  stick = {
    text = "sticky toggle";
  };
  calm = {
    text = "urgent disable";
  };

  # --- THE SCROLLING LAYOUT, which is why this compositor and not another --
  # `set_mode h` puts the next window in a column of its own; `set_mode v`
  # puts it under the current one. It is the whole difference, and it is per
  # workspace.
  columns = {
    text = "set_mode h";
  };
  rows = {
    text = "set_mode v";
  };
  transpose = {
    text = "layout_transpose";
  };
  width-next = {
    text = "cycle_size h next";
  };
  width-previous = {
    text = "cycle_size h prev";
  };
  height-next = {
    text = "cycle_size v next";
  };
  height-previous = {
    text = "cycle_size v prev";
  };
  width-set = {
    text = "set_size h {n}";
    argument = "number";
  };
  height-set = {
    text = "set_size v {n}";
    argument = "number";
  };
  # `all equal` shares the column out between every window in it. Written
  # without both words the command parses, answers success, and does nothing.
  fit-columns = {
    text = "fit_size h all equal";
  };
  fit-rows = {
    text = "fit_size v all equal";
  };
  fit-visible = {
    text = "fit_size h visible proportional";
  };
  maximise = {
    text = "fit_size h active proportional";
  };

  # --- where a column sits ---------------------------------------------
  align-left = {
    text = "align left";
  };
  align-right = {
    text = "align right";
  };
  align-centre = {
    text = "align center";
  };
  align-top = {
    text = "align up";
  };
  align-bottom = {
    text = "align down";
  };
  align-middle = {
    text = "align middle";
  };
  align-auto = {
    text = "align reset";
  };
  pin-beginning = {
    text = "pin beginning";
  };
  pin-end = {
    text = "pin end";
  };

  # --- marking several, and moving them together -----------------------
  select = {
    text = "selection toggle";
  };
  select-none = {
    text = "selection reset";
  };
  select-workspace = {
    text = "selection workspace";
  };
  select-move = {
    text = "selection move";
  };
  select-to-trail = {
    text = "selection to_trail";
  };

  # --- trails: a group of windows that ignores which workspace they are on --
  trailmark = {
    text = "trailmark toggle";
  };
  trailmark-next = {
    text = "trailmark next";
  };
  trailmark-previous = {
    text = "trailmark prev";
  };
  trailmark-jump = {
    text = "trailmark jump";
  };
  trail-new = {
    text = "trail new";
  };
  trail-next = {
    text = "trail next";
  };
  trail-previous = {
    text = "trail prev";
  };
  trail-clear = {
    text = "trail clear";
  };
  trail-delete = {
    text = "trail delete";
  };

  # --- reaching a window by typing a letter at it ----------------------
  jump = {
    text = "jump";
  };
  jump-tiling = {
    text = "jump tiling";
  };
  jump-everywhere = {
    text = "jump all";
  };
  jump-container = {
    text = "jump container";
  };
  jump-workspaces = {
    text = "jump workspaces";
  };
  jump-floating = {
    text = "jump floating";
  };

  # --- zooming out, and zooming a window -------------------------------
  # The workspace scale is a compositor-side zoom of the whole scene, still
  # interactive. The content scale is one window resampled, and it is the one
  # that is easy to leave switched on by accident.
  overview = {
    text = "scale_workspace overview";
  };
  overview-all = {
    text = "scale_workspaces toggle";
  };
  zoom-out = {
    text = "scale_workspace incr -0.05";
  };
  zoom-in = {
    text = "scale_workspace incr 0.05";
  };
  zoom-reset = {
    text = "scale_workspace reset";
  };
  content-smaller = {
    text = "scale_content incr -0.05";
  };
  content-bigger = {
    text = "scale_content incr 0.05";
  };
  content-reset = {
    text = "scale_content reset";
  };

  # --- the last few windows, in the order they were focused ------------
  ring-previous = {
    text = "focus_ring prev";
  };
  ring-next = {
    text = "focus_ring next";
  };
  ring-first = {
    text = "focus_ring first";
  };
  ring-last = {
    text = "focus_ring last";
  };
  ring-set = {
    text = "focus_ring set";
  };
  ring-remove = {
    text = "focus_ring remove";
  };

  # --- workspaces -------------------------------------------------------
  # `number N` NAMES the number, so the digit on the key is the digit of the
  # workspace whether or not it existed a moment ago.
  workspace-next = {
    text = "workspace next_on_output";
  };
  workspace-previous = {
    text = "workspace prev_on_output";
  };
  workspace-next-screen = {
    text = "workspace next";
  };
  workspace-previous-screen = {
    text = "workspace prev";
  };
  workspace-back = {
    text = "workspace back_and_forth";
  };
  workspace-go = {
    text = "workspace number {n}";
    argument = "number";
  };
  workspace-move = {
    text = "move container to workspace number {n}; workspace number {n}";
    argument = "number";
  };
  workspace-move-next = {
    text = "move container to workspace next_on_output";
  };
  workspace-move-previous = {
    text = "move container to workspace prev_on_output";
  };
  workspace-swap = {
    text = "workspace swap number {n}";
    argument = "number";
  };
  workspace-split = {
    text = "workspace split";
  };
  workspace-split-reset = {
    text = "workspace split reset";
  };
  workspace-to-screen-left = {
    text = "move workspace to output left";
  };
  workspace-to-screen-right = {
    text = "move workspace to output right";
  };

  # --- the drawer a window can be put away in --------------------------
  scratchpad-put = {
    text = "move scratchpad";
  };
  scratchpad-show = {
    text = "scratchpad show";
  };
  scratchpad-jump = {
    text = "scratchpad jump";
  };

  # --- the session ------------------------------------------------------
  # `reread` is not here: it is `Compositor::reread`, because its refusal is
  # an answer somebody waits for rather than a command that is sent and
  # forgotten.
  screens-off = {
    text = "output * power off";
  };
  screens-on = {
    text = "output * power on";
  };
  keep-shortcuts = {
    text = "shortcuts_inhibitor toggle";
  };

  # --- every shortcut of the compositor's own configuration ----------
  # The pinned compositor ships a configuration with some three hundred binds
  # and fifteen modes. The session never reads it (`-c`), so what those binds
  # do is declared here, and `actions.rs` binds them. See `actions::scroll_chords`.
  close-all = {
    text = "kill all";
  };
  reload = {
    text = "reload";
  };
  move-out-beginning = {
    text = "move beginning nomode";
  };
  move-out-end = {
    text = "move end nomode";
  };
  workspace-swap-name = {
    text = "workspace swap number {n} name_only";
    argument = "number";
  };
  workspace-split-v = {
    text = "workspace split v {n} 10";
    argument = "number";
  };
  workspace-split-h = {
    text = "workspace split h {n} 10";
    argument = "number";
  };
  jump-tiling-all = {
    text = "jump tiling all";
  };
  jump-floating-all = {
    text = "jump floating all";
  };
  jump-everywhere-all = {
    text = "jump all all";
  };
  jump-trailmark = {
    text = "jump trailmark";
  };
  jump-trailmark-all = {
    text = "jump trailmark all";
  };
  filter-tiling = {
    text = "filter active tiling";
  };
  filter-tiling-all = {
    text = "filter all tiling";
  };
  filter-tiling-only = {
    text = "filter active_only tiling";
  };
  filter-container = {
    text = "filter active container";
  };
  filter-container-all = {
    text = "filter all container";
  };
  filter-container-only = {
    text = "filter active_only container";
  };
  filter-floating = {
    text = "filter active floating";
  };
  filter-floating-all = {
    text = "filter all floating";
  };
  filter-floating-only = {
    text = "filter active_only floating";
  };
  filter-trailmark = {
    text = "filter active trailmark";
  };
  filter-trailmark-all = {
    text = "filter all trailmark";
  };
  filter-trailmark-only = {
    text = "filter active_only trailmark";
  };
  filter-visible = {
    text = "filter active visible";
  };
  filter-visible-all = {
    text = "filter all visible";
  };
  filter-visible-only = {
    text = "filter active_only visible";
  };
  filter-reset = {
    text = "filter reset";
  };
  placement-after = {
    text = "set_mode after";
  };
  placement-before = {
    text = "set_mode before";
  };
  placement-beginning = {
    text = "set_mode beginning";
  };
  placement-end = {
    text = "set_mode end";
  };
  placement-focus = {
    text = "set_mode focus";
  };
  placement-no-focus = {
    text = "set_mode nofocus";
  };
  centre-horizontal = {
    text = "set_mode center_horiz";
  };
  no-centre-horizontal = {
    text = "set_mode nocenter_horiz";
  };
  centre-vertical = {
    text = "set_mode center_vert";
  };
  no-centre-vertical = {
    text = "set_mode nocenter_vert";
  };
  reorder-auto = {
    text = "set_mode reorder_auto";
  };
  no-reorder-auto = {
    text = "set_mode noreorder_auto";
  };
  fit-fraction = {
    text = "set_mode fitfraction";
  };
  fit-split = {
    text = "set_mode fitsplit";
  };
  no-fit = {
    text = "set_mode nofit";
  };
  resize-narrower = {
    text = "resize shrink width {n}px";
    argument = "number";
  };
  resize-wider = {
    text = "resize grow width {n}px";
    argument = "number";
  };
  resize-shorter = {
    text = "resize shrink height {n}px";
    argument = "number";
  };
  resize-taller = {
    text = "resize grow height {n}px";
    argument = "number";
  };
  resize-shrink-right = {
    text = "resize shrink right {n}px";
    argument = "number";
  };
  resize-grow-right = {
    text = "resize grow right {n}px";
    argument = "number";
  };
  resize-shrink-down = {
    text = "resize shrink down {n}px";
    argument = "number";
  };
  resize-grow-down = {
    text = "resize grow down {n}px";
    argument = "number";
  };
  resize-shrink-left = {
    text = "resize shrink left {n}px";
    argument = "number";
  };
  resize-grow-left = {
    text = "resize grow left {n}px";
    argument = "number";
  };
  resize-shrink-up = {
    text = "resize shrink up {n}px";
    argument = "number";
  };
  resize-grow-up = {
    text = "resize grow up {n}px";
    argument = "number";
  };
  nudge-left = {
    text = "move left {n}px";
    argument = "number";
  };
  nudge-right = {
    text = "move right {n}px";
    argument = "number";
  };
  nudge-up = {
    text = "move up {n}px";
    argument = "number";
  };
  nudge-down = {
    text = "move down {n}px";
    argument = "number";
  };
  toggle-size-this = {
    text = "toggle_size this 1.0 1.0";
  };
  toggle-size-active = {
    text = "toggle_size active 1.0 1.0";
  };
  toggle-size-width = {
    text = "toggle_size all {n} 1.0";
    argument = "number";
  };
  toggle-size-height = {
    text = "toggle_size all 1.0 {n}";
    argument = "number";
  };
  toggle-size-reset = {
    text = "toggle_size reset";
  };
  fit-rows-visible = {
    text = "fit_size v visible proportional";
  };
  fit-columns-visible-equal = {
    text = "fit_size h visible equal";
  };
  fit-rows-visible-equal = {
    text = "fit_size v visible equal";
  };
  fit-columns-to-end = {
    text = "fit_size h toend proportional";
  };
  fit-rows-to-end = {
    text = "fit_size v toend proportional";
  };
  fit-columns-to-end-equal = {
    text = "fit_size h toend equal";
  };
  fit-rows-to-end-equal = {
    text = "fit_size v toend equal";
  };
  fit-columns-to-start = {
    text = "fit_size h tobeg proportional";
  };
  fit-rows-to-start = {
    text = "fit_size v tobeg proportional";
  };
  fit-columns-to-start-equal = {
    text = "fit_size h tobeg equal";
  };
  fit-rows-to-start-equal = {
    text = "fit_size v tobeg equal";
  };
  fit-rows-active = {
    text = "fit_size v active proportional";
  };
  fit-columns-all = {
    text = "fit_size h all proportional";
  };
  fit-rows-all = {
    text = "fit_size v all proportional";
  };
  trail-number = {
    text = "trail number {n}";
    argument = "number";
  };
  trail-to-selection = {
    text = "trail to_selection";
  };
  space-load = {
    text = "space load {n}";
    argument = "number";
  };
  space-save = {
    text = "space save {n}";
    argument = "number";
  };
  space-restore-hide = {
    text = "space restore_hide {n}";
    argument = "number";
  };
  space-restore = {
    text = "space restore {n}";
    argument = "number";
  };
  mode-split = {
    text = "mode \"split\"";
  };
  mode-jump = {
    text = "mode \"jump\"";
  };
  mode-filter = {
    text = "mode \"filter\"";
  };
  mode-placement = {
    text = "mode \"placement\"";
  };
  mode-width = {
    text = "mode \"width\"";
  };
  mode-height = {
    text = "mode \"height\"";
  };
  mode-resize = {
    text = "mode \"resize\"";
  };
  mode-floating = {
    text = "mode \"floating\"";
  };
  mode-toggle-width = {
    text = "mode \"toggle-width\"";
  };
  mode-toggle-height = {
    text = "mode \"toggle-height\"";
  };
  mode-align = {
    text = "mode \"align\"";
  };
  mode-fit = {
    text = "mode \"fit\"";
  };
  mode-trailmark = {
    text = "mode \"trailmark\"";
  };
  mode-trail = {
    text = "mode \"trail\"";
  };
  mode-spaces = {
    text = "mode \"spaces\"";
  };
  mode-default = {
    text = "mode \"default\"";
  };
}
