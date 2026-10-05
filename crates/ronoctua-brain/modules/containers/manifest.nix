# THE PERSON'S RUNNING CONTAINERS, counted on the modeline.
#
# A watch and nothing else: no calendar, no unit, no action of its own. crun
# keeps one folder per running container under `$XDG_RUNTIME_DIR/crun` and
# takes it away when the container ends, so the kernel tells the brain
# (inotify) and a count is one `readdir` — no process, nothing at rest. Why
# each is so is in `why.md`, beside this.
{
  what = "Mark the modeline while any of the person's containers runs, with how many.";

  needs = {
    root = false;
    network = false;
    # WHERE crun KEEPS ITS FOLDERS comes from the one crate that knows podman
    # (`runtime_dir`); the module reads a directory and runs nothing.
    boundaries = [ "ronoctua-containers" ];
  };

  # A WATCH: nothing runs, so nothing is scheduled.
  schedule = null;

  # A CONTAINER, the same drawing the containers' view wears in its kinds
  # column: a box, not a whale — no brand on the modeline.
  glyphs = {
    running = "md-cube_outline";
  };

  # THE ONE MARK, and only while it says something (invariant 14): while at
  # least one container runs, in the colour of a watch, with the count. A
  # click opens the containers' view (`containers-open`, `LEADER s o`) — the
  # mark's command, which the module names and the shell runs by name.
  marks = {
    running = {
      glyph = "running";
      hint = "containers-mark-running";
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the boundary crate that says where
  # crun's folders are, and the containers' own NixOS module, which is what
  # puts crun there at all.
  uses = [
    "crates/ronoctua-containers"
    "nix/modules/containers.nix"
  ];

  # THE CONTAINERS' VIEW SPEAKS IN THIS MODULE'S WORDS: the app and the module
  # are one feature, and `gen-i18n.py` leaves it to this list whether a file
  # outside the folder may say `containers-…` (brain.md §4, step 5). Each file
  # below is the app's — its view, its two doors, its registry entries, its
  # texts, its checks — and names the module's namespace because the app's
  # own keys and commands start with the same word.
  #
  # Outside the app, what names a word of its namespace by coincidence of the
  # feature's name, listed name by name: the boundary crate and its NixOS
  # module mention the VM test (`containers-vm`) and the image option
  # (`containers-image`), and the icon table holds the view's glyphs.
  namedBy = {
    "assets/i18n/en-GB.ftl" = "*";
    "assets/i18n/pt-BR.ftl" = "*";
    "assets/tokens/icons.nix" = "*";
    "crates/ronoctua-containers/src/fixtures.rs" = [ "containers-vm" ];
    "crates/ronoctua-containers/src/lib.rs" = [ "containers-vm" ];
    "nix/modules/containers.nix" = [ "containers-image" ];
    "crates/ronoctua-shell/src/containers.rs" = "*";
    "crates/ronoctua-shell/src/containers_feed.rs" = "*";
    "crates/ronoctua-shell/src/containers_photograph.rs" = "*";
    "crates/ronoctua-shell/src/leader.rs" = "*";
    "crates/ronoctua-shell/src/main.rs" = "*";
    "crates/ronoctua-shell/src/photograph.rs" = "*";
    "crates/ronoctua-shell/src/surface.rs" = "*";
    "tests/checks.nix" = "*";
    "tests/containers-vm.nix" = "*";
  };
}
