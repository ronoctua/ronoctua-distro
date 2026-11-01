# THE PERSON'S BRAIN MODULES REACH A MACHINE (owner, 2026-11-01): a machine
# compiles every `my-…` module its approvals include — through
# `withPersonModules`, which builds the daemon and the footer's data again —
# and reads nothing of one that waits for an approval.
#
#   - `machine-compiles-approved-person-modules`: two machines evaluated as
#     `lib.machine` builds every installed one. `tests/fixtures/machine-brain/`
#     holds four folders it includes (two with no NixOS half, two whose half
#     an approval names as it is) and two that wait (no approval; one edited
#     after its approval), whose manifests `throw`. Its brain is the daemon
#     that chain compiled for exactly the four, never the distro's, and the
#     folder that daemon was built from holds the four and no other; its
#     footer reads that chain's data. `tests/fixtures/machine-brain-waiting/`
#     holds ONE module, which waits: its brain and its footer are the
#     distro's own, the same derivations, its NixOS half is not imported, and
#     the report names it with the hash an approval must name. Evaluating the
#     brain's program forces every manifest the chain reads, so a waiting
#     module read at all is the evaluation's own error.
#
#   - `a-folder-that-does-not-evaluate-stops-only-itself`: a person's
#     manifest that does not parse, or aborts, is an error `tryEval` cannot
#     catch, and the machine reads a folder only once root evaluated it ALONE
#     (`local/brain-checked.json`). Two halves:
#       1. the machine `tests/fixtures/machine-brain-broken/` — one module
#          that loads, one the loader refuses, one nobody checked yet, one
#          whose manifest Nix's parser refuses and one that aborts, as root's
#          check recorded them — EVALUATES: its brain is compiled with the
#          two that evaluated, and its report says what it compiled and why
#          each other one is out (`does-not-evaluate` with Nix's words,
#          `unchecked`, `not-a-module` with the loader's);
#       2. root's real check (`ronoctua-person check-brain`, the program the
#          units run, with a real `nix` over a store of its own) over a copy
#          of that machine, a SYNTAX ERROR planted beside them (no file of
#          the tree may hold one): each folder in a process of its own, the
#          broken ones recorded as not evaluating with Nix's error line, the
#          others as evaluating — at the hashes, and with the verdicts, the
#          fixture's record says.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  mkCheck,
  machineLib,
  withPersonModules,
  rust,
}:
let
  plain = v: builtins.unsafeDiscardStringContext (toString v);
  brainOf = config: plain config.systemd.user.services.ronoctua-brain.serviceConfig.ExecStart;
  footerOf = config: plain config.systemd.user.services.ronoctua-footer.serviceConfig.ExecStart;
  brainProgram = r: plain "${r.brain}/bin/ronoctua-brain";
  footerProgram = r: plain "${r.shell}/bin/ronoctua-shell";

  # A MACHINE WHOSE APPROVALS INCLUDE FOUR FOLDERS, and the chain built for
  # exactly those — what `lib.machine` must have used.
  approved = ./fixtures/machine-brain;
  approvedConfig = (machineLib.machine { dir = approved; }).config;
  included = (machineLib.approvals approved).includedIds;
  chain = withPersonModules {
    dir = approved + "/person/brain";
    only = included;
  };
  # The folder the daemon's registry was generated from: the person's copy
  # with only the modules that load.
  copy = chain.brainModules.personModulesDir;

  # A MACHINE WHOSE ONE MODULE WAITS.
  waiting = ./fixtures/machine-brain-waiting;
  waitingConfig = (machineLib.machine { dir = waiting; }).config;

  facts = pkgs.writeText "person-brain-facts.json" (
    builtins.toJSON {
      approved = {
        inherit included;
        compiled = chain.brainModules.personIds;
        copied = if copy == null then [ ] else builtins.attrNames (builtins.readDir copy);
        brain = brainOf approvedConfig;
        chainBrain = brainProgram chain.rust;
        footer = footerOf approvedConfig;
        chainFooter = footerProgram chain.rust;
        etc = builtins.filter (lib.hasPrefix "ronoctua-fixture-") (
          builtins.attrNames approvedConfig.environment.etc
        );
      };
      waiting = {
        included = (machineLib.approvals waiting).includedIds;
        brain = brainOf waitingConfig;
        footer = footerOf waitingConfig;
        etc = builtins.filter (lib.hasPrefix "ronoctua-fixture-") (
          builtins.attrNames waitingConfig.environment.etc
        );
        report = waitingConfig.ronoctua.person.report.brain;
        hash = import ../nix/person-hash.nix (waiting + "/person/brain/my-waiting");
      };
      distroBrain = brainProgram rust;
      distroFooter = footerProgram rust;
    }
  );

  # A MACHINE SOME OF WHOSE FOLDERS DO NOT EVALUATE, as root's check
  # recorded them.
  broken = ./fixtures/machine-brain-broken;
  brokenConfig = (machineLib.machine { dir = broken; }).config;
  brokenIncluded = (machineLib.approvals broken).includedIds;
  brokenChain = withPersonModules {
    dir = broken + "/person/brain";
    only = brokenIncluded;
  };
  brokenFacts = pkgs.writeText "person-brain-broken-facts.json" (
    builtins.toJSON {
      included = brokenIncluded;
      brain = brainOf brokenConfig;
      chainBrain = brainProgram brokenChain.rust;
      report = brokenConfig.ronoctua.person.report.brain;
      recorded = builtins.fromJSON (builtins.readFile (broken + "/local/brain-checked.json"));
    }
  );
  # What root's `nix eval` of one folder runs (`nix/modules/person-apply.nix`).
  checkBundle = pkgs.callPackage ../nix/person-brain-check-bundle.nix { };
