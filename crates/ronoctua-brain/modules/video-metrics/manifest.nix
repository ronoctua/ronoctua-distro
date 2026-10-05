# THE VIDEO CATALOGUE IS SCANNED EVERY FEW HOURS, and the modeline says so
# while it is.
#
# The scan is the app's (`crates/ronoctua-video-metrics`, its unit in
# `nix/modules/video-metrics.nix`); the brain owns the WHEN: a cadence, the
# network, the power saver, and the one mark. Why each is so is in `why.md`,
# beside this.
{
  what = "Scan the person's YouTube channels for new videos every few hours; mark the modeline while it does.";

  needs = {
    root = false;
    network = true;
    # THE CATALOGUE'S LAST SCAN comes through the one crate that holds SQL
    # (`read::Catalog`, read-only); whether there is a way out through the
    # one that speaks NetworkManager; whether the power saver is on through
    # the one that speaks power-profiles-daemon.
    boundaries = [
      "ronoctua-video-metrics"
      "ronoctua-network"
      "ronoctua-power"
    ];
  };

  # A CADENCE AND NO CALENDAR: new videos have no good hour, they have an
  # interval — six hours, the design's. No countdown: a scan changes nothing
  # the person is looking at. INSIDE THE GATE: a scan that waits for a quiet
  # machine loses nothing, and details are asked slowly on purpose. An hour
  # and a half of patience: the report is written at the start and at the end,
  # and sixty details four to six seconds apart, the feeds and the thumbnails
  # of a large list are minutes, not hours.
  schedule = {
    kind = "routine";
    calendar = false;
    partOfDay = "any";
    everyMinutes = 360;
    countdownSeconds = 0;
    runawayMinutes = 90;
    gated = true;
  };

  # ONE UNIT AND NO REHEARSAL: a scan that changed nothing would be a scan
  # that asked YouTube for nothing, and asking is the whole of it. A brain that
  # only rehearses (`.#dev`) starts nothing and says so.
  units = {
    run = "ronoctua-video-metrics-scan.service";
  };

  # A CHANNEL ICON FOR THE SCAN, and YouTube's own drawing, in the danger
  # colour, for a scan in which every channel failed.
  glyphs = {
    scanning = "md-youtube_subscription";
    failed = "md-youtube";
  };

  # TWO MARKS, never both, and neither at rest (invariant 14): while a scan
  # runs, and — until a scan succeeds for some channel — when the last one
  # failed for every channel. A click opens this module's page.
  marks = {
    scanning = {
      glyph = "scanning";
      hint = "video-metrics-brain-mark-scanning";
    };
    failed = {
      glyph = "failed";
      hint = "video-metrics-brain-mark-failed";
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the scanner, and the unit that runs it.
  uses = [
    "crates/ronoctua-video-metrics"
    "nix/modules/video-metrics.nix"
  ];

  # THE APP CARRIES THE MODULE'S NAME — it is the page of the same name the
  # module schedules for (`gen-i18n.py` says a base text in a module's
  # namespace is this list's question): its door, its words, its unit, its
  # catalogue and the checks and tests about them.
  #
  # AND THE FILES THAT SAY THE APP'S NAME in prose or a path — its crate's
  # own, the crates that read its catalogue or ask YouTube for it, the unit's
  # import — and the shared files that say it once, each with the one name it
  # says. A name that goes takes its line here with it: the sealed check calls
  # an allowance that no longer occurs stale.
  namedBy = {
    "crates/ronoctua-video-metrics/src/lib.rs" = "*";
    "crates/ronoctua-video-metrics/src/main.rs" = "*";
    "crates/ronoctua-video-metrics/src/thumbs.rs" = "*";
    "crates/ronoctua-video-metrics/src/db.rs" = [ "video-metrics" ];
    "crates/ronoctua-video-metrics/src/person.rs" = [ "video-metrics" ];
    "crates/ronoctua-ytdlp/src/lib.rs" = [ "video-metrics" ];
    "crates/ronoctua-ytdlp/src/youtube.rs" = [ "video-metrics" ];
    "crates/ronoctua-consent/src/lib.rs" = [ "video-metrics" ];
    "crates/ronoctua-index/src/lib.rs" = [ "video-metrics" ];
    "crates/ronoctua-index/src/videos.rs" = [ "video-metrics" ];
    "crates/ronoctua-shell/Cargo.toml" = [ "video-metrics-readers-are-read-only" ];
    "Cargo.toml" = [ "video-metrics" ];
    "flake.nix" = [ "video-metrics" ];
    "nix/rust.nix" = [ "video-metrics" ];
    "assets/cache.nix" = [ "video-metrics" ];
    "assets/tokens/icons.nix" = [ "video-metrics" ];
    "assets/tokens/space.nix" = [ "video-metrics" ];
    "crates/ronoctua-shell/src/video_metrics.rs" = "*";
    "crates/ronoctua-shell/src/video_metrics_feed.rs" = "*";
    "crates/ronoctua-shell/src/video_metrics_photograph.rs" = "*";
    "crates/ronoctua-shell/src/video_metrics_window.rs" = "*";
    "crates/ronoctua-shell/src/video_metrics_words.rs" = "*";
    "crates/ronoctua-shell/src/player_ranking.rs" = "*";
    "crates/ronoctua-shell/src/main.rs" = "*";
    "crates/ronoctua-shell/src/leader.rs" = "*";
    "crates/ronoctua-shell/src/surface.rs" = "*";
    "crates/ronoctua-shell/src/photograph.rs" = "*";
    "assets/i18n/en-GB.ftl" = "*";
    "assets/i18n/pt-BR.ftl" = "*";
    "nix/cache.nix" = "*";
    "nix/modules/video-metrics.nix" = "*";
    "tests/checks.nix" = "*";
    "tests/video-metrics-vm.nix" = "*";
  };
}
