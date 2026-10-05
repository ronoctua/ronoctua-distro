# The security watch's own checks, in the shape every module's `checks.nix`
# has (`nix/brain-modules.nix`, `checkArguments`).
{
  mkCheck,
  pkgs,
  lib,
  self,
  pyPrelude,
  ...
}:

let
  vm = self.nixosConfigurations.vm.config;
  # THE RULES AS THE MACHINE GETS THEM: the evaluated firewall's tail, where
  # the declared table logs what it is about to refuse.
  rules = builtins.unsafeDiscardStringContext vm.networking.firewall.extraInputRules;
in
{
  checks = {
    # THE WATCH HEARS WHAT THE FIREWALL LOGS.
    #
    # Three places have to agree for a refused connection to reach the
    # modeline: the prefix the declared firewall logs with (`security.nix`,
    # evaluated), the constant the root helper's crate names
    # (`firewall::LOG_PREFIX`), and the match this module asks the journal
    # for — the kernel's transport, that prefix, and a priority at least as
    # loose as the level the rule logs at. Any one of them moved and the page
    # says "nothing refused" on a machine that refuses all day. The rule is
    # read as the machine gets it; the match is read out of `sources`.
    security-watch-hears-what-the-firewall-logs =
      let
        facts = pkgs.writeText "security-watch-rules.json" (builtins.toJSON { inherit rules; });
      in
      mkCheck "security-watch-hears-what-the-firewall-logs" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the firewall's rules arrive as the evaluated configuration,
        # JSON. FILE-WIDE over two items: the crate's constant and the body of
        # the module's `sources` (`body_of`), comments off and strings kept
        # (`uncommented`) — the needles ARE strings.
        rules = json.load(open("${facts}"))["rules"]
        floor(len(rules), 100, "characters of the declared firewall's input rules")
        crate = uncommented(pathlib.Path("${../../../ronoctua-security/src/firewall.rs}").read_text())
        module = uncommented(pathlib.Path("${./module.rs}").read_text())
        journal = uncommented(pathlib.Path("${../../../ronoctua-journal/src/lib.rs}").read_text())
        wrong = []

        constant = re.search(r'pub const LOG_PREFIX: &str = "([^"]+)";', crate)
        if not constant:
            sys.exit("ERROR: `firewall::LOG_PREFIX` went away; it is the word the watch listens for")
        prefix = constant.group(1)
        logged = re.findall(r'log level (\w+) prefix "([^"]+)"', rules)
        floor(len(logged), 1, "logging rules in the declared firewall")
        for level, said in logged:
            if said != prefix:
                wrong.append(f"the firewall logs with {said!r}, and the watch listens for {prefix!r}")

        sources = must(body_of(module, "sources"), "the module's `sources`")
        found = re.search(r"Source::Journal\(Match \{(.*?)\}\),", sources, re.S)
        first = must(found and found.group(1), "the first journal match")
        if "ronoctua_security::firewall::LOG_PREFIX" not in first:
            wrong.append("the refused match does not listen for `firewall::LOG_PREFIX`")
        if '"_TRANSPORT", &["kernel"]' not in first:
            wrong.append("the refused match does not ask for the kernel's transport")
        taken = re.search(r"max_priority: Some\(ronoctua_journal::(\w+)\)", first)
        levels = {n: int(v) for n, v in re.findall(r"pub const (\w+): Priority = (\d+);", journal)}
        syslog = {"emerg": 0, "alert": 1, "crit": 2, "err": 3, "warn": 4, "notice": 5, "info": 6, "debug": 7}
        if not taken or taken.group(1) not in levels:
            wrong.append("the refused match names no priority of `ronoctua-journal`")
        else:
            for level, _ in logged:
                if syslog.get(level, 99) > levels[taken.group(1)]:
                    wrong.append(f"the firewall logs at {level}, and the watch reads only up to {taken.group(1)}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the firewall logs {len(logged)} rules with {prefix!r}, and the watch hears them")
        END
      '';
  };
  slow = [ ];
}
