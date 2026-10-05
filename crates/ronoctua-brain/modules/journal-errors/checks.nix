# The journal-errors module's own checks, in the shape every module's
# `checks.nix` has: the arguments it asks for by name
# (`nix/brain-modules.nix`, `checkArguments`), and its checks, each named after
# its id.
{
  mkCheck,
  pkgs,
  pyPrelude,
  self,
  ...
}:

let
  vm = self.nixosConfigurations.vm.config;
  brain = vm.systemd.user.services.ronoctua-brain;
in
{
  checks = {
    # THE BRAIN THE MACHINE GETS CAN READ WHAT THIS MODULE WATCHES.
    #
    # Two things have to hold on the installed machine, and neither is in this
    # folder: the brain's unit hands it journalctl by its store path (the
    # variable `ronoctua-journal` reads — a unit's `PATH` is minimal, and a
    # bare name would fail far from here), and the person may read the
    # SYSTEM's journal, which journald's ACL gives `wheel`. Without the first
    # the page says the journal is not followed; without the second the
    # follower reads the person's own journal alone and a failing system
    # service is never an error here — which no test of the module sees,
    # because every one of them hands it the lines.
    journal-errors-the-brain-reads-the-journal =
      let
        facts = pkgs.writeText "journal-errors-machine.json" (
          builtins.toJSON {
            # AS WORDS: the path is compared by its name, and a string that
            # quoted the store would make this check build systemd.
            journalctl = builtins.unsafeDiscardStringContext (brain.environment.RONOCTUA_JOURNALCTL or "");
            groups = vm.users.users.${vm.ronoctua.user}.extraGroups or [ ];
          }
        );
      in
      mkCheck "journal-errors-the-brain-reads-the-journal" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the machine's unit and the person's groups arrive as the
        # evaluated configuration, JSON; the variable's name is the journal
        # crate's constant, strings kept.
        # FILE-WIDE: the variable's name is one module-level `pub const`.
        facts = json.load(open("${facts}"))
        journal = uncommented(pathlib.Path("${../../../ronoctua-journal/src/lib.rs}").read_text())
        floor(len(journal), 500, "characters of ronoctua-journal's lib.rs")
        variable = re.search(r'pub const VARIABLE: &str = "([^"]+)";', journal)
        if not variable:
            sys.exit("ERROR: `ronoctua_journal::VARIABLE` went away")
        wrong = []
        if variable.group(1) != "RONOCTUA_JOURNALCTL":
            wrong.append(f"the journal crate reads `{variable.group(1)}`, and the brain's unit sets RONOCTUA_JOURNALCTL")
        path = facts["journalctl"]
        if not re.fullmatch(r"/nix/store/[^/]+/bin/journalctl", path):
            wrong.append(f"the brain's unit does not hand it journalctl by its store path (it hands {path!r})")
        if "wheel" not in facts["groups"]:
            wrong.append("the person is not in `wheel`, so the system's journal is closed to their brain")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the brain is handed journalctl by its store path, and the person may read the system's journal")
        END
      '';
  };
  slow = [ ];
}
