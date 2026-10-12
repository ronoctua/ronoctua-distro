# The boot trial's watch's own checks, in the shape every module's
# `checks.nix` has (`nix/brain-modules.nix`, `checkArguments`).
{
  mkCheck,
  pkgs,
  self,
  pyPrelude,
  ...
}:

let
  vm = self.nixosConfigurations.vm.config;
in
{
  checks = {
    # THE WATCH LISTENS WHERE THE BLESS WRITES.
    #
    # Two places have to agree for a failed trial to reach the minibuffer: the
    # folder the machine module declares for the record (evaluated, and
    # readable by the person's brain: `0755`) and the constant the machine's
    # crate writes `state.json` under (`ronoctua_machine::STATE_DIR`). That
    # the watch listens on that constant is the module's own unit test
    # (`it_watches_the_records_folder`), which runs it. Either one moved and
    # the failure is never said — on the one occasion nobody was watching.
    boot-fallback-hears-the-bless =
      let
        facts = pkgs.writeText "boot-fallback-facts.json" (
          builtins.toJSON {
            stateDir = vm.ronoctua.machine.stateDir;
            tmpfiles = builtins.filter (
              r: builtins.match ".*ronoctua-machine.*" r != null
            ) vm.systemd.tmpfiles.rules;
          }
        );
      in
      mkCheck "boot-fallback-hears-the-bless" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the evaluated folder and its tmpfiles line arrive as JSON.
        # FILE-WIDE: the crate's constant, read from its file whole — comments
        # off and strings kept (`uncommented`): the needle IS a string.
        got = json.load(open("${facts}"))
        crate = uncommented(pathlib.Path("${../../../ronoctua-machine/src/lib.rs}").read_text())
        floor(len(crate), 2000, "characters of ronoctua-machine's lib.rs")
        wrong = []

        constant = re.search(r'pub const STATE_DIR: &str = "([^"]+)";', crate)
        if not constant:
            sys.exit("ERROR: `ronoctua_machine::STATE_DIR` went away; it is the folder the watch listens to")
        if constant.group(1) != got["stateDir"]:
            wrong.append(f"the crate writes under {constant.group(1)}, the system declares {got['stateDir']}")
        line = [r for r in got["tmpfiles"] if r.split()[1:2] == [got["stateDir"]]]
        if len(line) != 1 or line[0].split()[2] != "0755":
            wrong.append(f"the record's folder is not made readable by everybody: {got['tmpfiles']}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the crate writes the record under {got['stateDir']}, the folder the system makes readable")
        END
      '';
  };
  slow = [ ];
}