in
{
  machine-compiles-approved-person-modules =
    mkCheck "machine-compiles-approved-person-modules" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import json, sys

        # PARSED: two evaluated machines and the chain, as JSON.
        f = json.load(open("${facts}"))
        a, w = f["approved"], f["waiting"]
        wrong = []
        expected = ["my-approved", "my-common", "my-plain", "my-reaching"]
        if sorted(a["included"]) != expected:
            wrong.append(f"the fixture's approvals include {a['included']}: the check proves nothing")
        if sorted(a["compiled"]) != expected:
            wrong.append(f"the chain compiles {a['compiled']}, not the four included folders")
        if sorted(a["copied"]) != expected:
            wrong.append(f"the daemon is built from a folder holding {a['copied']}")
        if a["brain"] != a["chainBrain"]:
            wrong.append(f"the machine runs the brain {a['brain']}, not the one compiled with its modules")
        if a["brain"] == f["distroBrain"]:
            wrong.append("the machine with four modules of the person's runs the distro's brain: none was compiled")
        if a["footer"] != a["chainFooter"] or a["footer"] == f["distroFooter"]:
            wrong.append(f"the machine's footer is {a['footer']}, not the one reading its modules' data")
        if a["etc"] != ["ronoctua-fixture-my-approved", "ronoctua-fixture-my-reaching"]:
            wrong.append(f"the NixOS halves imported: {a['etc']}")
        if w["included"] != []:
            wrong.append(f"the waiting fixture includes {w['included']}: the check proves nothing")
        if w["brain"] != f["distroBrain"] or w["footer"] != f["distroFooter"]:
            wrong.append("a machine whose one module waits does not run the distro's own brain and footer")
        if w["etc"]:
            wrong.append(f"a waiting module's NixOS half was imported: {w['etc']}")
        awaiting = w["report"]["awaiting-approval"]
        if w["report"]["included"] or awaiting != [{"id": "my-waiting", "hash": w["hash"]}]:
            wrong.append(f"the waiting machine's report says {w['report']}")
        if wrong:
            print("ERROR: a machine does not compile exactly the person's modules it includes.",
                  file=sys.stderr)
            for x in wrong:
                print("       " + x, file=sys.stderr)
            raise SystemExit(1)
        print("OK: the approved and plain modules are compiled into the machine's brain and "
              "footer; a module that waits is neither read nor compiled nor imported")
        END
      '';

  a-folder-that-does-not-evaluate-stops-only-itself =
    mkCheck "a-folder-that-does-not-evaluate-stops-only-itself" [ pkgs.python3 pkgs.nix ]
      ''
        # 2. ROOT'S REAL CHECK over a copy of the machine, with a syntax error
        # planted beside its folders, a real `nix` and a store of its own.
        work=$(mktemp -d)
        cp -r ${broken} "$work/machine"
        cp -r ${checkBundle} "$work/bundle"
        chmod -R u+w,go-w "$work/machine" "$work/bundle"
        rm -f "$work/machine/local/brain-checked.json"
        cp -r "$work/machine/person/brain/my-fine" "$work/machine/person/brain/my-syntax"
        printf '{ what = ; }\n' > "$work/machine/person/brain/my-syntax/manifest.nix"
        sed -i 's/my-fine/my-syntax/g' "$work/machine/person/brain/my-syntax/i18n/"*.ftl
        chmod -R go-w "$work/machine"
        mkdir -p "$work/home" "$work/store"
        HOME="$work/home" \
          NIX_REMOTE="local?root=$work/store" \
          NIX_CONFIG=$'experimental-features = nix-command\nsubstituters =\nbuild-users-group =\n' \
          RONOCTUA_NIX=${pkgs.nix}/bin/nix \
          RONOCTUA_PERSON_BRAIN_CHECK="$work/bundle/check.nix" \
          ${rust.person}/bin/ronoctua-person check-brain --machine "$work/machine" --lock "$work/lock" \
          > "$work/said" 2>&1 || { cat "$work/said" >&2; echo "ERROR: root's check refused" >&2; exit 1; }

        python3 - "$work/machine/local/brain-checked.json" <<'END'
        import json, sys

        # PARSED: the evaluated machine, and the file root's check wrote.
        f = json.load(open("${brokenFacts}"))
        checked = json.load(open(sys.argv[1]))["modules"]
        recorded = f["recorded"]["modules"]
        wrong = []

        # 1. THE MACHINE EVALUATES, and says what it compiled and why not.
        if sorted(f["included"]) != ["my-fine", "my-incomplete"]:
            wrong.append(f"the machine lets the loader read {f['included']}")
        if f["brain"] != f["chainBrain"]:
            wrong.append("the machine's brain is not the one compiled with what evaluated alone")
        report = f["report"]
        if report["included"] != ["my-fine"]:
            wrong.append(f"the report says {report['included']} were compiled")
        refused = {r["id"]: r for r in report["refused"]}
        if sorted(refused) != ["my-aborts", "my-incomplete", "my-unchecked", "my-unparsed"]:
            wrong.append(f"the report refuses {sorted(refused)}")
        for id in ("my-aborts", "my-unparsed"):
            r = refused.get(id, {})
            if r.get("why") != "does-not-evaluate" or r.get("detail") != recorded[id]["detail"]:
                wrong.append(f"{id} is reported as {r}, not as root's check recorded it")
        if refused.get("my-unchecked", {}).get("why") != "unchecked":
            wrong.append(f"a folder nobody checked is reported as {refused.get('my-unchecked')}")
        incomplete = refused.get("my-incomplete", {})
        if incomplete.get("why") != "not-a-module" or "module.rs" not in incomplete.get("detail", ""):
            wrong.append(f"the folder the loader refuses is reported as {incomplete}")

        # 2. ROOT'S CHECK, each folder alone: what the fixture records.
        want = ["my-aborts", "my-fine", "my-incomplete", "my-syntax", "my-unchecked", "my-unparsed"]
        if sorted(checked) != want:
            wrong.append(f"root's check evaluated {sorted(checked)}; {want} are the folders")
        said = {
            "my-unparsed": "undefined variable",
            "my-aborts": "aborted",
            "my-syntax": "syntax error",
        }
        for id, c in sorted(checked.items()):
            if id in said:
                if c["evaluates"] or said[id] not in c["detail"]:
                    wrong.append(f"{id} is recorded as {c}: it does not evaluate, and Nix says so")
            elif not c["evaluates"] or c["detail"]:
                wrong.append(f"{id} is recorded as {c}: it evaluates alone")
            r = recorded.get(id)
            if r is not None and (r["hash"], r["evaluates"]) != (c["hash"], c["evaluates"]):
                wrong.append(f"{id}: the fixture records {r}, and root's check writes {c}")
        if sorted(recorded) != ["my-aborts", "my-fine", "my-incomplete", "my-unparsed"]:
            wrong.append(f"the fixture's record names {sorted(recorded)}: the check proves less")
        if wrong:
            print("ERROR: a folder that does not evaluate reaches further than itself.", file=sys.stderr)
            for x in wrong:
                print("       " + x, file=sys.stderr)
            raise SystemExit(1)
        print("OK: a manifest that does not parse, or aborts, stops root's process for that folder "
              "alone; the machine evaluates without it and reports it refused, with Nix's words")
        END
      '';
}
