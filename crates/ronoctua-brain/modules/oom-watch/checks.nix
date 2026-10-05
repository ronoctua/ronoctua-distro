# The oom-watch module's own checks, in the shape every module's `checks.nix`
# has: the arguments it asks for by name (`nix/brain-modules.nix`,
# `checkArguments`), and its checks, each named after its id.
{
  mkCheck,
  pkgs,
  pyPrelude,
  languages,
  brainModules,
  ...
}:

let
  entry = brainModules.notifications."oom-watch-killed";
in
{
  checks = {
    # EVERY WAY TO DIE HAS ITS SENTENCE, in every language.
    #
    # The notification's body selects on `$by`, a word the rules give
    # (`By::word`: the kernel for the machine or for a group's limit, oomd for
    # pressure or for fullness, and the two a unit's line alone can say). A
    # word with no variant is not an error anywhere: Fluent falls to the
    # default and the person reads that oomd closed a program the kernel
    # killed. And the process line selects on `$pid`, which is `0` when no
    # kernel line was heard — a numeric branch, which only works if it is
    # there.
    oom-watch-every-kill-has-its-sentence =
      mkCheck "oom-watch-every-kill-has-its-sentence"
        [
          pkgs.python3
        ]
        ''
          python3 - <<'END'
          import pathlib, re, sys
          exec(open("${pyPrelude}").read())

          # PARSED: the words out of `By::word`'s match arms (strings kept,
          # comments off), and each fragment's body key, from its line to the next
          # key at the left margin.
          rules = uncommented(pathlib.Path("${./rules.rs}").read_text())
          word = must(body_of(rules, "word"), "`By::word`")
          words = re.findall(r'Self::\w+\s*=>\s*"([a-z]+)"', word)
          floor(len(words), 6, "ways to die in `By::word`")
          wrong = []
          for language in ${builtins.toJSON languages}:
              text = pathlib.Path("${./i18n}", language + ".ftl").read_text()
              found = re.search(r"^notif-oom-watch-killed-body =(.*?)(?=^[a-z]|\Z)", text, re.M | re.S)
              if not found:
                  wrong.append(f"{language}: the notification has no body")
                  continue
              body = found.group(1)
              for w in words:
                  if not re.search(r"\*?\[" + re.escape(w) + r"\]", body):
                      wrong.append(f"{language}: no variant for a kill `by` `{w}`")
              if "[0]" not in body:
                  wrong.append(f"{language}: no branch for a kill with no process (`$pid` 0)")
          if wrong:
              for w in wrong:
                  print("ERROR: " + w, file=sys.stderr)
              sys.exit(1)
          print(f"OK: {len(words)} ways to die, each with its sentence in every language")
          END
        '';

    # A KILL THE BRAIN TELLS IS A KILL THE FOOTER ALREADY KNOWS HOW TO MERGE.
    #
    # oomd's kill reaches the footer twice: its bus signal (the footer's own
    # `memory-killed` notification) and this module's event. The footer adds
    # the event's details to the notification it raised instead of raising a
    # second one — and it recognises the event by the catalogue's tag, and the
    # kill by two of the event's values. Three names in three places, and a
    # rename of any one of them is two notifications for one kill, which no
    # test of either side sees alone.
    oom-watch-joins-the-bus-notice =
      let
        facts = pkgs.writeText "oom-watch-entry.json" (builtins.toJSON { inherit (entry) tags; });
      in
      mkCheck "oom-watch-joins-the-bus-notice" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the catalogue entry arrives as JSON from the loader; the
        # footer's tag is its constant, the event's values are `args_of`'s,
        # and the names the footer matches by are `ronoctua_notify::oom`'s — strings kept.
        tags = json.load(open("${facts}"))["tags"]
        surface = uncommented(pathlib.Path("${../../../ronoctua-shell/src/surface.rs}").read_text())
        oom = uncommented(pathlib.Path("${../../../ronoctua-notify/src/oom.rs}").read_text())
        module = uncommented(pathlib.Path("${./module.rs}").read_text())
        wrong = []
        tag = re.search(r'pub const MEMORY_KILLED: &str = "([^"]+)";', surface)
        if not tag:
            sys.exit("ERROR: the footer's `MEMORY_KILLED` went away")
        if tag.group(1) not in tags:
            wrong.append(f"the event's notification is not tagged `{tag.group(1)}`, which the footer merges on")
        given = must(body_of(module, "args_of"), "the module's `args_of`")
        for const in ("CGROUP_ARG", "EPOCH_ARG"):
            found = re.search(r"pub const " + const + r': &str = "([^"]+)";', oom)
            if not found:
                wrong.append(f"`ronoctua_notify::oom::{const}` went away")
                continue
            if f'("{found.group(1)}",' not in given:
                wrong.append(f"the event does not carry `{found.group(1)}`, which the footer matches a kill by")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the event is tagged as the footer's notice and carries what the footer matches it by")
        END
      '';
  };
  slow = [ ];
}
