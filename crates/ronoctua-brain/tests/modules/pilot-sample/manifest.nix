# THE SAMPLE MODULE (brain.md §7, S0): a module folder as the contract expects
# it, small enough to read at once.
#
# It is NEVER REGISTERED, and that is why it lives under `tests/modules/` and
# not under `modules/`: the loader (`nix/brain-modules.nix`) reads only the
# second. It is the generator's golden fixture — this file in, `manifest.rs`
# beside it out, byte for byte below that file's header, which is what
# `brain-modules-match-the-golden` compares — and the brain's own test loads
# it through the same `#[path]` shim every module gets (`build.rs`, `pilot.rs`).
#
# THE FOLDER IS COMPLETE — `module.nix`, `i18n/<lang>.ftl`, `checks.nix` and
# `why.md` beside this — so the loader, the text generator and the sealed
# check can be run on it exactly as they run on a real one.
{
  what = "A sample: mark the modeline while the disk is nearly full.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = {
    kind = "routine";
    calendar = false;
    intervalDays = 7;
    partOfDay = "any";
    everyMinutes = null;
    countdownSeconds = 0;
    runawayMinutes = 60;
    gated = false;
  };
  units = { };
  keys = [ "pilot-sample" ];
  whys = [ "when-empty" ];
  glyphs = {
    full = "md-harddisk";
  };
  marks = {
    full = {
      glyph = "full";
      hint = "pilot-sample-mark-full";
    };
  };
  notices = {
    full = {
      level = "warn";
      dismiss = true;
      snooze = [ 60 ];
    };
  };
  actions = {
    forget = {
      row = true;
    };
  };
  events = {
    emptied = {
      notification = null;
    };
  };
  uses = [ ];
  namedBy = { };
}
