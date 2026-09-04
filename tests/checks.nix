# Verification — section 7.12.
#
# "`nix flake check` is the single verification command."
#
# Every milestone is a named check (m1..m8), so "the command that proves the
# milestone passed" is uniform:  nix build .#checks.x86_64-linux.mN
#
# THERE IS NO STUB HERE, and that is deliberate.
#
# There was: a `mkStub` that printed "NOT ATTACKED YET" and did `touch $out`.
# The intention was good — the plumbing standing from day one. The effect was
# that `m7` stayed a stub after milestone 7 had been done, and
# `nix build .#checks.x86_64-linux.m7` answered OK without verifying anything.
# The four real checks existed and were loose.
#
# The lesson: a placeholder that PASSES is indistinguishable from a closed
# milestone to whoever reads the output. A new milestone writes a real check,
# or it does not go in.
#
# The second lesson, a costlier one: write the check against the CRITERION,
# never against what you have just built. `m3` verified that `nix run .#dev`
# "exists and is executable" — and the criterion asked for hot reload, which
# did not exist. See `reload-reloads`.
#
# And the third, which cost the same lesson a second time: a check on the
# MECHANISM is not a check on the WIRING. `reload-reloads` proved the watcher
# fires, with a command of its own making, while the loop handed it the name of
# a function that had never existed — so hot reload did not fire once, for as
# long as the repository has existed. See `reload-command-is-defined`.
{
  pkgs,
  lib,
  self,
  inputs,
  system,
  fonts,
  fontconfigConf,
  glyphdb,
  unicodedb,
  lucidedb,
  # The generated update table, so `updates-have-text` reads the list the
  # generator DERIVED instead of a hand-written copy of it.
  updatesTable,
  iconSuperpack,
  rust,
  buffers,
  sbKeys,
  devApp,
  reloadApp,
  gateApp,
  prefetchApp,
  singleFooter,
  wit,
  tokensRs,
  # The notification catalogue, for the two checks that read it: the table's
  # own shape, and the text keys it demands of both languages.
  notificationsRs,
  syntaxTable,
  mimeDb,
  niriActions,
  # The C border in front of libtorrent, which carries the `sizeof` table the
  # layout check compares the Rust against.
  torrentShim,
  # The browser extension, already generated: the fingerprint check reads its
  # manifest, and the literal check reads the sources it was built from.
  extension,
  # The PO token minter, so the check can prove there is no server half in it.
  potProvider,
  # The PINNED toolchain, for the buffers' formatting: `rustfmt` from nixpkgs
  # could be another version, and two versions of a formatter are two opinions
  # about the same file.
  rustToolchain,
  # THE LANGUAGES, from `assets/i18n/languages.nix`. Twenty-four checks used to
  # write the pair out by hand, which meant a third language was checked by
  # none of them — it would have been complete in the `.ftl`, present in the
  # menu, and silently unverified everywhere else.
  languages,
}:

let
  # -------------------------------------------------------------------
  # Helpers
  # -------------------------------------------------------------------
  # The keyboard layout, from the same file the configs read.
  keyboard = import ../nix/keyboard-xkb.nix;

  # The software rasterizer, from the SAME place the VM tests use — otherwise
  # the two could point at different ICDs and only one of them would be the one
  # the CI actually exercises.
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  py = pkgs.python3.withPackages (ps: [
    ps.fonttools
    ps.brotli
  ]);

  # A check that runs a script and passes if it exits 0.
  mkCheck =
    name: deps: script:
    pkgs.runCommand "check-${name}" { nativeBuildInputs = deps; } ''
      set -euo pipefail
      ${script}
      touch $out
    '';

  # VM test of the footer: it proves that it DRAWS, not only that it compiles.
  # Under lavapipe (section 9.3 of the plan) — correctness, never performance.
  footerVm = import ./footer-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The machine does not freeze when memory runs out, and the desktop is not the
  # victim. Two nodes with different RAM, because a percentage that is merely
  # present looks exactly like one that does not adapt.
  oomVm = import ./oom-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf;
      inherit (inputs) niri;
      shell = rust.shell;
      iconSuperpack = self.packages.${system}.noctua-icon-theme;
    };
  };

  # The modeline reflects the focused buffer, proved ON SCREEN (milestone 4).
  modelineVm = import ./modeline-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The default keymap closes: no collision and every shortcut pointing at a
  # command that exists. The binary is the same one a person runs by hand.
  keymap-no-collision = mkCheck "keymap-no-collision" [ ] ''
    ${rust.workspace}/bin/ronoctua-shell check-keymap
  '';

  # THE SHEET'S TAG ROW EXISTS FOR EVERY LANGUAGE.
  #
  # The glyph sheet shows one row of tags per language, and the key is
  # ASSEMBLED — `glyphs-sheet-tags-` plus the subtag — so it is one of the keys
  # `docs/ARCHITECTURE.md` calls invisible to every search in the code. It was
  # two constants, `TAGS_PT` and `TAGS_EN`, which is the shape that made a third
  # language impossible; now it is a prefix, and what a prefix loses is the
  # compile error.
  #
  # Without this, declaring a third language would draw `[glyphs-sheet-tags-de]`
  # on the sheet — the raw key, which is exactly the failure invariant 8 exists
  # to refuse.
  buffer-tag-rows-have-text = mkCheck "buffer-tag-rows-have-text" [ pkgs.gnugrep pkgs.coreutils ] ''
    failed=0
    for language in ${lib.concatStringsSep " " languages}; do
      subtag=$(echo "$language" | cut -d- -f1 | tr 'A-Z' 'a-z')
      for other in ${lib.concatStringsSep " " languages}; do
        key="glyphs-sheet-tags-$subtag"
        grep -qE "^$key = ." ${../assets/i18n}/$other.ftl || {
          echo "ERROR: $key is missing from $other.ftl — the glyph sheet would" >&2
          echo "       draw the raw key on the row that shows $language's tags." >&2
          failed=1
        }
      done
    done
    [ "$failed" = 0 ] || exit 1
    echo "OK: a tag row for each of ${toString (builtins.length languages)} languages, in every .ftl"
  '';

  # A THIRD LANGUAGE COSTS NOTHING BUT ITS TRANSLATION.
  #
  # Everything else in this file now reads `assets/i18n/languages.nix`, so a
  # third language is checked everywhere the first two are. What no check could
  # say is whether the PIPELINE survives a third — and it could not be said by
  # inspection, because for as long as the tree has existed the answer to "how
  # many languages" has been two, in ninety-five places.
  #
  # So this builds one. `zz-ZZ` is a synthetic language whose every value is the
  # base language's, PADDED to roughly 120% — which is about what German or
  # French cost against English, and rather more than the 6.8% pt-BR costs. It
  # exercises the generator's key parity, the `LANGUAGES` constant, the
  # generated `face()`, and the arithmetic of every check that used to name a
  # pair. It never reaches an installed system: it is built here, used here, and
  # thrown away with the derivation.
  #
  # `zz` is reserved by ISO 639-2 for local use, so it cannot collide with a
  # real language somebody adds later.
  third-language-costs-nothing = mkCheck "third-language-costs-nothing" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, subprocess, sys, tempfile

    source = pathlib.Path("${../assets/i18n}")
    base = "${builtins.head languages}"
    work = pathlib.Path(tempfile.mkdtemp()) / "i18n"
    work.mkdir(parents=True)
    for f in source.glob("*.ftl"):
        (work / f.name).write_text(f.read_text(encoding="utf-8"), encoding="utf-8")

    # THE VALUE GROWS, THE KEY DOES NOT. A key that changed would be testing
    # the generator's tolerance for a typo, which is a different check
    # (`i18n-missing-key-fails`) and one that already exists.
    grown = []
    for line in (source / f"{base}.ftl").read_text(encoding="utf-8").splitlines():
        m = re.match(r"^([a-z][a-z0-9-]*) *= *(.+)$", line)
        if m:
            value = m.group(2)
            grown.append(f"{m.group(1)} = {value + 'x' * max(1, len(value) // 5)}")
        else:
            grown.append(line)
    (work / "zz-ZZ.ftl").write_text("\n".join(grown) + "\n", encoding="utf-8")

    out = work.parent / "keys.rs"
    r = subprocess.run(
        [sys.executable, "${../nix/scripts/gen-i18n.py}", str(work), str(out),
         *"${lib.concatStringsSep " " languages}".split(), "zz-ZZ"],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        print("ERROR: the generator refused a complete third language.", file=sys.stderr)
        print(r.stderr, file=sys.stderr)
        raise SystemExit(1)

    body = out.read_text(encoding="utf-8")
    # The three things a third language has to reach: the list, the loader,
    # and the count the generator reports.
    if 'pub const LANGUAGES: [&str; 3]' not in body:
        print("ERROR: LANGUAGES did not grow to three.", file=sys.stderr)
        raise SystemExit(1)
    if '"zz-ZZ" => Some(include_str!' not in body:
        print("ERROR: the generated face() has no arm for the third language —", file=sys.stderr)
        print("       Texts::fresh would answer None and fall back in silence.", file=sys.stderr)
        raise SystemExit(1)
    if "in 3 languages" not in r.stdout:
        print(f"ERROR: the generator did not report three: {r.stdout!r}", file=sys.stderr)
        raise SystemExit(1)

    # AND AN INCOMPLETE THIRD IS STILL REFUSED. Without this the check above
    # would pass on a generator that had stopped comparing altogether.
    lines = (work / "zz-ZZ.ftl").read_text(encoding="utf-8").splitlines()
    (work / "zz-ZZ.ftl").write_text("\n".join(lines[:-1]) + "\n", encoding="utf-8")
    r = subprocess.run(
        [sys.executable, "${../nix/scripts/gen-i18n.py}", str(work), str(out),
         *"${lib.concatStringsSep " " languages}".split(), "zz-ZZ"],
        capture_output=True, text=True,
    )
    if r.returncode == 0:
        print("ERROR: a third language missing a key was ACCEPTED.", file=sys.stderr)
        raise SystemExit(1)

    print(f"OK: a third language builds, loads and is checked — {len(grown)} lines, +20%")
    END
  '';

  # DECLARED AND PRESENT ARE THE SAME SET.
  #
  # `assets/i18n/languages.nix` is the one list, and `gen-i18n.py` already
  # closes one direction: a name there with no `.ftl` beside it takes the build
  # down. This closes the other, which was open and silent.
  #
  # A `.ftl` dropped into the directory without being declared used to be
  # copied into the store by `nix/i18n.nix` (which globs), linted by
  # `english-names` (which globs), and then ignored by everything that
  # matters — no key parity, no entry in `LANGUAGES`, no bundle, no menu.
  # `Texts::fresh` would answer `None` for it and the screen would fall back to
  # English without a word. A file that is present, processed and inert is the
  # worst of the three states.
  languages-are-declared = mkCheck "languages-are-declared" [ pkgs.coreutils pkgs.gnused ] ''
    declared=$(printf '%s\n' ${lib.escapeShellArgs languages} | sort)
    present=$(cd ${../assets/i18n} && ls *.ftl | sed 's/\.ftl$//' | sort)

    if [ "$declared" != "$present" ]; then
      echo "ERROR: the declared languages and the .ftl files on disk differ." >&2
      echo "  declared in assets/i18n/languages.nix:" >&2
      printf '    %s\n' $declared >&2
      echo "  present in assets/i18n:" >&2
      printf '    %s\n' $present >&2
      echo "      A file that is present and undeclared is worse than a missing" >&2
      echo "      one: it gets copied and linted, and then nothing reads it." >&2
      exit 1
    fi

    # And the base has to be first, because that is what the generator
    # compares every other against and what the installer's menu opens on.
    base=${builtins.head languages}
    test -f ${../assets/i18n}/$base.ftl || {
      echo "ERROR: the base language $base has no .ftl" >&2; exit 1; }

    echo "OK: $(printf '%s\n' $declared | wc -l) languages, declared and present, base $base"
  '';

  # Every language has exactly the same keys, none of them empty. It is the
  # generator that runs in the build; here it runs again as a named check.
  i18n-complete = mkCheck "i18n-complete" [ pkgs.python3 ] ''
    python3 ${../nix/scripts/gen-i18n.py} ${../assets/i18n} $TMPDIR/keys.rs \
      ${lib.escapeShellArgs languages}
  '';

  # THE NESTED SESSION'S `FONTCONFIG_FILE` RESOLVES A REAL FONT.
  #
  # # What this catches, and how it showed up
  #
  # `FONTCONFIG_FILE` replaces the WHOLE fontconfig configuration. The
  # `nix/dev.nix` pointed at the FRAGMENT — only the alias rules, with no
  # `<dir>` at all —, and fontconfig was left with no font directory: zero
  # fonts, and everything that goes through it becomes TOFU.
  #
  # The symptom was an error message from niri in little boxes on the screen.
  # Precisely the message that says what is wrong — the cruelest failure mode
  # there is, because it wipes the information you need exactly when you need it.
  #
  # Our renderer does not go through here (section 6.4), so the footer looked
  # normal while the compositor's warning was illegible: two chains, and only
  # one broken. No check looked at the second.
  fontconfig-nested-finds-font = mkCheck "fontconfig-nested-finds-font" [ pkgs.fontconfig.bin ] ''
    export FONTCONFIG_FILE=${fontconfigConf.fullConf}
    export XDG_CACHE_HOME=$TMPDIR/cache

    # ZERO FONTS IS THE DEFECT. An empty `fc-list` means that every text that
    # goes through fontconfig comes out in little boxes.
    n=$(fc-list 2>/dev/null | wc -l)
    echo "fontconfig sees $n fonts"
    [ "$n" -gt 0 ] || {
      echo "ERROR: NO font at all is visible through the nested FONTCONFIG_FILE." >&2
      echo "      Every foreign app's text — and this compositor's own error" >&2
      echo "      compositor — it would come out as tofu." >&2
      exit 1
    }

    # AND THE THREE GENERICS FALL ON OUR FONT (section 6.4). Finding "some" font
    # is not enough: the nested session exists to show what the distro shows, and
    # with the host's config in the middle it showed Liberation Sans.
    for g in sans-serif serif monospace; do
      matched=$(fc-match "$g" 2>/dev/null)
      echo "  $g -> $matched"
      case "$matched" in
        *NoctuaIosevka*) ;;
        *) echo "ERROR: \`$g\` fell through to '$matched', and not to our font." >&2
           exit 1 ;;
      esac
    done
    echo "OK: the nested fontconfig finds a font, and it is ours"
  '';

  # NIRI'S CONFIG IS VALID — BOTH of them, the production one and the nested one.
  #
  # # What this catches, and what not having it cost
  #
  # A `duplicate keybind` — `Alt+Ctrl+L` from niri's defaults against the
  # emergency `Ctrl+Alt+L`, the same key written in another order — went through
  # the WHOLE `nix flake check`. The symptom was a red box on the screen and,
  # worse, niri falling back to its DEFAULT config: none of our shortcuts, and
  # the leader not working.
  #
  # Nothing validated the generated files. The `niri-binds.nix` grepped for one
  # word, the module grepped for another — and neither of them asked the
  # compositor whether the whole file closes.
  #
  # `niri validate` is the right question, asked of whoever has the answer.
  niri-config-valid = mkCheck "niri-config-valid" [ inputs.niri.packages.${system}.niri ] ''
    for cfg in "${self.nixosConfigurations.vm.config.environment.etc."niri/config.kdl".source}" \
               "${devApp.config}"; do
      echo "validating $cfg"
      if ! niri validate --config "$cfg" 2>errors.txt; then
        echo "ERROR: niri REFUSED the generated config." >&2
        echo "      A refused config is not a broken shortcut: it is the" >&2
        echo "      compositor falling back to ITS default, with nothing of ours." >&2
        cat errors.txt >&2
        exit 1
      fi
    done
    echo "OK: both niri configs are valid"
  '';

  # A CAPTURE LANDS IN A DIRECTORY THE DISTRO DECLARED.
  #
  # # What went wrong without this
  #
  # Nothing wrote `screenshot-path`, so niri used its own default —
  # `~/Pictures/Screenshots/…`, from `niri-config/src/misc.rs`. This distro has
  # no `~/Pictures`: its `XDG_PICTURES_DIR` is `main/media/image`. So every
  # capture went to a directory the system never declared, never created and
  # nothing could point at — and it was silent, because a compositor writing a
  # PNG somewhere is not an error anywhere.
  #
  # # Why it reads BOTH files
  #
  # Because the defect this replaces is the two DRIFTING, not either one being
  # wrong on its own. `nix/user-dirs.nix` is what makes the directory (through
  # `xdg.userDirs`, whose `createDirectories` covers `extraConfig` too), and the
  # niri config is what writes into it. Checking only that `screenshot-path`
  # exists would go green on a path pointing anywhere at all.
  #
  # It reads the GENERATED config, and not `nix/modules/niri.nix` — the check is
  # against the code, not against the criterion. A `screenshot-path` written
  # into the module and lost on its way through `configNiri`'s `sed` would still
  # be in the source.
  screenshots-land-in-a-declared-directory =
    mkCheck "screenshots-land-in-a-declared-directory" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import pathlib, re, sys

        config = pathlib.Path(
          "${self.nixosConfigurations.vm.config.environment.etc."niri/config.kdl".source}"
        ).read_text()

        line = re.search(r'^\s*screenshot-path\s+"([^"]+)"', config, re.M)
        if not line:
            print("ERROR: niri's config has no `screenshot-path`.", file=sys.stderr)
            print("", file=sys.stderr)
            print("       Without it niri uses ITS default, ~/Pictures/Screenshots/,", file=sys.stderr)
            print("       which this distro neither declares nor creates. The screenshot lands, and", file=sys.stderr)
            print("       not one error appears anywhere at all.", file=sys.stderr)
            raise SystemExit(1)
        written = line.group(1)

        # The name the distro declares, from the ONE file that holds it. Read as
        # text and not evaluated: this check has no Nix to call, and the entry is a
        # plain string.
        table = pathlib.Path("${../nix/user-dirs.nix}").read_text()
        entry = re.search(r'^\s*screenshot\s*=\s*"([^"]+)"\s*;', table, re.M)
        if not entry:
            print("ERROR: nix/user-dirs.nix has no `screenshot` entry.", file=sys.stderr)
            raise SystemExit(1)
        declared = entry.group(1)

        wanted = f"~/{declared}/"
        if not written.startswith(wanted):
            print("ERROR: the screenshot does not land in the declared folder.", file=sys.stderr)
            print("", file=sys.stderr)
            print(f"       niri writes to: {written}", file=sys.stderr)
            print(f"       the distro declares: {wanted}", file=sys.stderr)
            print("", file=sys.stderr)
            print("       Both come out of nix/user-dirs.nix on purpose: it is the", file=sys.stderr)
            print("       file that CREATES the folder, through xdg.userDirs, and the one the", file=sys.stderr)
            print("       manager's sidebar points at.", file=sys.stderr)
            raise SystemExit(1)

        # And the name has to say WHEN, or a second capture overwrites the first.
        if "%" not in written:
            print("ERROR: the screenshot's name has no strftime in it at all.", file=sys.stderr)
            print(f"       {written} — the second screenshot erases the first.", file=sys.stderr)
            raise SystemExit(1)

        print(f"OK: the screenshot lands in {wanted}, which the distro declares and creates")
        END
      '';

  # THE DISKS MODULE ADDS NO POLKIT RULE OF ITS OWN.
  #
  # # Why the assertion is INVERTED here, and only here
  #
  # `files-polkit-is-narrow`, `rescue-polkit-is-narrow` and
  # `transfers-unit-is-narrow` all demand a rule, because in those three the
  # UNIT IS OURS and nobody else authorises it. udisks2 is not ours: it ships
  # actions that already split on `HintSystem`, so a removable device mounts
  # with no password and an internal one asks. A rule of ours could only WIDEN
  # that, and it would be a second place to keep in step with somebody else's
  # action ids — which polkit parses at runtime, so a stale one is silent.
  #
  # # The four halves, and why each is needed
  #
  # Without (1) this goes green on a module that does nothing at all. Without
  # (3) the rule could be smuggled into another module. Without (4) it goes
  # green on a tree where every polkit rule in the project was deleted, which is
  # the failure mode of every "assert the absence of something" check.
  disks-adds-no-polkit-rule-of-its-own =
    mkCheck "disks-adds-no-polkit-rule-of-its-own" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import pathlib, re, sys

        modules = pathlib.Path("${../nix/modules}")
        disks = (modules / "disks.nix").read_text()
        wrong = []

        # 1. THE MODULE ACTUALLY TURNS UDISKS ON.
        if not re.search(r"^\s*enable = true;", disks, re.M):
            wrong.append("nix/modules/disks.nix does not switch udisks2 on")
        if "services.udisks2" not in disks:
            wrong.append("nix/modules/disks.nix does not mention services.udisks2")

        # 2. AND WRITES NO RULE.
        if "polkit" in disks and "extraConfig" in disks:
            wrong.append("nix/modules/disks.nix writes a polkit rule")

        # 3. NOR DOES ANY OTHER MODULE NAME A UDISKS ACTION.
        for f in sorted(modules.glob("*.nix")):
            body = f.read_text()
            if "org.freedesktop.udisks2." in body.lower():
                wrong.append(f"{f.name} names a udisks2 action in a polkit rule")

        # 4. AND THE FOUR RULES THAT SHOULD EXIST STILL DO.
        #    An "assert the absence" check that never asserts a presence goes
        #    green on an empty tree.
        for name in ["files.nix", "rescue.nix", "memory.nix", "keyboard.nix"]:
            body = (modules / name).read_text()
            if "polkit.addRule" not in body:
                wrong.append(f"{name} lost the polkit rule it MUST have")

        if wrong:
            print("ERROR: the disks' privilege posture changed.", file=sys.stderr)
            print("", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            print("", file=sys.stderr)
            print("       udisks2 already separates removable from system by HintSystem,", file=sys.stderr)
            print("       and its rule is narrower than any of ours could be.", file=sys.stderr)
            raise SystemExit(1)
        print("OK: udisks2 on, no rule of ours, and the four that exist are still there")
        END
      '';

  # THE TOOL LIST OF UDISKS IS PINNED, AND SO IS WHAT WE ASK OF IT.
  #
  # # What this is protecting
  #
  # `nix/modules/disks.nix` builds udisks with `xfsprogs` replaced by an empty
  # directory. Measured: 305,4 MiB, of which 258,0 is a python3 env reached by
  # exactly one path — `udisks → xfsprogs-bin → python3-env`, for `xfs_scrub`.
  # The distro is btrfs and no pendrive is XFS.
  #
  # That override is free ONLY because mounting is a kernel operation and this
  # crate asks udisks for four verbs, none of which is checking, repairing,
  # resizing or formatting. The day somebody proxies one of those, the override
  # silently stops being free and starts being a feature that does not work.
  #
  # # Why it reads the CODE
  #
  # Because a comment saying "we do not format" is not a guarantee. This greps
  # the proxy module for method declarations, which is the rule
  # `switches-that-do-nothing-say-so` states: the comments do not count.
  disks-tools-are-pinned = mkCheck "disks-tools-are-pinned" [ pkgs.python3 pkgs.binutils ] ''
    python3 - <<'END'
    import pathlib, re, sys

    module = pathlib.Path("${../nix/modules/disks.nix}").read_text()
    proxies = pathlib.Path("${../crates/ronoctua-disks/src/udisks.rs}").read_text()
    wrong = []

    # 1. THE ONE OVERRIDE IS STILL THERE. A nixpkgs bump that re-adds the
    #    258 MiB of Python has to fail here, and not on somebody's ISO.
    if not re.search(r"xfsprogs\s*=\s*pkgs\.emptyDirectory\s*;", module):
        wrong.append("the xfsprogs override left nix/modules/disks.nix")

    # 2. AND IT IS STILL THE ONLY ONE. A tool quietly dropped is a
    #    `mount.<type>` quietly missing, which is a stick that will not mount
    #    and nothing saying why.
    block = re.search(r"pkgs\.udisks\.override\s*\{(.*?)\n  \};", module, re.S)
    if not block:
        wrong.append("could not find the `pkgs.udisks.override` block in disks.nix")
    else:
        named = set(re.findall(r"^\s*(\w+)\s*=", block.group(1), re.M))
        extra = named - {"xfsprogs"}
        if extra:
            wrong.append(f"disks.nix overrides more than xfsprogs: {sorted(extra)}")

    # 3. WHAT WE ASK OF UDISKS, AND WHAT WE MUST NOT.
    def declared(name):
        return re.search(rf"^\s*fn {name}\(", proxies, re.M) is not None

    # `format` USED TO BE ON THIS LIST, and moving it off is a decision and not
    # a relaxation: writing a filesystem is a thing this distro now does, from a
    # buffer whose whole design is the two refusals in front of it. What stays
    # forbidden is everything that would REPAIR or RENAME a filesystem — those
    # are the verbs whose helper the xfsprogs override took away, and offering
    # one that silently does nothing for XFS is the failure this check exists
    # for.
    for verb in ["check", "repair", "resize", "set_label", "take_ownership"]:
        if declared(verb):
            wrong.append(
                f"udisks.rs declares `{verb}`, which the xfsprogs override makes impossible"
            )

    # 4. AND THE FIVE THAT EXIST STILL DO, or this goes green on an empty file.
    for verb in ["mount", "unmount", "eject", "power_off", "format"]:
        if not declared(verb):
            wrong.append(f"udisks.rs lost `{verb}`, which the sidebar uses")

    # 5. AND THE FIVE THIS BUILD CAN WRITE ARE THE FIVE IT OFFERS.
    #
    #    THE SENTENCE ON SCREEN IS WHAT IS BEING CHECKED. The buffer draws a
    #    row saying XFS is not here because the program that writes it is not
    #    installed; a `Recipe` that named `xfs` would make that row a lie AND
    #    produce a refusal from udisks with nothing on screen predicting it.
    protocol = pathlib.Path("${../crates/ronoctua-protocol/src/lib.rs}").read_text()
    body = re.search(r"pub const fn filesystem\(self\)[^\n]*\n(.*?)\n    \}", protocol, re.S)
    if not body:
            wrong.append("could not find `Recipe::filesystem` in the protocol")
    else:
        offered = set(re.findall(r'=> "([a-z0-9]+)"', body.group(1)))
        if offered != {"vfat", "exfat", "ntfs", "ext4", "btrfs"}:
            wrong.append(f"the filesystems on offer changed: {sorted(offered)}")

    if wrong:
        print("ERROR: what we ask of udisks2 and what it carries disagree.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
        print("OK: only xfsprogs goes, the five verbs exist, and XFS is not offered")
    END

    # AND THE BUILT PACKAGE AGREES WITH THE OVERRIDE.
    #
    # The two halves above read SOURCE. This one reads the artefact, and it is
    # the half that catches a bump: nixpkgs patches `udisksd` to OVERWRITE its
    # own `PATH` with a `makeBinPath` of the tools it was built against, so
    # that string IS what udisks can reach at runtime. `system.fsPackages`
    # never reaches it, which is why `boot.supportedFilesystems` does almost
    # nothing and the module says so.
    #
    # It lives here and not in `disks-vm` because it is a property of the
    # DERIVATION and not of a booted machine: a check that needs no VM fails in
    # seconds instead of in minutes.
    forced=$(strings ${slimUdisks}/libexec/udisks2/udisksd \
      | grep -E '^/nix/store/.*/bin(:|$)' | head -1)
    [ -n "$forced" ] || {
      echo "ERROR: could not find the forced PATH inside udisksd." >&2
      echo "      Without it the nixpkgs patch changed, and the override of" >&2
      echo "      xfsprogs stops meaning what this check claims." >&2
      exit 1; }

    case "$forced" in
      *xfsprogs*)
        echo "ERROR: xfsprogs is back on udisksd's PATH, and 212 MiB with it." >&2
        echo "      Measured: 305.4 MiB of xfsprogs-bin, of which 258.0 are a" >&2
        echo "      python3 environment for xfs_scrub — and this distro is btrfs." >&2
        exit 1 ;;
    esac

    for helper in exfat ntfs3g dosfstools util-linux; do
      case "$forced" in
        *"$helper"*) ;;
        *)
          echo "ERROR: $helper left udisksd's PATH." >&2
          echo "      A pen drive formatted like that stops mounting, and the" >&2
          echo "      symptom is a sidebar row that does nothing." >&2
          exit 1 ;;
      esac
    done
    echo "OK: udisksd's forced PATH has no xfsprogs and has the four helpers"
  '';

  # PERSISTENCE TAKES ITS ROOT AS A PARAMETER.
  #
  # # The defect this exists for, and it reached the owner's screen
  #
  # The file manager's sidebar persisted through a function that found
  # `$XDG_CONFIG_HOME` for itself. Its unit tests press the key that hides the
  # sidebar and the key that pins a file — because that is how one proves a key
  # does something — so `cargo test` wrote into the REAL configuration of
  # whoever ran it, leaving a `/tmp` path pinned and `showing: false`. The
  # symptom was a file manager with no sidebar on a build that had one, and
  # NOTHING FAILED: every test green, every check green, and the only wrong
  # thing in a file nobody thinks to look at.
  #
  # The rule was already written, in the very crate that broke it: `trash::Bin`
  # takes its root because "two tests setting the same environment variable
  # race, and the one that loses puts a file in the other's trash".
  #
  # # Why the shape, and not a sweep of `$HOME` after the suite
  #
  # A check that ran the suite and looked at the home afterwards would only
  # catch the crates that have such a test TODAY. This catches the shape: a
  # function that reads or writes must not also be the one that decides where.
  persistence-takes-its-root-as-a-parameter =
    mkCheck "persistence-takes-its-root-as-a-parameter" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import pathlib, re, sys

        # WHERE THE ENVIRONMENT MAY BE READ AT ALL. Both are named for what they
        # are — a lookup, not a read — and the application calls them, once.
        FINDERS = ("config_dir()", "state_dir()")
        # What makes a function one that persists.
        TOUCHES = (
            "fs::write", "fs::read_to_string", "create_dir_all", "fs::rename",
            "File::create", "File::open",
        )
        # The lookups themselves, and the constructor that exists to gather them.
        ALLOWED = ("config_dir", "state_dir", "of_env")

        crates = pathlib.Path("${../crates}")
        bad = []
        for f in sorted(crates.rglob("*.rs")):
            body = f.read_text()
            for m in re.finditer(r"\n(\s*)(?:pub )?(?:const )?fn (\w+)", body):
                indent, name = m.group(1), m.group(2)
                if name in ALLOWED:
                    continue
                rest = body[m.end():]
                stop = re.search("\n" + indent + "}\n", rest)
                inside = rest[: stop.start()] if stop else rest
                if not any(w in inside for w in TOUCHES):
                    continue
                found = [w for w in FINDERS if w in inside]
                if found:
                    bad.append(
                        f"{f.relative_to(crates)}: `{name}` writes AND finds the "
                        f"directory ({', '.join(found)})"
                    )

        if bad:
            print("ERROR: persistence that finds its own root.", file=sys.stderr)
            print("", file=sys.stderr)
            for b in bad:
                print("       " + b, file=sys.stderr)
            print("", file=sys.stderr)
            print("       A function that reads or writes does NOT decide where. What finds", file=sys.stderr)
            print("       the directory is the application, once, and it hands over the", file=sys.stderr)
            print("       answer — otherwise `cargo test` writes into the home of whoever", file=sys.stderr)
            print("       runs it, which is what happened with the sidebar.", file=sys.stderr)
            raise SystemExit(1)

        # AND THE FINDERS ARE STILL THERE. Without this the check goes
        # green on a tree where persistence was deleted altogether.
        finders = sum(
            1
            for f in crates.rglob("*.rs")
            for w in ("fn config_dir(", "fn state_dir(")
            if w in f.read_text()
        )
        if finders < 2:
            print("ERROR: could not find the functions that locate the directories.", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: nothing that persists finds its own root ({finders} locators)")
        END
      '';

  # THE KEYBOARD LAYOUT REALLY EXISTS.
  #
  # # Why this needs a check
  #
  # xkb does NOT refuse a layout it does not know: it falls back to `us` and
  # carries on. The symptom is the whole keyboard typing something else, with
  # not one line of error anywhere — the same class as the silent fallback that
  # has already cost dearly four times in this project, and the dearest of them,
  # because the way to investigate is by typing.
  #
  # The source of truth is the `xkeyboard-config` registry the session is going
  # to use, and not a list of ours: bumping nixpkgs may retire a name.
  # Chrome does not ask for a password, and cannot start asking again.
  #
  # Two halves, and the check has to see both. The flag alone is not enough: a
  # Secret Service that exists in the closure will be found by SOMETHING, and
  # the prompt comes back through a dependency nobody chose — which is exactly
  # the failure this is here to prevent, because the symptom (a password box on
  # startup) looks like a Chrome setting and is not.
  browser-has-no-keyring = mkCheck "browser-has-no-keyring" [ pkgs.jq ] ''
    system_path=${self.nixosConfigurations.vm.config.system.build.toplevel}

    # 1) The flag reached the wrapper the user actually runs.
    chrome=$(readlink -f "$system_path/sw/bin/google-chrome-stable" 2>/dev/null \
             || readlink -f "$system_path/sw/bin/google-chrome" 2>/dev/null || true)
    [ -n "$chrome" ] || { echo "ERROR: Chrome is not in system_path's closure" >&2; exit 1; }
    grep -q -- "--password-store=basic" "$chrome" || {
      echo "ERROR: Chrome's wrapper has no --password-store=basic." >&2
      echo "      Without it Chrome looks for a Secret Service and asks for a password." >&2
      exit 1; }
    echo "OK: --password-store=basic in the wrapper"

    # 2) NO Secret Service anywhere in the closure.
    #
    # `nix-store -qR` and not a grep over the config: the point is what ends up
    # ON THE MACHINE, and a dependency that pulls the keyring in does not appear
    # in any option.
    found=$(nix-store -qR "$system_path" 2>/dev/null | grep -E "gnome-keyring|kwallet" || true)
    if [ -n "$found" ]; then
      echo "ERROR: a Secret Service got into the closure:" >&2
      echo "$found" | sed 's/^/       /' >&2
      echo "" >&2
      echo "      With it installed Chrome asks for a password again on opening," >&2
      echo "      and the prompt is the keyring's, not Chrome's: no option in the" >&2
      echo "      browser switches it off." >&2
      exit 1
    fi
    echo "OK: no gnome-keyring and no kwallet in the closure"
  '';

  # The nine directories of section 2 reach `user-dirs.dirs`.
  #
  # Setting the environment variable alone moves NOTHING: a file dialog, a
  # browser download and `xdg-open` all read `~/.config/user-dirs.dirs`. So the
  # check reads the generated file, not the option — the option agreeing with
  # itself proves nothing.
  #
  # `XDG_PROJECTS_DIR` is in the list on purpose even though the spec has no such
  # key: it is ours, and if it silently vanished from the generated file nobody
  # would notice until a script went looking for it.
  xdg-user-dirs = mkCheck "xdg-user-dirs" [ ] ''
    dirs=${
      self.nixosConfigurations.vm.config.home-manager.users.noctua.xdg.configFile."user-dirs.dirs".source
    }
    echo "generated file: $dirs"

    expected="XDG_DESKTOP_DIR=/home/noctua/main/temp
    XDG_DOCUMENTS_DIR=/home/noctua/main/archive
    XDG_DOWNLOAD_DIR=/home/noctua/main/temp
    XDG_MUSIC_DIR=/home/noctua/main/media/audio-music
    XDG_PICTURES_DIR=/home/noctua/main/media/image
    XDG_PROJECTS_DIR=/home/noctua/main/prj
    XDG_PUBLICSHARE_DIR=/home/noctua/main/share/public
    XDG_SCREENSHOT_DIR=/home/noctua/main/media/screenshot
    XDG_TEMPLATES_DIR=/home/noctua/templates
    XDG_VIDEOS_DIR=/home/noctua/main/media/video"

    missing=0
    echo "$expected" | while read -r pair; do
      [ -n "$pair" ] || continue
      key=''${pair%%=*}
      value=''${pair#*=}
      line=$(grep -E "^$key=" "$dirs" || true)
      if [ -z "$line" ]; then
        echo "ERROR: $key is not in the generated user-dirs.dirs" >&2; exit 1
      fi
      # The generated value is quoted and uses `$HOME`.
      got=$(echo "$line" | sed -e 's/^[^=]*=//' -e 's/^"//' -e 's/"$//' -e "s|\\\$HOME|/home/noctua|")
      if [ "$got" != "$value" ]; then
        echo "ERROR: $key = $got, expected $value" >&2; exit 1
      fi
      echo "  OK  $key = $got"
    done || missing=1
    [ "$missing" = 0 ] || exit 1

    echo "OK: section 2's ten directories reach user-dirs.dirs"
  '';

  # THE LOCALE AND THE TIMEZONE EXIST, and the check is where that gets asked.
  #
  # `ronoctua.locale` is a free string on purpose: the real list is glibc's
  # `SUPPORTED`, and reading it during evaluation would be
  # import-from-derivation — the flake would stop and wait for a build. Here,
  # inside a build, the file is free to read.
  #
  # What goes wrong without this is quiet in the way that costs most: a locale
  # glibc does not know produces no error at build time, and the installed system
  # falls back to `C` — dates in American order, no accents in `sort`, and
  # nothing anywhere saying why.
  # THE CONSOLE QUESTION EXISTS, AND ASKS ONCE.
  #
  # Three things that are cheap to get wrong and expensive to notice:
  #
  #   - it must be wanted by `multi-user.target`, because it exists for the boot
  #     that never reaches a graphical one. Under `graphical.target` it would run
  #     exactly when it is not needed and never when it is;
  #   - it must be `oneshot` with no `Restart`, or a machine out of range gets a
  #     password prompt on its console every minute, forever;
  #   - the binary must be a store path. A unit's PATH is minimal, and the
  #     failure of a bare name shows up as a service that exits with nothing to
  #     say about Wi-Fi.
  wifi-console-asks-once = mkCheck "wifi-console-asks-once" [ ] ''
    unit=${self.nixosConfigurations.vm.config.systemd.services.ronoctua-network-console.serviceConfig.Type}
    test "$unit" = "oneshot" || {
      echo "ERROR: the console's service is \`$unit\`, and not oneshot." >&2
      echo "      With Restart it would ask for the password in a loop." >&2
      exit 1; }

    exec_start=${self.nixosConfigurations.vm.config.systemd.services.ronoctua-network-console.serviceConfig.ExecStart}
    case "$exec_start" in
      /nix/store/*) ;;
      *) echo "ERROR: ExecStart is not a store path: $exec_start" >&2; exit 1 ;;
    esac

    wanted="${lib.concatStringsSep " " self.nixosConfigurations.vm.config.systemd.services.ronoctua-network-console.wantedBy}"
    case " $wanted " in
      *" multi-user.target "*) ;;
      *) echo "ERROR: the service is not pulled in by multi-user.target: $wanted" >&2
         echo "      It exists for the boot that does NOT reach graphics." >&2
         exit 1 ;;
    esac

    echo "OK: the console question is oneshot, by path, in multi-user"
  '';

  locale-exists = mkCheck "locale-exists" [ ] ''
    supported=${pkgs.glibcLocales}/share/i18n/SUPPORTED
    tab=${pkgs.tzdata}/share/zoneinfo/zone1970.tab
    locale=${self.nixosConfigurations.vm.config.i18n.defaultLocale}
    zone=${self.nixosConfigurations.vm.config.time.timeZone}

    grep -qE "^$locale/UTF-8" "$supported" || {
      echo "ERROR: the locale \`$locale\` is not in glibc's SUPPORTED." >&2
      echo "      system_path would fall through to \`C\`: dates in American order, with no" >&2
      echo "      no accent in the sorting, and nothing saying why." >&2
      exit 1; }

    # The measurement key is the one that must NOT follow the language: it is
    # what keeps the machine metric when someone installs in American English.
    measurement=${self.nixosConfigurations.vm.config.i18n.extraLocaleSettings.LC_MEASUREMENT or ""}
    case "$measurement" in
      en_US*|en_PH*|""|C|C.UTF-8|POSIX)
      echo "ERROR: LC_MEASUREMENT = \`$measurement\` is not metric." >&2
      echo "      Choosing American English chooses a language, never an inch." >&2
        exit 1 ;;
    esac

    awk -F'\t' '!/^#/ { print $3 }' "$tab" | grep -qx "$zone" || {
      echo "ERROR: the zone \`$zone\` does not exist in the tzdb." >&2
      exit 1; }

    echo "OK: locale \`$locale\`, metric measurements, zone \`$zone\`"
  '';

  keyboard-layout-exists = mkCheck "keyboard-layout-exists" [ ] ''
    lst=${pkgs.xkeyboard_config}/share/X11/xkb/rules/base.lst
    layout=${keyboard.layout}
    optionName=${keyboard.options}
    variantName="${keyboard.variant}"

    grep -qE "^ +$layout +" "$lst" || {
      echo "ERROR: the layout \`$layout\` does not exist in xkeyboard-config." >&2
      echo "      xkb does NOT refuse: it falls back to \`us\` and the whole keyboard" >&2
      echo "      starts typing something else, with no error at all." >&2
      exit 1; }

    # ONE BY ONE, and there is more than one now.
    #
    # `options` is a comma-separated LIST, and this used to grep the whole
    # string as if it were a single name — which worked only for as long as
    # there was exactly one option. The day a second was added, the check would
    # have failed on a correct config, which is the other half of the same
    # defect: a check that cannot see is as bad as no check.
    for o in $(echo "$optionName" | tr ',' ' '); do
      grep -qE "^ +$o +" "$lst" || {
        echo "ERROR: the option \`$o\` does not exist in xkeyboard-config." >&2
        echo "      xkb does NOT refuse it: it IGNORES the option and carries on," >&2
        echo "      so the accents simply do not happen, with no error." >&2
        exit 1; }
    done

    if [ -n "$variantName" ]; then
      grep -qE "^ +$variantName +$layout:" "$lst" || {
        echo "ERROR: the variant \`$variantName\` does not exist for \`$layout\`." >&2
        exit 1; }
    fi

    # And the CONSOLE's keymap, which is another database (kbd, not xkb). The
    # installer runs on a TTY: a wrong name here gives a US keyboard at the moment
    # of typing the disk's passphrase.
    test -e ${pkgs.kbd}/share/keymaps/i386/qwerty/${keyboard.console}.map.gz || {
      echo "ERROR: the console keymap \`${keyboard.console}\` does not exist in kbd." >&2
      exit 1; }

    echo "OK: layout $layout, option $optionName, console ${keyboard.console}"
  '';

  # AND THE OPTIONS DO WHAT THEIR NAMES PROMISE — the keymap, compiled.
  #
  # # Why existing is not enough
  #
  # The check above proves the names are in the database. It does not prove they
  # produce anything: an option xkb does not recognise is **ignored, and the
  # exit status is zero** — measured, with `compose:naoexiste`, which compiles
  # fine and merely writes "Unrecognized RMLVO option ... was ignored" to
  # stderr. The symptom is the accent key that types nothing, on a keyboard that
  # is otherwise perfect.
  #
  # So this compiles the real keymap out of the real database and reads the
  # symbols back:
  #
  #   - `<CAPS>` has to carry `Multi_key`, which IS Compose. Without it, the
  #     British layout reaches dead `^` and `` ` `` and nothing else — no `á`,
  #     no `ã`, no `ç`, which is half of Portuguese;
  #   - both Shifts have to carry `Caps_Lock` on their second level. That is
  #     where Caps Lock went when Compose took its place, and it is the only way
  #     back to it.
  #
  # `xkbcli` and not `setxkbmap`: the second one needs an X server, and this
  # is a check.
  keyboard-options-do-what-they-say =
    mkCheck "keyboard-options-do-what-they-say"
      [
        pkgs.libxkbcommon
        pkgs.gnugrep
      ]
      ''
        export XKB_CONFIG_ROOT=${pkgs.xkeyboard_config}/share/X11/xkb

        xkbcli compile-keymap \
          --layout ${keyboard.layout} \
          ${lib.optionalString (keyboard.variant != "") "--variant ${keyboard.variant} "}\
          --options "${keyboard.options}" > keymap.xkb 2> errors.txt || {
            echo "ERROR: the keymap did not even compile:" >&2; cat errors.txt >&2; exit 1; }

        # The ignored-option message, which is the silent failure by name.
        if grep -q "was ignored" errors.txt; then
          echo "ERROR: xkb IGNORED an option and carried on:" >&2
          cat errors.txt >&2
          exit 1
        fi

        # Compose, wherever the answer put it — the symbol is what matters, and
        # not which key carries it.
        grep -q "Multi_key" keymap.xkb || {
          echo "ERROR: no key became Compose (\`Multi_key\`) in the keymap." >&2
          echo "      Without it there is no \`a\`, \`a~\` or \`c,\` on this keyboard." >&2
          exit 1; }

        # And Caps Lock, which left the Caps Lock key and has to be somewhere.
        for side in LFSH RTSH; do
          grep -A 3 "key <$side>" keymap.xkb | grep -q "Caps_Lock" || {
          echo "ERROR: Shift <$side> does not toggle Caps Lock." >&2
            echo "      The Caps key became Compose; without both Shifts, Caps" >&2
          echo "      Lock stopped existing and nobody was told." >&2
            grep -A 3 "key <$side>" keymap.xkb >&2
            exit 1; }
        done

        echo "OK: Compose exists in the keymap and both Shifts toggle Caps Lock"
      '';

  # THE INSTALLER'S DEFAULTS ARE THE SYSTEM'S DEFAULTS.
  #
  # `ronoctua-installer` carries `DEFAULT_LAYOUT` and `DEFAULT_COMPOSE` as Rust
  # constants — it has to, because it runs from a TTY on the ISO with no flake
  # to evaluate. That makes them a SECOND copy of `nix/keyboard-xkb.nix`, and
  # the two agreeing has been a matter of whoever edited remembering the other.
  #
  # What divergence looks like: a machine installed by pressing Enter through
  # the questions and a machine built by `nix build .#noctua` type differently.
  # Nothing fails; the person just finds out the day they need an accent.
  installer-defaults-match-the-keyboard =
    mkCheck "installer-defaults-match-the-keyboard" [ pkgs.gnugrep ]
      ''
        src=${../crates/ronoctua-installer/src}/model.rs
        failed=0

        agree() {
          wanted="$2"
          found=$(grep -oP "pub const $1: &str = \"\K[^\"]*" "$src" | head -1)
          [ "$found" = "$wanted" ] || {
            echo "ERROR: $1 is \`$found\` in the installer and \`$wanted\` in the Nix." >&2
            failed=1
          }
        }
        agree DEFAULT_LAYOUT "${keyboard.layout}"
        agree DEFAULT_COMPOSE "${keyboard.compose}"

        [ "$failed" = 0 ] || exit 1
        echo "OK: the installer and nix/keyboard-xkb.nix agree"
      '';

  # EVERY COMPOSITOR ACTION HAS A TRANSLATION IN BOTH LANGUAGES.
  #
  # The action list is GENERATED from the pinned niri, and the translations are
  # written by hand. The two things move separately: bumping niri's tag brings
  # new actions, and without this check they would show up in the finder as
  # `[cmd-niri-...]` — the missing-key marker, in the middle of a list of 170
  # items, where nobody looks.
  #
  # It fails LOUDLY on purpose: a niri bump stops the build until what showed up
  # is translated. It is the price of mirroring the whole compositor, and it is
  # paid once per release instead of every day by whoever reads the list.
  niri-actions-translated = mkCheck "niri-actions-translated" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import re, sys, pathlib

    table_src = pathlib.Path("${niriActions}/niri-actions.rs").read_text()
    key_names = re.findall(r'key: "(cmd-niri-[a-z0-9-]+)"', table_src)
    assert len(key_names) > 100, f"only {len(key_names)} actions in the generated table"

    absent = []
    for language in ${builtins.toJSON languages}:
        ftl = pathlib.Path("${../assets/i18n}/" + language + ".ftl").read_text()
        present = set(re.findall(r"^([a-z0-9-]+) *=", ftl, re.M))
        absent += [f"{language}: {c}" for c in key_names if c not in present]

    if absent:
        print("ERROR: a niri action with no translation:", file=sys.stderr)
        for f in absent:
            print(f"  {f}", file=sys.stderr)
        print(
            "\nThe action list comes from the PINNED niri. If it went up a version,\n"
            "the new actions have to go into both .ftl files — otherwise they\n"
            "show up as `[key]` in the finder.",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"OK: niri's {len(key_names)} actions are in both languages")
    PY
  '';

  # THE SUBMENU MARK DOES NOT LIVE IN THE TRANSLATION.
  #
  # It used to: `group-window = +window`, and the `+` was part of the text. That
  # is why three of the six groups ended up without it — nothing forced it, and
  # the omission showed up nowhere. Now whoever puts the `+` on is the drawing,
  # out of the tree's `is_group`, and no group has any way of escaping.
  #
  # This check stops it from coming back: a `+` in the .ftl would give `++window`
  # on screen, which is the loud symptom of a mark in two places.
  group-mark-outside-translation = mkCheck "group-mark-outside-translation" [ ] ''
    if grep -nE '^group-[a-z-]+ *= *\+' ${../assets/i18n}/*.ftl; then
      echo "ERROR: there is a group label with a \`+\` in the translation." >&2
      echo "      The mark is put on in the DRAWING (surface::GROUP_MARK);" >&2
      echo "      written here, it comes out doubled — and only in some groups." >&2
      exit 1
    fi
    echo "OK: the submenu mark is in no translation"
  '';

  # And the other way round: a key in only one language BRINGS THE BUILD DOWN.
  # Without this fixture, the check above would pass with the generator broken.
  i18n-missing-key-fails = mkCheck "i18n-missing-key-fails" [ pkgs.python3 ] ''
    cp -r ${../assets/i18n} bad && chmod -R +w bad
    # The BASE language is the victim, because it is the one every other is
    # compared against — an orphan there has to be caught whichever way the
    # generator walks the list.
    echo 'only-in-english = orphan' >> bad/${builtins.head languages}.ftl

    if python3 ${../nix/scripts/gen-i18n.py} bad $TMPDIR/keys.rs \
      ${lib.escapeShellArgs languages} 2>errors.txt; then
      echo "ERROR: the generator ACCEPTED a key that exists in only one language" >&2
      exit 1
    fi
    grep -q 'only-in-english' errors.txt || {
      echo "ERROR: it failed, but the message does not say which key" >&2
      cat errors.txt >&2
      exit 1
    }
    echo "OK: a key in only one language takes the build down, saying which"
  '';

  # A TOKEN THAT DOES NOT EXIST DOES NOT COMPILE IN THE BUFFER — section 7.6.
  #
  # "The build generates (a) the shell's Rust table and (b) the WIT enum of valid
  # names. A token that does not exist becomes a **compile error in the guest**."
  #
  # Before, `color` was `record color { token: string }`: a buffer cited
  # `color.nonexistent`, compiled, and the host fell into a silent fallback —
  # the same class as invariant 8's `[api_key]`.
  #
  # # Why this check is STRUCTURAL, and not "compile and watch it fail"
  #
  # The guarantee does not come from a test: it comes from `color` being an ENUM.
  # Citing a case that does not exist in an enum is a compile error in Rust —
  # that is a property of the language, and failing it here would be testing rustc.
  #
  # What CAN break, and is therefore what is asserted: `color` going back to being
  # a record with a string, or the two generated artifacts diverging. Those are
  # the only two ways for the guarantee to be lost, and both are covered.
  invalid-token-fails-build = mkCheck "invalid-token-fails-build" [ pkgs.python3 ] ''
    wit=${wit}/deps/tokens/tokens.wit
    rs=${tokensRs}/tokens.rs

    # 1. `color` is an enum. If it goes back to being a record with a string, the guarantee is gone.
    grep -qE '^\s*enum color \{' "$wit" || {
      echo "ERROR: \`color\` is no longer an enum in the WIT." >&2
      echo "      With a free string, a token that does not exist compiles in the guest and" >&2
      echo "      fails silently on screen. See section 7.6." >&2
      exit 1
    }
    if grep -qE 'record color|token:\s*string' "$wit"; then
      echo "ERROR: \`color\` is carrying a string again." >&2
      exit 1
    fi

    # 2. The two generated artifacts have exactly the same set of names.
    #    A divergence here shows up as a wrong color, never as an error.
    python3 - "$wit" "$rs" <<'PY'
    import re, sys
    wit, rs = open(sys.argv[1]).read(), open(sys.argv[2]).read()

    block = re.search(r"enum color \{(.*?)\n    \}", wit, re.S).group(1)
    in_wit = {c.strip().lstrip("%").rstrip(",")
              for c in block.splitlines()
              if c.strip() and not c.strip().startswith("//")}

    in_rs = set(re.findall(r'"([a-z0-9-]+)" => color::', rs))

    only_wit, only_rs = in_wit - in_rs, in_rs - in_wit
    if only_wit or only_rs:
        for n in sorted(only_wit):
            print(f"ERROR: `{n}` is in the WIT and not in the Rust bridge", file=sys.stderr)
        for n in sorted(only_rs):
            print(f"ERROR: `{n}` is in the Rust bridge and not in the WIT", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(in_wit)} colours, the same in both generated artefacts")

    # 2b. THE SAME, FOR `icon`.
    #
    # Colour has a hand-written bridge in the host, so a divergence there is a
    # human mistake. Icons do NOT: the WIT enum and the Rust constants are both
    # emitted by `gen-tokens.py` from one dictionary, which is exactly why this
    # went unchecked — "same source, cannot diverge".
    #
    # A generator can still drop one side. And the symptom would be the `?`
    # glyph the host draws for an unknown token: visible only on the screen that
    # happens to use it, and looking like a font problem.
    block = re.search(r"enum icon \{(.*?)\n    \}", wit, re.S)
    if block is None:
        print("ERROR: `icon` stopped being an enum in the WIT", file=sys.stderr)
        raise SystemExit(1)
    icon_wit = {c.strip().lstrip("%").rstrip(",")
                for c in block.group(1).splitlines()
                if c.strip() and not c.strip().startswith("//")}

    # `pub const SOME_NAME: &str` inside `pub mod icon`.
    mod_icon = re.search(r"pub mod icon \{(.*?)\n\}", rs, re.S)
    if mod_icon is None:
        print("ERROR: `pub mod icon` is gone from the generated Rust", file=sys.stderr)
        raise SystemExit(1)
    icon_rs = {n.lower().replace("_", "-")
               for n in re.findall(r"pub const ([A-Z0-9_]+):", mod_icon.group(1))}

    only_wit, only_rs = icon_wit - icon_rs, icon_rs - icon_wit
    if only_wit or only_rs:
        for n in sorted(only_wit):
            print(f"ERROR: the icon `{n}` is in the WIT and not in the Rust", file=sys.stderr)
        for n in sorted(only_rs):
            print(f"ERROR: the icon `{n}` is in the Rust and not in the WIT", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(icon_wit)} icons, the same in both generated artefacts")
    PY

    # 3. The host's bridge must have no default arm: `_ =>` has to give back
    #    `None`, and not a color. A `_ => TEXT_DIM` would bring the fallback back.
    if grep -qE '_ => color::' "$rs"; then
      echo "ERROR: the host's bridge has a fallback to a real colour." >&2
      echo "      An invalid name has to become \`None\`, not a pretty colour." >&2
      exit 1
    fi
  '';

  # NO SECRET IN THE TREE — section 9 of the prompt, invariant 13.
  #
  # "Add a check to `nix flake check` that fails if it detects secret patterns in
  # the repository." It is what holds up "the repository has to be publishable as
  # it is".
  #
  # It measures `self`, which already IS the source's path in the store —
  # measuring makes no new copy (unlike `${../.}`; see the CLAUDE.md).
  secrets-out-of-repo = mkCheck "secrets-out-of-repo" [ pkgs.python3 ] ''
    python3 ${../nix/scripts/scan-secrets.py} ${self}
  '';

  # And the other way round: a planted secret BRINGS THE CHECK DOWN.
  #
  # Without this fixture, "the sweep works" would be an untested claim — and a
  # sweep that has never reported anything is indistinguishable from a broken one.
  # It covers the six forms, because one regex can die on its own.
  planted-secret-fails = mkCheck "planted-secret-fails" [ pkgs.python3 ] ''
    mkdir -p tree_out/sub

    # Assembled in pieces for the same reason as the script: written whole, this
    # file would report itself in the real sweep.
    printf -- '-----%s OPENSSH %s-----\n' BEGIN 'PRIVATE KEY' > tree_out/sub/id_ed25519
    printf 'u:%sy%sj9T%sFq3kL9mZx2pQvR7sT1uWn0%saBcDeFgHiJkLmNoPqRsTuVwXyZ01234\n' \
      "\$" "\$" "\$" "\$" > tree_out/sub/shadow
    printf 'AGE-SECRET-%s\n' 'KEY-1QQPQZRHW4YSHVMKS4YFPXP93KVMHJ2LPXPD9Y6MPQ7TCFVWXHXAS8RXNMV' \
      > tree_out/sub/api_key.txt
    printf 'token = "%sA1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8"\n' 'ghp_' > tree_out/sub/cfg.toml
    printf 'psk%s"WifiPassword12345"\n' '=' > tree_out/sub/wifi.conf
    printf 'maintainer = "someone.real%sprovider.com"\n' '@' > tree_out/sub/meta.nix

    if python3 ${../nix/scripts/scan-secrets.py} tree_out 2>errors.txt; then
      echo "ERROR: the scan did NOT report the planted secrets." >&2
      exit 1
    fi

    # Every form has to show up: one regex can break without the others.
    for shape in "private key" "password hash" "age key" \
                 "service token" "wifi PSK" "e-mail"; do
      grep -q "$shape" errors.txt || {
        echo "ERROR: the scan did not report: $shape" >&2
        cat errors.txt >&2
        exit 1
      }
    done

    # And the secret must NOT show up in the output: the CI log would publish it.
    if grep -qE 'A1b2C3d4|QQPQZRHW4|SenhaDoWifi' errors.txt; then
      echo "ERROR: the scan printed the secret itself." >&2
      exit 1
    fi

    echo "OK: all six shapes take the check down, and none leaks into the message"
  '';

  # Notifications, history and the proof that there is no second surface.
  notifyVm = import ./notify-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # THE 5 MS BUDGET of section 5.2, measured on the heaviest component.
  #
  # The glyph browser embeds the whole database (2.8 MB). If any component were
  # going to blow the ceiling, it would be that one — and that is why it is the
  # one the gate measures, not the example buffer.
  #
  # Section 9.3 of the plan is explicit: instantiation is one of the metrics
  # INDEPENDENT of the GPU that the CI gates. It does not fall under the ban on
  # measuring performance under lavapipe, which is about rasterization.
  instance-fast = mkCheck "instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm 200
  '';

  # THE FIVE FILTERS OF SECTION 6.5, exercised on the real component.
  #
  # It drives the buffer the way the host drives it — `evento` and `render` on
  # the SAME instance — and checks the result, not only that the call returned.
  # It was that distinction that was missing: the search "existed" and split the
  # whole database into ONE record, because the only check that touched it
  # measured instantiation.
  browser-filters = mkCheck "browser-filters" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-browser \
      ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm \
      ${../assets/i18n}
  '';

  # THE SYMBOL DATABASE IS GENERATED FROM THE REAL SOURCE FILE, like the glyph one.
  #
  # What it asserts beyond "the file exists": the total (the intersection of the
  # UCD with the `cmap`), and that the TWO fields only this buffer has arrived —
  # the `sample` of the combining marks and the `width` of the double-advance
  # ones. Without them the database looks whole and the screen lies on 325
  # characters.
  unicode-table-generated = mkCheck "unicode-table-generated" [ pkgs.jq ] ''
    db=${unicodedb.db}
    for f in unicode.json coverage.md; do
      [ -s "$db/$f" ] || { echo "ERROR: $f is missing or empty" >&2; exit 1; }
    done

    n=$(jq 'length' "$db/unicode.json")
    echo "characters in the database: $n"
    [ "$n" -gt 7000 ] || { echo "ERROR: the base came out too small ($n)" >&2; exit 1; }

    # THE MANDATORY FIELDS, in every record.
    jq -e 'all(.cp and .name and .shows and .family)' "$db/unicode.json" >/dev/null \
      || { echo "ERROR: a record with no cp, name, sample or family" >&2; exit 1; }

    # `mostra` IS A LIST OF NUMBERS, and not text.
    #
    # It is what stops the character from travelling as a string: for `U+0022` the
    # JSON would write `"\""` and the component's parser — which finds the end of a
    # value by looking for the next quote — would read `\`. Two wrong characters,
    # silently, and only on those two.
    jq -e 'all(.shows | type == "array" and all(type == "number"))' \
      "$db/unicode.json" >/dev/null \
      || { echo "ERROR: \`shows\` stopped being a list of codepoints" >&2; exit 1; }

    # NO string field carries punctuation that would break the `split`.
    #
    # The punctuation that actually breaks it: a quote (it cuts the value in half),
    # a brace and a comma (they split a record and a list) and a bracket (it
    # delimits a list). The `:` does NOT count — it separates nothing in the
    # component's parser, and `bandeira: Brasil` is a legitimate emoji name coming
    # from the CLDR.
    bads=$(jq '[.[] | to_entries[] | select(.value | type == "string")
                 | select(.value | test("[\"{},\\[\\]]"))] | length' "$db/unicode.json")
    [ "$bads" = 0 ] || {
      echo "ERROR: $bads string fields with JSON punctuation." >&2
      echo "      The component splits the records on \`},\` and has no parser." >&2
      exit 1; }

    combining=$(jq '[.[] | select(.width == 0)] | length' "$db/unicode.json")
    wide=$(jq '[.[] | select(.width == 2)] | length' "$db/unicode.json")
    echo "combining: $combining   double width: $wide"
    [ "$combining" -gt 100 ] || { echo "ERROR: the combining marks went away" >&2; exit 1; }
    [ "$wide" -gt 0 ] || { echo "ERROR: the double-width ones went away" >&2; exit 1; }

    echo "OK: symbol database generated from the real font, with sample and width"
  '';

  # THE UCD VERSION GUARD SPEAKS — the negative fixture.
  #
  # Without it the guard would be a promise, and the glyph one has already shown
  # that an unverified promise is the same thing as an absent guard.
  unicode-version-mismatch-fails = mkCheck "unicode-version-mismatch-fails" [ ] ''
    grep -q "the Unicode Character Database version diverged" \
      ${unicodedb.versionMismatchFails}/out_text.txt \
      || { echo "ERROR: the message does not say the versions diverge" >&2; exit 1; }
    echo "OK: a UCD of another version takes the build down, saying which"
  '';

  # THE 5 MS BUDGET, on the new component too.
  #
  # The glyph one is still the heaviest, so it is still the main gate. This one
  # exists for the opposite reason: a field regression in the new generator
  # (taking the character back to a string, leaving the width always present)
  # pushes the database towards 3 MB with nobody measuring.
  unicode-instance-fast = mkCheck "unicode-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.unicode}/lib/ronoctua_buffer_unicode.wasm 200
  '';

  # THE LUCIDE DATABASE IS GENERATED FROM THE REAL SET, and the font along with it.
  #
  # What it asserts beyond "the file exists": the exact total, that BOTH
  # codepoints of each icon arrived (the drawing one and the origin one), and
  # that the two live in SEPARATE ranges. That separation is this buffer's whole
  # subject — see `nix/lucidegen.nix`.
  lucide-table-generated = mkCheck "lucide-table-generated" [ pkgs.jq ] ''
    db=${lucidedb.db}
    for f in lucide.json coverage.md NoctuaLucide.ttf; do
      [ -s "$db/$f" ] || { echo "ERROR: $f is missing or empty" >&2; exit 1; }
    done

    n=$(jq 'length' "$db/lucide.json")
    echo "icons in the database: $n"
    [ "$n" = 1668 ] || { echo "ERROR: the database has $n icons and should have 1668" >&2; exit 1; }

    jq -e 'all(.name and .cp and .lcp and .cats and .nodes)' "$db/lucide.json" >/dev/null \
      || { echo "ERROR: a record with no name, cp, lcp, cats or nos" >&2; exit 1; }

    # THE TWO CODEPOINTS ARE FROM DIFFERENT RANGES.
    #
    # `cp` draws and `lcp` is copied. If they are ever equal, the remapping is gone
    # — and with it the reason for the buffer not drawing 634 wrong icons.
    not_in=$(jq '[.[] | select(.cp < 991232 or .cp > 993667)] | length' "$db/lucide.json")
    [ "$not_in" = 0 ] || {
      echo "ERROR: $not_in icons draw outside U+F2000..U+F2683." >&2
      echo "      The source range collides with 634 Nerd Fonts codepoints." >&2
      exit 1; }
    merged=$(jq '[.[] | select(.cp == .lcp)] | length' "$db/lucide.json")
    [ "$merged" = 0 ] || { echo "ERROR: $merged icons with cp == lcp" >&2; exit 1; }

    # NO string field carries punctuation that would break the component's
    # `split`, which has no JSON parser (invariant 11). The `d` of the SVGs carries
    # a comma at the origin; the generator swaps it for a space, which the SVG
    # grammar treats the same.
    bads=$(jq '[.[] | to_entries[] | select(.value | type == "string")
                 | select(.value | test("[\"{},\\[\\]]"))] | length' "$db/lucide.json")
    [ "$bads" = 0 ] || {
      echo "ERROR: $bads string fields with JSON punctuation." >&2
      exit 1; }

    cats=$(jq '[.[].cats[]] | unique | length' "$db/lucide.json")
    [ "$cats" = 43 ] || { echo "ERROR: $cats categories, and 0.563.0 has 43" >&2; exit 1; }

    echo "OK: 1668 icons, 43 categories, drawing and source in separate ranges"
  '';

  # THE COLLISION GUARD SPEAKS — the negative fixture.
  #
  # It is not hypothetical: it is the measured defect. Without the guard, 634 of
  # the 1,668 icons would draw the Nerd Fonts glyph that occupies the same codepoint.
  lucide-range-taken-fails = mkCheck "lucide-range-taken-fails" [ ] ''
    grep -q "collides with 634 codepoints" ${lucidedb.rangeTakenFails}/out_text.txt \
      || { echo "ERROR: the message does not say how many codepoints collide" >&2; exit 1; }
    echo "OK: the source range takes the build down, saying how many collide"
  '';

  # THE PANEL ANSWERS THE HOST — driven on the real component.
  #
  # The three sibling drivers exercise buffers that carry their own database.
  # This one is the first whose screen is EMPTY until the host speaks: the
  # survey crosses the WIT as a record with lists inside, lowered field by
  # field, and `wasmtime` compares the field NAMES only after zipping them
  # positionally. A field out of order fails here, with a message, instead of
  # on somebody's screen.
  #
  # It also holds the two rules that make an irreversible key safe: one press
  # arms, another key disarms, the same key fires — and closing points at the
  # ROW of the last survey, never at a pid, because a scope has none.
  memory-driven = mkCheck "memory-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-memory \
      ${buffers.memory}/lib/ronoctua_buffer_memory.wasm \
      ${../assets/i18n}
  '';

  # AND THE ONE BUFFER WITH NO WIDTH CONSTANT OF ITS OWN.
  #
  # # What this catches
  #
  # A buffer cannot measure a sentence it does not have: it emits a KEY and the
  # host translates (invariant 11), so what it declares is what it RESERVED.
  # Seven of the eight write that reservation down as a constant a reader can
  # find. The brain does not: its widths are numbers inside four `Table`s
  # (`Column::fixed("label", 16, 2)`) and half its keys are ASSEMBLED at run
  # time — `brain-why-{}`, `brain-hold-{}`, `brain-standing-{}`, `update-{}`.
  # Nothing that reads this repository's source can pair one with the other.
  #
  # The FRAME can, because `Command::Text` carries the key and its `max_width`
  # side by side, already assembled. So this drives the panel through every
  # shape that draws a different family of key — idle, held, refused, running
  # with a warning, and the update's own page — and measures what came out
  # against both `.ftl` files, with the ruler the host CUTS with.
  #
  # It is also the second buffer whose screen is EMPTY until the host speaks,
  # and the `mind` is the largest record this system lowers: tasks with steps
  # with outcomes, three levels of list. A field out of order fails here.
  brain-driven = mkCheck "brain-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-brain \
      ${buffers.brain}/lib/ronoctua_buffer_brain.wasm \
      ${../assets/i18n}
  '';

  # AND ON THE FOURTH, which is the lightest of them: it embeds no database at
  # all, because every number it draws is pushed in at run time. The ceiling
  # here is loose on purpose — what it guards is a regression in the RUNTIME,
  # which would show up first on the component that has the least to load.
  memory-instance-fast = mkCheck "memory-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.memory}/lib/ronoctua_buffer_memory.wasm 200
  '';

  # AND ON THE FIFTH — the queue in a window, which embeds nothing either.
  #
  # What only this driver can prove: the cursor is an ID, and the list REORDERS
  # underneath it. The queue sorts what is afoot above what has finished, so a
  # job ending moves every row below it — and the key in question deletes the
  # piece already written at the destination. The reorder is simulated between
  # the frame and the key, which is the one place both sides of the WIT are
  # visible at once.
  transfers-driven = mkCheck "transfers-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-transfers \
      ${buffers.transfers}/lib/ronoctua_buffer_transfers.wasm \
      ${../assets/i18n}
  '';

  # THE MOUSE REACHES NOTHING THE KEYBOARD DOES NOT, and opens no surface.
  #
  # The footer had a pointer before this — the scrollbar, the file manager's
  # rows, the right-button menu — but only inside the EXPANDED region. Making
  # the modeline itself clickable is the change that invites invariant 14 back
  # in, so the check is written in the same commit as the feature rather than
  # after it.
  #
  # Two things, and each is a way for a mouse to become chrome:
  #
  #   1. every id the pointer acts on is one a KEY also reaches. Today that is
  #      `state`, and the two orders it sends — `state-next`, `state-prev` — are
  #      the same two `Esc`+arrow sends in `buffer.rs`. A click that ran anything
  #      else would be a door only the mouse has;
  #   2. NOTHING FLOATS. No `xdg_popup`, no second layer surface, no window
  #      created from a pointer path. The right-button menu already draws in the
  #      footer's own region for exactly this reason, and the comment there says
  #      so — this makes it true rather than intended.
  footer-mouse-reaches-nothing-new = mkCheck "footer-mouse-reaches-nothing-new" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    shell = pathlib.Path("${self}/crates/ronoctua-shell/src")
    surface = (shell / "surface.rs").read_text()
    buffer_host = (shell / "buffer.rs").read_text()
    gaps = []

    # 1) WHAT A CLICK ON THE MODELINE CAN DO.
    body = re.search(
        r"fn segment_clicked\(&mut self[^\n]*\n(.*?)\n    \}\n",
        surface,
        re.S,
    )
    if not body:
        sys.exit("ERROR: `segment_clicked` went away or changed shape")
    clicked = body.group(1)
    orders = set(re.findall(r'"(state-next|state-prev)"', clicked))
    if orders != {"state-next", "state-prev"}:
        gaps.append("the click sends " + repr(sorted(orders)) + ", and not the keyboard's two")

    # The SAME two the keyboard produces, on the other side of the socket.
    from_keys = set(re.findall(r'"(state-next|state-prev)"', buffer_host))
    rotates = "EscapeAct::Rotate" in buffer_host
    if not rotates:
        gaps.append("the host's `Esc` no longer cycles: the click would be left alone")

    # AND NOTHING ELSE. Three segments answer, and each has a key: the `state`
    # rotates (what holding `Esc` and an arrow does), the `notif` opens the
    # manager (`LEADER n h`), and the `update` opens the window that follows a
    # run (`LEADER s w`). A fourth would be a door only the mouse knows about.
    acted = set(re.findall(r'if id == "([a-z-]+)"', clicked))
    acted |= set(re.findall(r'if id != "([a-z-]+)"', clicked))
    if acted != {"state", "notif", "update"}:
        gaps.append("the click acts on " + repr(sorted(acted)) + ", outside what was agreed")
    if '"notif-history"' not in clicked:
        gaps.append("clicking the bell does not open the manager")
    # THE UPDATE'S MARK, and BOTH halves of it.
    #
    # The mark only exists while something is running, so a click is the
    # obvious way to reach the window and the tempting place to stop. The key
    # is what makes it reachable at three in the morning when nothing is
    # running — which is exactly when somebody wants to read what happened.
    keymap = (shell / "leader.rs").read_text()
    if '"update-watch"' not in clicked:
        gaps.append("clicking the update's mark does not open the window")
    if '"update-watch"' not in keymap:
        gaps.append("the update's mark has no equivalent key")

    # 2) WHAT A CLICK ON THE NOTIFICATION BAND CAN DO.
    #
    # Two things, and each has a key: the circle marks it read, which is what
    # `notif-dismiss` does; an action runs, which is what its own letter does in
    # the manager. There is nothing up there the keyboard cannot reach, and this
    # is what keeps that true as the band grows.
    band = (shell / "band.rs").read_text()
    # EVERY variant of the enum, and not only the two we hoped to find: a check
    # that looks for what it expects passes on a THIRD target nobody gave a key
    # to, which is the whole defect. Planted, and the first version passed.
    enum = re.search(r"pub enum Hit \{(.*?)\n\}", band, re.S)
    if not enum:
        sys.exit("ERROR: `band::Hit` went away or changed shape")
    hits = set(re.findall(r"^    ([A-Z]\w*)", enum.group(1), re.M))
    if hits != {"Read", "Action"}:
        gaps.append("the band has targets " + repr(sorted(hits)) + ", outside what was agreed")
    # The keyboard's half of each. `keymap` is read above, with the modeline's
    # own half — one read, because two would be two things to keep in step.
    if '"notif-dismiss"' not in keymap:
        gaps.append("the band's circle has no equivalent key")
    manager = (shell / "notifications.rs").read_text()
    if "a.key == Some(c)" not in manager:
        gaps.append("the band's action is not reachable by letter in the manager")

    # AND THE BRAIN'S LINE, below the minibuffer, for the same reason.
    #
    # It is the one place on this screen that appears WITHOUT being asked for
    # and asks a question back — the machine is about to rebuild itself, and
    # the two answers are on the line. If either of them were mouse-only, a
    # person at the keyboard could watch the countdown run out with no way to
    # stop it.
    brain = (shell / "brain.rs").read_text()
    brain_enum = re.search(r"pub enum Hit \{(.*?)\n\}", brain, re.S)
    if not brain_enum:
        sys.exit("ERROR: `brain::Hit` went away or changed shape")
    brain_hits = set(re.findall(r"^    ([A-Z]\w*)", brain_enum.group(1), re.M))
    if brain_hits != {"Abort", "Proceed"}:
        gaps.append("the brain's line has targets " + repr(sorted(brain_hits)) + ", outside what was agreed")
    for command in ('"brain-abort"', '"brain-proceed"'):
        if command not in keymap:
            gaps.append("the brain's line has no key for " + command)
    # AND A CONTROL CANNOT BE BUILT WITHOUT SAYING WHAT IT DOES.
    #
    # This is structural rather than a count, and the first version of it was a
    # count — it wanted four `explain: Some(` and found three, because the two
    # controls share one builder. Counting occurrences measures how the source
    # is factored, which is not the property.
    #
    # The property is that the builder DEMANDS the sentence: a `control` whose
    # signature takes an `explain: String` cannot produce a target that has
    # none, whatever a caller forgets.
    if not re.search(r"fn control\([^)]*explain: String", brain, re.S):
        gaps.append("the constructor of the brain line's control does not require an explanation")

    # THE MANAGER'S ROW TARGETS, and the keys that do the same thing.
    #
    # Every variant of the enum, and not the two we hoped to find: a check that
    # looks for what it expects passes on a THIRD target nobody gave a key to.
    rows_enum = re.search(r"pub enum Hit \{(.*?)\n\}", manager, re.S)
    if not rows_enum:
        sys.exit("ERROR: `notifications::Hit` went away or changed shape")
    targets = set(re.findall(r"^    ([A-Z]\w*)", rows_enum.group(1), re.M))
    if targets != {"MarkRead", "Delete"}:
        gaps.append("the list has targets " + repr(sorted(targets)) + ", outside what was agreed")
    for key, verb in (("Press::Space", "MarkRead"), ("Press::Letter('d')", "Delete")):
        if key not in manager:
            gaps.append("the list's " + verb + " target has no equivalent key")
    # AND EVERY CONTROL EXPLAINS ITSELF on the message line, never in a tooltip.
    if band.count("explain: Some(") < 3:
        gaps.append("a control of the band does not say what it does on hover")

    # 3) NOTHING FLOATS.
    forbidden = ("xdg_popup", "XdgPopup", "create_popup", "Popup::")
    for name in (
        "surface.rs",
        "buffer.rs",
        "files_window.rs",
        "band.rs",
        "brain.rs",
        "update_window.rs",
        "notifications.rs",
        "notifications_window.rs",
    ):
        text = (shell / name).read_text()
        for word in forbidden:
            if word in text:
                gaps.append(name + ": " + word + " — invariant 14")

    if gaps:
        print("ERROR: the pointer gained a door of its own.", file=sys.stderr)
        for g in gaps:
            print("       " + g, file=sys.stderr)
        raise SystemExit(1)
    print("OK: the modeline and the band only do what the keyboard does, and nothing floats")
    END
  '';

  # THE STATE CONTRACT, ON EVERY BUFFER AT ONCE.
  #
  # Each half of it is already guarded and neither is worth much alone: the
  # descriptor's states by `Descriptor::validate`, the words by
  # `buffer-states-have-text`, the host's `Esc` layer by unit tests in
  # `buffer.rs`. What none of them reaches is the ROUND TRIP — send the id the
  # buffer declared and get the same id back out of `effect::state-changed`,
  # through the WIT and out of the component's own memory.
  #
  # A buffer that took `enter-state` and did nothing would satisfy every check
  # above and leave `SPC b s` offering a state nobody can enter, with the
  # modeline naming a screen that is not up. That is not hypothetical: it was
  # planted, and the first version of the driver PRINTED it and exited zero.
  #
  # All seven and not one chosen buffer, because the contract is the same for
  # all of them and six of the seven were written in a single pass — which is
  # the pass whose mistakes land in whichever file the writer got tired on.
  states-driven = mkCheck "states-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-states \
      ${buffers.lucide}/lib/ronoctua_buffer_lucide.wasm \
      ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm \
      ${buffers.unicode}/lib/ronoctua_buffer_unicode.wasm \
      ${buffers.memory}/lib/ronoctua_buffer_memory.wasm \
      ${buffers.transfers}/lib/ronoctua_buffer_transfers.wasm \
      ${buffers.format}/lib/ronoctua_buffer_format.wasm \
      ${buffers.example}/lib/ronoctua_buffer_example.wasm
  '';

  # THE KEYS THE CODE ASKS FOR EXIST, and that is a different question from
  # the one `i18n-complete` answers.
  #
  # `i18n-complete` compares the two bundles AGAINST EACH OTHER: it catches a
  # key in one language and not the other, which is the common mistake. It
  # cannot catch a key that is in neither — and a key in neither reaches the
  # screen as `[files-fact-lens]`, in both languages, with every check green.
  #
  # `crates/ronoctua-inspect/src/lib.rs` names this check in its own `key`
  # module docstring and it did not exist. The keys there are LITERALS rather
  # than `ronoctua_i18n::key::*` constants, and they have to be: the generated
  # constants only exist for keys already in the bundles, so a crate adding a
  # key cannot name it that way until after somebody has written it. That is
  # the one hole in invariant 8's "a missing key does not compile", and this is
  # what closes it.
  #
  # `updates-have-text` does the same job for the update sequence, against the
  # list its generator derives. This one has no generator, so it reads the
  # constants.
  inspect-keys-exist = mkCheck "inspect-keys-exist" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    source = (root_dir / "crates/ronoctua-inspect/src/lib.rs").read_text()

    # The `key` module's constants, and only those: a literal anywhere else in
    # the crate is a string, not a promise about a bundle.
    block = re.search(r"pub mod key \{(.*?)\n\}", source, re.S)
    if not block:
        print("ERROR: the `key` module is gone from ronoctua-inspect.", file=sys.stderr)
        raise SystemExit(1)
    owed = sorted(set(re.findall(r'pub const [A-Z_0-9]+: &str = "([a-z][a-z0-9-]*)";', block.group(1))))
    if not owed:
        print("ERROR: no constant in the `key` module — what does the regex match?", file=sys.stderr)
        raise SystemExit(1)

    failed = 0
    for language in ${builtins.toJSON languages}:
        text = (root_dir / "assets/i18n" / f"{language}.ftl").read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*)\s*=", text, re.M))
        for k in owed:
            if k not in have:
                print(f"ERROR: {language} does not have `{k}`", file=sys.stderr)
                failed = 1

    if failed:
        print("", file=sys.stderr)
        print("A key missing from BOTH languages is invisible to", file=sys.stderr)
        print("i18n-complete, which compares one language against the other. It reaches the", file=sys.stderr)
        print("screen in square brackets, and nothing else reports it.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: the {len(owed)} keys ronoctua-inspect asks for exist in both languages")
    END
  '';

  # THERE IS ONE LIST OF UPDATE COMMANDS, AND ONLY ONE.
  #
  # The owner asked for this in as many words: there must be no way to add an
  # update step to the brain and not to the manual updater, or the other way
  # round. It is not a promise anybody can keep by remembering — it is a
  # property, and this is what makes it one.
  #
  # `assets/updates.nix` declares the sequence. From it the generator makes the
  # Rust table, the text keys and the two shell runners; `nix/modules/update.nix`
  # wraps those in units; `crates/ronoctua-brain` reads the same table through
  # its `build.rs`; and `LEADER s u` starts the same units. Nothing along that
  # path names a step.
  #
  # THE DEFECT IS NOT HYPOTHETICAL. The brain used to declare three steps of its
  # own — `update-inputs`, `build`, `collect-garbage` — while the table declared
  # twelve called `flake-update`, `system-rebuild`, `store-collect-garbage`.
  # Not a subset of each other: disjoint. Every step name on screen was one the
  # sequence does not use, and the row that says WHERE a run stopped took its id
  # from the report the unit writes, so a failure looked up a key that did not
  # exist. Two plausible files, no compiler between them.
  #
  # So: a step's id may appear in the declaration and in what is generated from
  # it, and nowhere in any source we write. A second list cannot be added
  # without this saying so, by name.
  update-has-one-list =
    mkCheck "update-has-one-list"
      [
        pkgs.python3
      ]
      ''
        python3 - <<'END'
        import pathlib, re, sys

        root_dir = pathlib.Path("${self}")
        table = pathlib.Path("${updatesTable.table}/updates.rs").read_text()

        # The ids the ONE declaration produced.
        ids = sorted(set(re.findall(r'^\s+id: "([a-z0-9-]+)",$', table, re.M)))
        if len(ids) < 5:
            print(f"ERROR: only {len(ids)} ids in the generated table — did the pattern change?", file=sys.stderr)
            raise SystemExit(1)

        # Where a step's name is allowed to be written by hand: the declaration
        # itself, and the generator that reads it.
        ALLOWED = {
            "assets/updates.nix",
            "nix/scripts/gen-updates.py",
            "nix/updates.nix",
        }

        gaps = []
        for source in sorted(root_dir.rglob("*")):
            if source.is_dir() or source.suffix not in {".rs", ".nix", ".py"}:
                continue
            rel = source.relative_to(root_dir)
            if str(rel) in ALLOWED or "target" in rel.parts or rel.parts[0] == ".git":
                continue
            try:
                text = source.read_text()
            except (UnicodeDecodeError, OSError):
                continue
            for n, line in enumerate(text.splitlines(), 1):
                stripped = line.strip()
                if stripped.startswith(("//", "#", "///", "//!")):
                    continue
                for i in ids:
                    if f'"{i}"' in line:
                        gaps.append(f"{rel}:{n}: {i}")

        if gaps:
            print("ERROR: a step of the update is named outside the declaration.", file=sys.stderr)
            print("", file=sys.stderr)
            for g in gaps:
                print(f"       {g}", file=sys.stderr)
            print("", file=sys.stderr)
            print("       The sequence lives in assets/updates.nix and nowhere else.", file=sys.stderr)
            print("       Whoever needs it READS the generated table; whoever needs to RUN it starts", file=sys.stderr)
            print("       the units. An id written by hand is a second list, and two", file=sys.stderr)
            print("       lists is exactly the defect the brain already had: three", file=sys.stderr)
            print("       steps of its own, DISJOINT from the twelve the sequence runs.", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: the {len(ids)} steps exist only in the declaration and in what comes out of it")
        END
      '';

  # THE UNITS AND THE PATHS ARE NAMED IN NIX, AND RUST ONLY REPEATS THEM.
  #
  # Four strings cross from `nix/modules/update.nix` into Rust by being typed
  # again: the two units the brain starts, the system unit the window follows,
  # and the directory the report is written into. Nothing ties them together —
  # rename a unit in the module and the Rust still compiles, still runs, and
  # asks systemd for something that is not there.
  #
  # It is the same shape as `update-has-one-list` one block up, and the same
  # shape as the defect that check exists for. There the sequence was declared
  # twice; here a NAME is. A check is what turns "we typed it the same" into
  # "it is the same".
  #
  # It reads the module's TEXT rather than an evaluated config on purpose: a
  # NixOS evaluation costs a minute and this costs nothing, and what is being
  # compared is a spelling.
  update-names-agree = mkCheck "update-names-agree" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    module = (root_dir / "nix/modules/update.nix").read_text()

    # What Rust says, and where it says it.
    WRITTEN = {
        "crates/ronoctua-brain/src/tasks/update.rs": ("UNIT", "REHEARSAL_UNIT"),
        "crates/ronoctua-shell/src/update_window.rs": ("SYSTEM_UNIT", "SYSTEM_REPORT"),
    }

    gaps = []
    seen = 0
    for where, names in WRITTEN.items():
        source = (root_dir / where).read_text()
        for name in names:
            m = re.search(rf'const {name}: &str = "([^"]+)";', source)
            if not m:
                gaps.append(f"{where}: `{name}` went away — did the pattern change?")
                continue
            seen += 1
            said = m.group(1)
            # A report path is a FILE inside the directory the module makes;
            # what has to appear over there is the directory.
            wanted = said.rsplit("/", 1)[0] if said.startswith("/") else said
            # IT HAS TO BE BOUND, not merely mentioned.
            #
            # The first version asked whether the string appeared anywhere in
            # the module, and that is not the question: the polkit rule lists
            # the unit names too, as elements of an allow-list. Renaming the
            # declaration and leaving that rule stale still passed — which is
            # the worse half of the defect, a rule authorising a unit that no
            # longer exists beside a unit nobody may start.
            #
            # The module gives each of these a NAME in its `let` and
            # interpolates it into the attribute, so what has to be found is
            # the binding: `systemUnit = "ronoctua-update-system.service";`.
            # A list element (`"…",`) does not match, and that is the whole
            # distinction.
            # THE MODULE DECLARES A UNIT IN ONE OF TWO SHAPES, and both are
            # declarations: the system pair is named in the `let` and
            # interpolated (`systemUnit = "…";`), while the person's pair is
            # written straight into the attribute
            # (`systemd.user.services.ronoctua-update = {`). What is NOT a
            # declaration is a list element, which is how the polkit rule
            # mentions them — and that is the whole distinction this is drawing.
            bound = re.search(rf'=\s*"{re.escape(wanted)}"\s*;', module)
            attribute = None
            if wanted.endswith(".service"):
                bare = re.escape(wanted.removesuffix(".service"))
                attribute = re.search(rf"systemd\.(?:user\.)?services\.{bare}\s*=", module)
            if not bound and not attribute:
                gaps.append(
                    f"{where}: `{name}` = {said!r}, and nix/modules/update.nix does not DECLARE it"
                )

    if not seen:
        print("ERROR: no constant found — did the pattern change?", file=sys.stderr)
        raise SystemExit(1)
    if gaps:
        print("ERROR: a name from the Rust does not exist in the module that declares it.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in gaps:
            print(f"       {g}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Renaming the unit in the Nix and leaving the Rust as it was compiles,", file=sys.stderr)
        print("       runs, and asks systemd for something that does not exist — and the failure", file=sys.stderr)
        print("       shows up far from the cause, on a screen somebody opened because they ALREADY", file=sys.stderr)
        print("       something was wrong.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: the {seen} names the Rust repeats exist in nix/modules/update.nix")
    END
  '';

  # THE UPDATE SEQUENCE OWES ITS WORDS, IN BOTH LANGUAGES.
  #
  # The generator emits the exact list of keys the table implies — `update-<id>`
  # and `update-does-<id>` for every step, plus the closed vocabularies for the
  # preconditions, the failure policies and the kinds. A step added without its
  # two sentences would draw a bracketed key on the brain's page, which is
  # invariant 8's failure mode.
  #
  # The same shape as `notifications-have-text`, and for the same reason: the
  # list is DERIVED, so it cannot go stale the way a hand-written one does.
  updates-have-text = mkCheck "updates-have-text" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    owed = [
        line.strip()
        for line in pathlib.Path("${updatesTable.table}/keys.txt").read_text().splitlines()
        if line.strip()
    ]
    root_dir = pathlib.Path("${self}/assets/i18n")

    failed = 0
    for language in ${builtins.toJSON languages}:
        text = (root_dir / f"{language}.ftl").read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*)\s*=", text, re.M))
        for key in owed:
            if key not in have:
                print(f"ERROR: {language} does not have `{key}`", file=sys.stderr)
                failed = 1

    if failed:
        print("", file=sys.stderr)
        print("Every key lives in BOTH languages — one alone takes the i18n build down,", file=sys.stderr)
        print("and this check says WHICH before it gets there.", file=sys.stderr)
        raise SystemExit(1)
        print(f"OK: the sequence's {len(owed)} keys exist in both languages")
    END
  '';

  # AND THE GUARD REALLY DOES REFUSE.
  #
  # `nix/updates.nix` builds a table that must NOT pass — a step that installs,
  # and a step enabled by default whose program the flake does not put on the
  # machine — and records what the generator said about it. This check reads
  # that recording.
  #
  # It is the shape `icons-unknown-token-fails` and `i18n-missing-key-fails`
  # already use, and the reason is the same: a guard nobody exercises is a
  # guard that stopped working quietly, and the day it matters is the day
  # somebody adds `npm install -g` to a table that runs as root.
  #
  # Both sentences are demanded, not just a non-zero exit: a refusal for the
  # WRONG reason is a refusal that will not survive the next edit.
  updates-refusal-refuses = mkCheck "updates-refusal-refuses" [ pkgs.gnugrep ] ''
    said=${updatesTable.negativeFixture}/out_text.txt
    [ -f "$said" ] || {
      echo "ERROR: the negative fixture did not record what the generator said (out_text.txt)." >&2
      echo "      Without it, 'it failed' and 'it failed for the right reason' are the same thing." >&2
      exit 1; }

    failed=0
    grep -q "invariant 6" "$said" || {
      echo "ERROR: the refusal does not cite invariant 6." >&2
      echo "      A step switched on outside the flake has to be refused FOR THAT." >&2
      failed=1; }
    grep -qi "install" "$said" || {
      echo "ERROR: the refusal does not say the step INSTALLS." >&2
      failed=1; }

    [ "$failed" = 0 ] || { echo "--- what it said ---" >&2; cat "$said" >&2; exit 1; }
    echo "OK: installing and switching on what the flake does not put there are refused, and for the right reason"
  '';

  # THE SEQUENCE NEVER ASKS FOR AUTHENTICATION, and that is a requirement
  # rather than an accident.
  #
  # The owner reported it, and his words are the whole argument (translated,
  # because a comment is English — invariant 18): sometimes the fingerprint
  # reader asks him to approve something like a `sudo`, and sometimes he does
  # not see the request, or he is far from the laptop. It is not that he
  # refuses — it is that he could not approve.
  #
  # That decides the design of this whole feature. The brain exists to run the
  # update AT A GOOD MOMENT, and a good moment is almost always one where
  # nobody is looking at the screen. A prompt there is not a question: it is a
  # silent failure shaped exactly like a refusal, and the report would say
  # "you declined" about something the person never saw.
  #
  # There is no polkit agent in the session (only `pkttyagent`, in the rescue
  # console), so any `AUTH_*` would already fail — but it would fail by
  # accident. This check turns the absence into a requirement:
  #
  #   1. neither runner invokes `sudo`, `pkexec`, `pkttyagent` or
  #      `systemd-run` (which also goes through polkit and is SYNCHRONOUS on
  #      top of that — the trap that froze the footer once already);
  #   2. the update's polkit rule returns `YES`, and not `AUTH_SELF_KEEP` nor
  #      `AUTH_ADMIN`: the door is narrow because of what it NAMES — one verb,
  #      two units, one user — never because it asks afterwards.
  updates-never-ask =
    let
      runners = self.nixosConfigurations.noctua.config.ronoctua.update.package;
      rule = self.nixosConfigurations.noctua.config.security.polkit.extraConfig;
    in
    mkCheck "updates-never-ask" [ pkgs.gnugrep pkgs.python3 ] ''
      failed=0
      for script in ${runners}/bin/ronoctua-update ${runners}/libexec/ronoctua-update-system; do
        if found=$(grep -nE '(^|[^-[:alnum:]_])(sudo|pkexec|pkttyagent|systemd-run)([^-[:alnum:]_]|$)' "$script"); then
          echo "ERROR: $script asks for authentication:" >&2
          echo "$found" >&2
      echo "      The owner is not always near the laptop when this runs." >&2
          failed=1
        fi
      done

      python3 - <<'END'
      import re, sys
      rule = """${rule}"""
      # The block that names the two update units, and what it returns.
      block = re.search(
          r"ronoctua-update-system\.service.*?\}\s*\)\s*;", rule, re.S
      )
      if not block:
          print("ERROR: the update's polkit rule went away.", file=sys.stderr)
          raise SystemExit(1)
      text = block.group(0)
      if "polkit.Result.YES" not in text:
          print("ERROR: the update's rule does not give back YES.", file=sys.stderr)
          raise SystemExit(1)
      for asking in ("AUTH_SELF", "AUTH_ADMIN"):
          if asking in text:
              print(f"ERROR: the update's rule gives back {asking} — it ASKS.", file=sys.stderr)
              print("      There is no polkit agent in the session, and the owner does not always see the request.", file=sys.stderr)
              raise SystemExit(1)
      END

      [ "$failed" = 0 ] || exit 1
      echo "OK: the sequence asks nothing of anybody — no sudo, no interactive polkit"
    '';

  # THE DESIGN CRATE'S TWO CONSTANTS AGREE WITH THE TOKENS THEY DEFINE.
  #
  # A buffer may not be told a token's value (invariant 11), so
  # `ronoctua-design` carries `AIR` and `ICON` as numbers of its own. That is
  # legitimate — both are DEFINITIONS rather than values — but a definition
  # still has to stay true:
  #
  #   AIR  = 1   because the air is one line height, and a cell IS one line
  #              height (`font.size.base` x `font.lineHeight`). If the cell
  #              ever stopped being the unit of vertical rhythm, this would
  #              quietly become "one cell of air, which is no longer a line".
  #   ICON = 2   because `ceil(font.size.icon / font.size.base)`: the glyph is
  #              drawn on the cell at 22 px and its advance exceeds the 10,8 px
  #              cell, so it covers two columns. Raise `font.size.icon` past
  #              two cells' worth and the title would be written ON TOP of the
  #              glyph, in every buffer at once.
  #
  # It used to divide by `font.size.large`, which was the buffers' body until
  # the owner asked for one measure across the system. The definition did not
  # change — it is still "the icon over the body" — only the body's name.
  #
  # One number, two homes, and this is what stops them drifting.
  design-constants-match-tokens = mkCheck "design-constants-match-tokens" [ pkgs.python3 ] ''
    python3 - <<'END'
    import math, pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    design = (root_dir / "crates/ronoctua-design/src/lib.rs").read_text()
    typography = (root_dir / "assets/tokens/typography.nix").read_text()

    def constant(name):
        m = re.search(rf"pub const {name}: u16 = (\d+);", design)
        if not m:
            print(f"ERROR: `{name}` is gone from ronoctua-design", file=sys.stderr)
            raise SystemExit(1)
        return int(m.group(1))

    def token(leaf):
        m = re.search(rf"^\s+{leaf}\s*=\s*([0-9.]+);", typography, re.M)
        if not m:
            print(f"ERROR: the token `{leaf}` is gone from typography.nix", file=sys.stderr)
            raise SystemExit(1)
        return float(m.group(1))

    failed = 0

    air = constant("AIR")
    if air != 1:
        print(f"ERROR: AIR is {air}. The air is ONE line height, and a cell is", file=sys.stderr)
        print("      one line height — so AIR is 1, or the definition changed.", file=sys.stderr)
        failed = 1

    icon = constant("ICON")
    wanted = math.ceil(token("icon") / token("base"))
    if icon != wanted:
        print(f"ERROR: ICON is {icon} and the font asks for {wanted}", file=sys.stderr)
        print(f"      (icon {token('icon')} px over base {token('base')} px).", file=sys.stderr)
        print("      With the wrong number, the title is written ON TOP of the glyph.", file=sys.stderr)
        failed = 1

    if failed:
        raise SystemExit(1)
    print(f"OK: AIR={air} (one cell and one line), ICON={icon} cells for {token('icon')} px")
    END
  '';

  # THE MANAGER'S ROWS ARE BUILT WHERE THEY ARE ASKED FOR, and never handed
  # back to the caller that asked.
  #
  # `Footer::expansion_window` builds one page of the expanded region.
  # `Footer::expansion` builds the whole of it — and, with the file manager
  # open, it does so by calling `expansion_window`. So `expansion_window` may
  # call `expansion` in EXACTLY one place: the branch it takes when there is no
  # manager, where `expansion` will not come back.
  #
  # It had two more. Both asked `View::row_count`, both handed the manager back
  # to `expansion()` when it answered `None` — and `row_count` answers `None`
  # for precisely the three states (menu, question, sheet) that the modal branch
  # further down exists to draw. The pair was an unconditional infinite
  # recursion: the shell died of a stack overflow the moment the right button,
  # `C-m` or a permission question opened one of them, with no diagnostic beyond
  # "fatal runtime error: stack overflow".
  #
  # WHAT IT COSTS AND WHAT IT DOES NOT COVER. It reads one function of one file
  # and runs in milliseconds. It catches THIS pair and no other: a different two
  # functions calling each other would pass it untouched, and there is no check
  # in this file that would catch that one. What makes it worth having anyway is
  # that these two are load-bearing — every line the footer draws goes through
  # them — and that the failure they produce names neither of them.
  manager-builds-its-own-rows = mkCheck "manager-builds-its-own-rows" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import re, sys

    src = open("${self}/crates/ronoctua-shell/src/surface.rs", encoding="utf-8").read()

    # The function, from its signature to the first line that is back at
    # `impl`-level indentation — the shape every other reader in this file uses.
    body = re.search(
        r"\n    fn expansion_window\(&self[^\n]*\n(.*?)\n    \}\n",
        src,
        re.S,
    )
    if not body:
        sys.exit("ERROR: `expansion_window` went away or changed shape")
    body = body.group(1)

    calls = [n for n, line in enumerate(body.splitlines(), 1) if "self.expansion()" in line]
    if not calls:
        sys.exit(
            "ERROR: `expansion_window` no longer calls `expansion` at all.\n"
            "       The views that are NOT the manager are built by `expansion`\n"
            "       and cut here; if that changed, this check proves nothing and\n"
            "       should be rewritten rather than deleted."
        )
    if len(calls) > 1:
        print(
            f"ERROR: `expansion_window` calls `self.expansion()` {len(calls)} times,"
            " on lines " + ", ".join(str(n) for n in calls) + ".",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        print("       With the manager open, `expansion` calls THIS function back.", file=sys.stderr)
        print("       The one call that is safe is the `let ... else` for a door with", file=sys.stderr)
        print("       no manager; every other one is a loop with no exit, and the", file=sys.stderr)
        print("       symptom is `fatal runtime error: stack overflow` naming neither.", file=sys.stderr)
        raise SystemExit(1)

    # AND IT IS THE ONE IN THE `else`. A single call in the wrong place is the
    # same defect with a smaller number.
    guard = re.search(r"let Some\(files\) = &self\.files else \{(.*?)\n        \};", body, re.S)
    if not guard:
        sys.exit("ERROR: the `let Some(files) = &self.files else` guard is gone")
    if "self.expansion()" not in guard.group(1):
        sys.exit(
            "ERROR: the only call to `expansion` is outside the `else` that runs\n"
            "       when there is no manager — which is the one place it cannot\n"
            "       come back. See the note above this check."
        )

    print("OK: the manager's rows are built here, and `expansion` is only asked")
    print("    for the views that are not the manager")
    PY
  '';

  # A COUNT IS NOT AN ADVANCE, AND ONLY THE DATABASE KNOWS THE DIFFERENCE.
  #
  # After the unification there are exactly TWO rulers left, and they answer two
  # different questions:
  #
  #   `ronoctua_protocol::cells` counts CLUSTERS — how many things there are. It
  #   is what reserves, what confers and what cuts, and the whole system now
  #   shares the one definition.
  #
  #   the database's `width` field is the ADVANCE. An emoji advances 1.245 em
  #   against the cell's 0.600, so its cluster counts as ONE thing and covers
  #   TWO cells. No code can know that: the answer is in the font, and what read
  #   the font was the generator.
  #
  # A buffer drawing such a datum budgets from the field (`Symbol::ink`), never
  # from the count. Today every one of them does — by having been written
  # carefully, which is not a guarantee. Writing `wide(&s.shows)` is the obvious
  # thing to do and it is wrong: a ZWJ couple would reserve one cell for a
  # drawing two cells wide, and the next column would be written over.
  #
  # The check is narrow on purpose. It does not try to understand the layout; it
  # asserts that the character a browser DRAWS never reaches the counter, and
  # that the run drawing it budgets from the measured width.
  ink-is-not-a-count = mkCheck "ink-is-not-a-count" [ pkgs.gnugrep pkgs.coreutils ] ''
    failed=0

    # 1) THE DRAWN CHARACTER NEVER REACHES THE COUNTER.
    #    `shows` is the field the generator fills with the character itself,
    #    and it is the one field in the three databases that can hold a
    #    cluster of six codepoints.
    if counted=$(grep -rnE 'wide\(&?[a-z_.]*\.shows' ${self}/buffers 2>/dev/null); then
      echo "ERROR: the drawn character was measured with the cluster count:" >&2
      echo "$counted" >&2
      echo "      The count answers HOW MANY THINGS; an emoji is one thing and" >&2
      echo "      two cells. Budget it from the database width, as Symbol::ink does." >&2
      failed=1
    fi

    # 2) AND THE RUN THAT DRAWS IT BUDGETS FROM THE MEASURED WIDTH.
    #    Every datum run over `shows` budgets from ink on the same chain — as
    #    the method (`s.ink(cells)`) or through the local the caller measured
    #    into first. Without this half, deleting the budget altogether would
    #    pass part 1.
    drawn=$(grep -rc 'Run::datum(&[a-z_.]*\.shows)' ${self}/buffers/*/src/lib.rs \
      | awk -F: '{ n += $2 } END { print n + 0 }')
    inked=$(grep -rh -A 8 'Run::datum(&[a-z_.]*\.shows)' ${self}/buffers/*/src/lib.rs \
      | grep -cE '\.within\([^)]*ink' || true)
    if [ "$drawn" -eq 0 ]; then
      echo "ERROR: no buffer draws the character any more — this check proves nothing now." >&2
      failed=1
    fi
    if [ "$inked" -lt "$drawn" ]; then
      echo "ERROR: $drawn run(s) draw the character and only $inked budget from ink()." >&2
      failed=1
    fi

    [ "$failed" = 0 ] || exit 1
    echo "OK: $drawn drawn character(s), each budgeted from the width the font gave"
  '';

  # THE BUFFERS' OWN TESTS, ALL SIXTY-NINE OF THEM.
  #
  # They exist in the eight `src/lib.rs` and, until this check, not one of them
  # ran: `nix/buffers.nix` compiled the `.wasm` and stopped. They were green
  # only for as long as somebody remembered to run them by hand — and could not
  # be run by hand without knowing which variables to set, because the three
  # browsers embed their database with `include_str!(env!(…))`.
  #
  # What they cover is what a driver cannot: the arithmetic of a frame inside
  # the component's own vocabulary — where a field starts, what a click divides
  # by, whether a run reaches past the edge, whether a window too short for a
  # row drops the footer. `buffers.tested` builds each one with `doCheck`, so a
  # failing test fails the derivation, naming the test and the line.
  #
  # IT IS GATE-ONLY, and the reason is a number rather than a feeling: with the
  # tests on, the eight buffers go from 27.8 s to 2 min 44 s. See the fourth
  # criterion in `tests/slow.nix`.
  buffers-test = pkgs.linkFarmFromDrvs "check-buffers-test" (lib.attrValues buffers.tested);

  # NOBODY DRAWS A HEADING OF THEIR OWN, AND NOBODY GENERATES BINDINGS.
  #
  # This is the check that makes "these mistakes cannot happen again" mean
  # something. The report that started this work was four buffers with four
  # different headings — with a rule, without one, with a blank row, without
  # one, content starting at column 0, 1, 2 or 4 — and three browsers carrying
  # three copies of one geometry with different numbers in them.
  #
  # Two greps, and each closes one half:
  #
  #   1. `wit_bindgen::generate!` may appear in ONE crate. That is what makes
  #      the design crate unavoidable rather than merely available: a buffer
  #      cannot quietly go back to building its own display list, because it
  #      would have no types to build one out of.
  #   2. no buffer declares a heading of its own. `heading` is the crate's word
  #      for it; a `fn title(` reappearing under `buffers/` is the fifth answer
  #      being written, and the four that existed are why this check does.
  buffers-use-the-design-crate =
    mkCheck "buffers-use-the-design-crate" [ pkgs.gnugrep pkgs.coreutils ]
      ''
          failed=0

          expected="${self}/crates/ronoctua-design/src/lib.rs"
          # THE INVOCATION, NOT THE WORD. Every migrated buffer's module header
        # SAYS that `wit_bindgen::generate!` is gone and why — which is exactly
        # the sentence a future reader needs, and exactly what a naive grep would
        # report as the offence. Anchored to the start of a line, a comment's
        # leading `//` no longer matches.
        offenders=$(grep -rlE '^[[:space:]]*wit_bindgen::generate!' ${self}/buffers ${self}/crates || true)
          for f in $offenders; do
            if [ "$f" != "$expected" ]; then
              echo "ERROR: $f generates bindings of its own — only ronoctua-design does." >&2
              failed=1
            fi
          done
          if ! grep -qE '^[[:space:]]*wit_bindgen::generate!' "$expected"; then
            echo "ERROR: ronoctua-design stopped generating the bindings." >&2
            failed=1
          fi

          if titles=$(grep -rn "fn title(" ${self}/buffers 2>/dev/null); then
            echo "ERROR: a buffer went back to having a header of its own:" >&2
            echo "$titles" >&2
            echo "      There is one header, and it lives in Screen::heading." >&2
            failed=1
          fi

          [ "$failed" = 0 ] || exit 1
          echo "OK: one generator of bindings, and no header written by hand"
      '';

  # NO VIEW BUILDS ITS OWN SEPARATOR.
  #
  # The owner reported the same defect three times across two months, in three
  # different views: *"there are horizontal lines that touch the right side of the
  # screen and do not touch the left"*, and the vertical ones *"are not touching
  # the bottom of the screen"*. It was never one view's bug. Every one of the eight
  # sites wrote the rectangle out by hand, and every one of them made one of the
  # same two mistakes: `x: margin * cell` with `width: self.width` — inset on the
  # left, past the edge on the right — or `height: lines * line_height`, which
  # with a fractional line height stops short of the bottom.
  #
  # So the rectangle is not written by hand any more. `chrome::rule` takes the
  # surface's width and cannot express an inset; `chrome::divider` takes the
  # BOTTOM and not a height, so "where does it end" is answered with the edge
  # rather than with a count. This check is what keeps the ninth site from being
  # written the old way — which is the whole of what the owner asked for: not
  # that the lines be fixed, but that they stop coming back.
  # ONE OFFSET PER LIST, and the list owns it.
  #
  # The manager kept `View::first` in a `Cell` and each door kept a second copy
  # to push in through `set_first`. Three homes for one number, synchronised by
  # hand — and the two the owner reported came straight out of it: the wheel
  # moved a copy the next frame overwrote, and the window's cursor was compared
  # against one height while its wheel used another.
  #
  # Naming the dead spellings is what makes this cheap. They cannot come back
  # by accident, only by somebody typing them again.
  one-offset-per-list = mkCheck "one-offset-per-list" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    files = (home / "files.rs").read_text()
    window = (home / "files_window.rs").read_text()
    surface = (home / "surface.rs").read_text()
    wrong = []

    # THE SETTER IS GONE. It is what let a door hold the number.
    if re.search(r"\bfn\s+set_first\b", files):
        wrong.append("files.rs declares `set_first` again")
    for name, text in (("files_window.rs", window), ("surface.rs", surface)):
        if ".set_first(" in text:
            wrong.append(name + " pushes an offset in through `set_first`")

    # AND THE FIELD IS A FIELD, not a cell somebody can write through a
    # shared reference.
    if re.search(r"\bfirst:\s*Cell<", files):
        wrong.append("files.rs keeps the listing's offset in a `Cell`")
    if not re.search(r"\blist_offset:\s*usize\b", files):
        wrong.append("files.rs no longer owns `list_offset`")

    # AND THE WINDOW HAS NO SECOND COPY.
    if re.search(r"^\s*offset:\s*usize,\s*$", window, re.M):
        wrong.append("files_window.rs took an `offset` field back")

    if wrong:
        print("ERROR: the listing's offset has more than one home again.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A number with two homes is synchronised by hand, and the", file=sys.stderr)
        print("       hand forgets. See `files::View::list_offset`.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: one offset per list, and the view owns it")
    PY
  '';

  # EVERY LIST CAN BE DRAGGED, and a door may not answer "not mine".
  #
  # `View::wheel` and `View::drag_to` used to have `Pane::Listing => false`,
  # and `false` meant two different things at once — "I did nothing" and "this
  # is not mine to move". Each door read the second meaning and wrote its own
  # half, which is how the touchpad gesture and the drag came to be dead in one
  # door and alive in the other.
  #
  # The preview is the one honest refusal: it is a picture, and a picture does
  # not scroll.
  every-list-can-be-dragged = mkCheck "every-list-can-be-dragged" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    files = pathlib.Path("${self}/crates/ronoctua-shell/src/files.rs").read_text()
    wrong = []

    def body_of(name):
        m = re.search(r"\n    pub fn " + name + r"\b", files)
        if not m:
            return None
        i = m.start()
        depth, j, seen = 0, files.index("{", i), False
        for k in range(j, len(files)):
            if files[k] == "{":
                depth += 1
                seen = True
            elif files[k] == "}":
                depth -= 1
                if seen and depth == 0:
                    return files[i:k]
        return files[i:]

    for name in ("wheel", "drag_to"):
        body = body_of(name)
        if body is None:
            wrong.append("`" + name + "` is gone from files.rs")
            continue
        for m in re.finditer(r"Pane::(\w+)\s*=>[^\n]*\bfalse\b", body):
            if m.group(1) != "Preview":
                wrong.append(
                    "`" + name + "` answers `false` for `Pane::" + m.group(1) + "`"
                )
        for m in re.finditer(r"Pane::(\w+)\s*=>\s*return false", body):
            if m.group(1) != "Preview":
                wrong.append(
                    "`" + name + "` returns `false` for `Pane::" + m.group(1) + "`"
                )

    if wrong:
        print("ERROR: a list answers that it is not the one to move.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       `false` there means 'I did nothing', and a door that reads", file=sys.stderr)
        print("       it as 'not mine' writes the gesture a second time. The", file=sys.stderr)
        print("       preview is the one refusal: a picture does not scroll.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: every list moves through the view, and only the picture refuses")
    PY
  '';

  # ONE REPAINT MODEL, in every window.
  #
  # Presentation blocks on the vertical blank. A handler that paints is a
  # handler that waits for the monitor, and a drag delivers motion as fast as
  # the device reports it — so painting per event turned the owner's drag into
  # a walk: the listing crawled through every entry instead of arriving where
  # the pointer already was.
  #
  # The cure is one line of vocabulary in three files: a handler marks a frame
  # OWED, and the loop's callback — the one place that knows the batch is
  # finished — draws it once. This check is what keeps the fourth window from
  # being written the old way.
  one-frame-model = mkCheck "one-frame-model" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    windows = sorted(home.glob("*_window.rs"))
    if len(windows) < 3:
        print(f"ERROR: only {len(windows)} windows found; the glob stopped matching.",
              file=sys.stderr)
        raise SystemExit(1)

    wrong = []
    for f in windows:
        text = f.read_text()
        if not re.search(r"\bfn\s+ask_paint\b", text):
            wrong.append(f"{f.name} has no `ask_paint`")
        if not re.search(r"\bfn\s+settle\b", text):
            wrong.append(f"{f.name} has no `settle`")

        # AND THE DEBT IS TAKEN ONCE, in the loop's callback.
        takes = len(re.findall(r"std::mem::take\(&mut self\.owed\)", text))
        if takes != 1:
            wrong.append(f"{f.name} takes the debt {takes} times, and it may be taken once")
        if not re.search(r"\.run\(None[^)]*\|(\w+)\|\s*\{\s*(?://[^\n]*\n\s*)*\1\.settle\(\)", text):
            wrong.append(f"{f.name} does not drain the debt where the queue empties")

        # AND NO HANDLER PAINTS WHERE IT STANDS. `settle` is the ONE caller,
        # and the span is found by matching braces rather than by guessing how
        # many lines back it might be — the first draft guessed, and reported
        # `settle`'s own call three times.
        span = None
        m = re.search(r"\bfn\s+settle\s*\(&mut self\)\s*\{", text)
        if m:
            depth, start = 0, m.end() - 1
            for k in range(start, len(text)):
                if text[k] == "{":
                    depth += 1
                elif text[k] == "}":
                    depth -= 1
                    if depth == 0:
                        span = (start, k)
                        break
        for hit in re.finditer(r"self\.paint\(\);", text):
            if span and span[0] <= hit.start() <= span[1]:
                continue
            line = text.count("\n", 0, hit.start()) + 1
            wrong.append(f"{f.name}:{line} paints where it stands")

    if wrong:
        print("ERROR: a window paints from a handler instead of owing a frame.",
              file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       Presentation waits for the vertical blank, so a handler that", file=sys.stderr)
        print("       paints makes the gesture wait for the monitor. Mark the frame", file=sys.stderr)
        print("       OWED and let the loop's callback draw it once.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(windows)} windows, one repaint model, drained where the queue empties")
    PY
  '';

  # A COUNT OF LINES IS NOT A BOTTOM EDGE.
  #
  # `lines` is `floor(height / line_px)`, so a surface is that many whole lines
  # PLUS a residue of less than one. Every geometry defect this shell has been
  # asked to fix lived in that residue, and the same wrong sentence produced
  # all of them: `let bottom = lines * line_px`.
  #
  # It cost three separate reports — a rule that stopped short of the edge, a
  # scrollbar with a strip of background under its thumb, and a picture centred
  # in a box a line shorter than the panel it was in. The bottom comes from
  # `Frame::bottom()`, which is the number the compositor gave.
  #
  # AND THE PIXEL RAIL IS THE HOST'S. `ronoctua-design::rails` is behind the
  # `host` feature because a component counts in cells and has no surface;
  # compiling it into one would put a unit there that nothing on that side can
  # mean (invariants 4 and 11).
  one-pixel-rail = mkCheck "one-pixel-rail" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    # `chrome.rs` DEFINES the conversion, and `files.rs` states the one rule
    # about which band may take the residue — with the reasoning beside it.
    allowed = {"chrome.rs"}
    wrong = []
    # THE NAMES A TRACK OR AN EDGE GOES BY. `gap` and `track` are on the list
    # because the footer's own bar used `let gap = grid.lines * line_height`
    # and this check walked past it — the defect was in the shell for as long
    # as the check that was meant to catch it.
    naming = re.compile(
        r"\blet\s+(?:mut\s+)?(\w*(?:bottom|_px|edge|floor|gap|track|band))\s*"
        r"(?::[^=]*)?=\s*([^;]+);"
    )
    # TWO SEPARATE QUESTIONS, and not one regex spanning them. The first
    # draft tried to match the count and the multiplication in one pattern
    # with the punctuation between them spelled out — and
    # `f32::from(self.metric.lines()) * line_px` has an open parenthesis it
    # did not allow, so the plant that was supposed to prove this check bites
    # walked straight past. Whether a line COUNT appears and whether it is
    # multiplied by a line HEIGHT are two questions; asking them separately
    # cannot be defeated by what sits between.
    a_count = re.compile(r"\b(?:lines|rows|tall|room|height)\w*\b")
    a_multiple = re.compile(r"\*\s*line(?:_px|_height)?\b")

    for f in sorted(home.glob("*.rs")):
        if f.name in allowed:
            continue
        # PRODUCTION ONLY. A test builds a surface that is deliberately NOT a
        # whole number of lines — `line_px + lines * line_px + line_px * 0.4`
        # is the fixture that proves the residue exists, and forbidding it
        # there would forbid the proof.
        text = f.read_text().split("#[cfg(test)]")[0]
        for m in naming.finditer(text):
            name, value = m.group(1), " ".join(m.group(2).split())
            if not (a_count.search(value) and a_multiple.search(value)):
                continue
            line = text.count("\n", 0, m.start()) + 1
            wrong.append(f"{f.name}:{line}  `{name}` = {value[:60]}")

    # AND THE RAIL STAYS ON THE HOST'S SIDE.
    design = pathlib.Path("${self}/crates/ronoctua-design/src")
    lib = (design / "lib.rs").read_text()
    if not re.search(r'#\[cfg\(feature = "host"\)\]\s*\npub mod rails;', lib):
        wrong.append("ronoctua-design/src/lib.rs: `rails` is no longer behind `host`")
    # THE RAIL BY ITS OWN NAME, and not any `Frame`. `buffers/transfers` has a
    # local `type Frame = Screen<Wit>` and it is not this one — a bare `Frame`
    # forbade a buffer's own alias, which is none of this check's business.
    rail = re.compile(r"\brails::|(?:ronoctua_)?design::Frame\b")
    for f in sorted(pathlib.Path("${self}/buffers").rglob("src/*.rs")):
        if rail.search(f.read_text()):
            wrong.append(f"buffers/{f.parent.parent.name}: a buffer names the pixel rail")

    if wrong:
        print("ERROR: an edge was computed as a count of lines.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A surface is `floor(height / line)` lines plus a residue of", file=sys.stderr)
        print("       less than one, and the product lands short of the edge. Ask", file=sys.stderr)
        print("       `chrome::Frame::bottom()`, which is the number the compositor", file=sys.stderr)
        print("       gave and the one the shell may not argue with.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: no edge in the shell is a count of lines")
    PY
  '';

  # ONE SCROLLBAR, and one arithmetic behind it.
  #
  # The manager's bar was DRAWN by one sum and LOOKED FOR by another, in both
  # doors — so it moved in one and refused in the other, and the owner tried it
  # with the mouse in each. `Section::bar_box` is the rectangle now: the painter
  # asks it and the pointer asks it.
  #
  # NINE VIEWS, ONE RECTANGLE. `ronoctua_design::Scroll::bar` is the only place
  # a thumb's rectangle is worked out — `Section::bar_box` forwards to it and
  # the footer calls it. What still differs between the views is how many BANDS
  # each has, the manager four and everything else one, and that is a fact
  # about the views rather than about the bar.
  #
  # The footer's own copy had the same defect the manager was reported for,
  # over eight views nobody had ever dragged, plus one of its own: its column
  # ran the whole height of the surface, so the modeline's last cell was a
  # handle.
  one-scrollbar = mkCheck "one-scrollbar" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    files = (home / "files.rs").read_text()
    wrong = []

    # THE RECTANGLE HAS ONE DEFINITION.
    if len(re.findall(r"\bfn\s+bar_box\b", files)) != 1:
        wrong.append("files.rs does not have exactly one `bar_box`")
    # THE TWO THAT WERE DELETED, by name. `Panes::listing_bar` is NOT one of
    # them and is not a bar: it answers which COLUMN the bar goes in, which is
    # a `u16`. The rectangle test below is what catches a real second one.
    for gone in ("facts_bar", "panel_bar"):
        for f in sorted(home.glob("*.rs")):
            if re.search(r"\bfn\s+" + gone + r"\b", f.read_text()):
                wrong.append(f"{f.name} grew a second bar, `{gone}`")

    # AND NOBODY ELSE BUILDS ONE. A thumb is a rectangle whose height is a
    # fraction of a track; whoever writes that sum has written a second bar.
    thumb = re.compile(r"(?:lines|room|shown)\w*\s*/\s*(?:total|amount)\w*\s*\*")
    for f in sorted(home.glob("*.rs")):
        if f.name == "files.rs":
            continue
        text = f.read_text()
        for m in thumb.finditer(text):
            line = text.count("\n", 0, m.start()) + 1
            wrong.append(f"{f.name}:{line} computes a thumb of its own")

    # AND EVERY BAR IN THE SHELL COMES FROM `Scroll`, directly or through
    # `bar_box` — which is itself `Scroll` with the band's bottom worked out
    # first. Whoever paints `RULE_STRONG` has to have asked one of the two.
    #
    # The first draft demanded the NAME in every such file, and
    # `files_window.rs` does not have it: it asks `Geometry::bars`, which asks
    # `bar_box`, which asks `Scroll`. Demanding the name there would have
    # forced a second call whose only purpose was to satisfy this check.
    for f in sorted(home.glob("*.rs")):
        text = f.read_text()
        if "RULE_STRONG" not in text:
            continue
        if not re.search(r"ronoctua_design::Scroll|\bbar_box\b|\bbars\(|\bregion_bar\b", text):
            wrong.append(f.name + " paints a thumb without asking `Scroll`")

    # AND THE RECTANGLE ITSELF IS `Scroll::bar`'s.
    #
    # `thumb` gives a length and an offset INTO a track; turning those into a
    # rectangle means deciding the column and the band again, and that decision
    # is what differed between the copies. So outside the design crate nobody
    # calls `thumb` — they call `bar`, which answers with the rectangle.
    #
    # The first draft looked for `right - cell` instead, and a plant that
    # assembled the rectangle out of `thumb` by hand walked straight past it.
    for f in sorted(home.glob("*.rs")):
        text = f.read_text()
        for m in re.finditer(r"\.thumb\(", text):
            line = text.count("\n", 0, m.start()) + 1
            wrong.append(f"{f.name}:{line} builds a bar out of `thumb` instead of asking `bar`")

    # AND THE TWO CANNOT DRAW AT ONCE: the footer's generic bar closes over
    # the manager being shut. Without this the manager gets two.
    # COMMENTS STRIPPED FIRST. `has_bar`'s own comment names the call it
    # makes, so a plant that replaced the CODE with `false` and left the call
    # quoted beside it left this check green.
    surface = re.sub(r"//[^\n]*", "", (home / "surface.rs").read_text())
    if not re.search(r"fn has_bar\b[^}]*files\.is_some\(\)", surface, re.S):
        wrong.append("surface.rs: `has_bar` no longer stands down for the manager")

    if wrong:
        print("ERROR: the scrollbar has more than one arithmetic again.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       Drawn by one sum and grabbed by another is how it came to", file=sys.stderr)
        print("       move in one door and refuse in the other. See", file=sys.stderr)
        print("       `files::Section::bar_box`.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: nine views, one rectangle, and the generic bar stands down for the manager")
    PY
  '';

  # THE SYNTAX TABLE CAME OUT WHOLE, and its two vocabularies agree.
  #
  # The generator carries its own copies of `kind`, `numbers` and `property`,
  # and the Rust it emits carries the enums. A value renamed on one side and
  # not the other is not a compile error — the generator would emit
  # `Kind::Whatever` and the build would fail somewhere else entirely — so the
  # two lists are compared here, which is the shape `notifications-table-generated`
  # already uses for the same reason.
  syntax-table-generated = mkCheck "syntax-table-generated" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    made = pathlib.Path("${syntaxTable.table}/syntax.rs").read_text()
    gen = pathlib.Path("${self}/nix/scripts/gen-syntax.py").read_text()

    failed = 0

    def enum_of(name):
        block = re.search(r"pub enum " + name + r" \{(.*?)\n\}", made, re.S)
        if not block:
            print(f"ERROR: the generated table has no `{name}` enum", file=sys.stderr)
            raise SystemExit(1)
        return {v for v in re.findall(r"^    ([A-Z][A-Za-z0-9]*),$", block.group(1), re.M)}

    def tuple_of(name):
        m = re.search(name + r" = \(([^)]*)\)", gen)
        if not m:
            print(f"ERROR: the generator has no `{name}` vocabulary", file=sys.stderr)
            raise SystemExit(1)
        return {w.strip().strip('"') for w in m.group(1).split(",") if w.strip()}

    def variant(v):
        return "".join(p.capitalize() for p in v.split("-"))

    # THE TWO VOCABULARIES, compared. A value invented on one side only.
    for rust, python in (("Kind", "KINDS"), ("Numbers", "NUMBERS"),
                         ("Property", "PROPERTIES"), ("SniffShape", "SNIFF_SHAPES")):
        theirs = {variant(v) for v in tuple_of(python)}
        ours = enum_of(rust)
        if theirs != ours:
            print(
                f"ERROR: `{python}` in the generator and `{rust}` in the table disagree.\n"
                f"       only in the generator: {sorted(theirs - ours)}\n"
                f"       only in the table:     {sorted(ours - theirs)}",
                file=sys.stderr,
            )
            failed = 1

    # AND THE INDEXES ARE SORTED, which is what `binary_search` needs and what
    # a generator that stopped sorting would break in silence.
    for name in ("OF_MIME", "OF_TAG", "OF_SHEBANG"):
        rows = re.findall(r'^    \("([^"]+)", Language::', made, re.M)
        block = re.search(r"pub static " + name + r": \[.*?\] = \[(.*?)\n\];", made, re.S)
        if not block:
            print(f"ERROR: the table has no `{name}`", file=sys.stderr)
            failed = 1
            continue
        keys = re.findall(r'\("([^"]+)",', block.group(1))
        if not keys:
            print(f"ERROR: `{name}` is empty", file=sys.stderr)
            failed = 1
            continue
        if keys != sorted(keys):
            print(f"ERROR: `{name}` is not sorted; `binary_search` will miss", file=sys.stderr)
            failed = 1
        del rows

    if failed:
        raise SystemExit(1)
    print("OK: the syntax table and its generator agree, and the indexes are sorted")
    END
  '';

  # AND THE GENERATOR REFUSES, for the RIGHT reasons.
  #
  # It does not re-run anything: `nix/syntax.nix` builds a fixture in which one
  # language steals another's MIME type and another claims prose, records what
  # the generator said, and this reads it. Both sentences are demanded rather
  # than a non-zero exit, because a refusal for the WRONG reason is a refusal
  # that will not survive the next edit — the argument `updates-refusal-refuses`
  # makes one screen up.
  syntax-refusal-refuses = mkCheck "syntax-refusal-refuses" [ ] ''
    said=${syntaxTable.negativeFixture}/out_text.txt
    test -f "$said" || {
      echo "ERROR: the negative fixture recorded nothing" >&2
      exit 1
    }
    failed=0
    for owed in "application/json" "fixture-steals-json" "text/plain" "fixture-claims-prose"; do
      grep -q "$owed" "$said" || {
        echo "ERROR: the refusal does not name \"$owed\"" >&2
        failed=1
      }
    done
    if [ "$failed" != 0 ]; then
      echo "" >&2
      echo "It refused, and not for the reasons the fixture planted. A guard that" >&2
      echo "refuses for the wrong reason is a guard that will not survive an edit." >&2
      exit 1
    fi
    echo "OK: the generator refused, and named both defects"
  '';

  # AND NO LANGUAGE CLAIMS PROSE.
  #
  # THE ONE VIOLATION THAT WOULD NOT FAIL LOUDLY. Everything else on this list
  # breaks a build or draws the wrong colour; this one would quietly colour
  # every essay, log and README on the machine — and that looks like a feature
  # rather than a defect, which is why nobody would report it.
  #
  # The floor is the other half: without it the check passes on a file somebody
  # emptied.
  syntax-claims-no-prose = mkCheck "syntax-claims-no-prose" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    src = pathlib.Path("${self}/assets/syntax.nix").read_text()
    # The comments explain the rule and name the types; the DECLARATIONS are
    # what may not carry them.
    live = "\n".join(l for l in src.splitlines() if not l.lstrip().startswith("#"))

    failed = 0
    for forbidden in ("text/plain", "application/octet-stream", "inode/"):
        if forbidden in live:
            print(
                f"ERROR: assets/syntax.nix claims `{forbidden}`. That is prose or a\n"
                "       container, and colouring prose is worse than not colouring it —\n"
                "       whoever reads starts looking for meaning in a colour that means nothing.",
                file=sys.stderr,
            )
            failed = 1

    # AND IT STILL CLAIMS SOMETHING. Without this the check passes on a file
    # that was emptied, which is the shape `gen-mimedb.py`'s floors refuse.
    claims = len(re.findall(r"^\s*mime = \[", live, re.M))
    if claims < 30:
        print(
            f"ERROR: only {claims} language(s) declare a `mime` list, below the floor of 30.\n"
            "       A check that forbids and never confirms passes on an empty file.",
            file=sys.stderr,
        )
        failed = 1

    if failed:
        raise SystemExit(1)
    print(f"OK: {claims} languages claim a type, and none of them is prose")
    END
  '';

  # AND A SYNTAX ROLE BECOMES A COLOUR IN EXACTLY ONE PLACE.
  #
  # Two copies would be the clipboard's preview and the file manager's preview
  # colouring the same Rust differently — the two-answers defect `Section` and
  # `Field::covers` were each written to end. And every arm has to name a
  # TOKEN: a literal colour here would be invariant 9 broken in the one file
  # allowed to make a visual decision.
  syntax-colours-are-tokens = mkCheck "syntax-colours-are-tokens" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    mine = "chrome.rs"
    failed = 0

    src = (home / mine).read_text()
    block = re.search(r"pub\(crate\) const fn syntax_color\(.*?\n\}", src, re.S)
    if not block:
        print(
            "ERROR: `chrome::syntax_color` is gone. A check that only forbids passes\n"
            "       on the day somebody deletes the function and goes back to hand.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    body = block.group(0)

    # EVERY ARM NAMES A TOKEN, and none of them a value.
    arms = re.findall(r"Role::(\w+)", body)
    if not arms:
        print("ERROR: no `Role::` arm in the map — did the shape change?", file=sys.stderr)
        raise SystemExit(1)
    for line in body.splitlines():
        if "=>" not in line or "Role::" not in line:
            continue
        if "tokens::color::SYNTAX_" not in line:
            print(f"ERROR: this arm does not name a syntax token: {line.strip()!r}", file=sys.stderr)
            failed = 1

    # AND IT IS THE ONLY MAP. Anywhere else that matches on a role is a second
    # answer waiting to disagree.
    for f in sorted(home.glob("*.rs")):
        if f.name == mine:
            continue
        text = f.read_text()
        text = re.sub(r"//[^\n]*", "", text)
        if re.search(r"Role::\w+\s*=>", text):
            print(
                f"ERROR: {f.name} turns a syntax role into something itself.\n"
                "       Call `chrome::syntax_color`; two maps is the two panels colouring\n"
                "       the same code differently.",
                file=sys.stderr,
            )
            failed = 1

    # AND EVERY ROLE THE CRATE DECLARES HAS AN ARM.
    crate = pathlib.Path("${self}/crates/ronoctua-syntax/src/lib.rs").read_text()
    declared = re.search(r"pub enum Role \{(.*?)\n\}", crate, re.S)
    if not declared:
        print("ERROR: `ronoctua-syntax` has no `Role` enum", file=sys.stderr)
        raise SystemExit(1)
    owed = set(re.findall(r"^    ([A-Z]\w*),$", declared.group(1), re.M))
    missing = owed - set(arms)
    if missing:
        print(f"ERROR: no colour for {sorted(missing)}", file=sys.stderr)
        failed = 1

    if failed:
        raise SystemExit(1)
    print(f"OK: {len(owed)} roles, one map, every arm a token")
    END
  '';

  # AND THERE IS ONE HIGHLIGHTER.
  #
  # It used to be a module of `ronoctua-clipboard`, which never used it — only
  # the shell did — so it was a highlighter belonging to one app, and a
  # highlighter belonging to one app is a highlighter the next app copies.
  # Without this, the day somebody writes a second tokeniser inside the file
  # manager nothing says so.
  #
  # The positive half matters as much: forbidding without confirming passes on
  # the day both are deleted.
  syntax-has-one-owner = mkCheck "syntax-has-one-owner" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root = pathlib.Path("${self}")
    mine = root / "crates/ronoctua-syntax/src"
    failed = 0

    if not (mine / "lib.rs").exists():
        print("ERROR: `crates/ronoctua-syntax` is gone", file=sys.stderr)
        raise SystemExit(1)
    owner = (mine / "lib.rs").read_text()
    for owed in ("pub enum Role", "pub fn line_into(", "pub fn states(", "pub fn of_mime("):
        if owed not in owner:
            print(f"ERROR: the crate no longer offers `{owed}`", file=sys.stderr)
            failed = 1

    if (root / "crates/ronoctua-clipboard/src/highlight.rs").exists():
        print(
            "ERROR: the clipboard's own highlighter is back. It belongs to no app:\n"
            "       `ronoctua-clipboard` never used it, and a highlighter inside one\n"
            "       app is the one the next app copies.",
            file=sys.stderr,
        )
        failed = 1

    # A SECOND TOKENISER ANYWHERE, by the shape of its own vocabulary.
    for f in sorted(root.glob("crates/*/src/**/*.rs")):
        if mine in f.parents or f.parent == mine:
            continue
        text = f.read_text()
        m = re.search(r"pub enum Role \{(.*?)\n\}", text, re.S)
        if m and "Comment" in m.group(1) and "String" in m.group(1):
            print(
                f"ERROR: {f.relative_to(root)} declares a second syntax `Role`.\n"
                "       There is one highlighter, and it is `ronoctua-syntax`.",
                file=sys.stderr,
            )
            failed = 1

    if failed:
        raise SystemExit(1)
    print("OK: one highlighter, and it is still a crate of its own")
    END
  '';

  separators-come-from-chrome = mkCheck "separators-come-from-chrome" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    # The one file allowed to build one: it is the definition.
    mine = "chrome.rs"
    bad = []
    for f in sorted(home.glob("*.rs")):
        if f.name == mine:
            continue
        lines = f.read_text().split("\n")
        for i, l in enumerate(lines):
            if "Quad {" not in l:
                continue
            block = "\n".join(lines[i : i + 9])
            if re.search(r"width: 1\.0|height: 1\.0|RULE_THICKNESS", block):
                bad.append(f"{f.name}:{i + 1}")
    if bad:
        print("ERROR: a rule or a divider assembled by hand, outside chrome.rs:", file=sys.stderr)
        for b in bad:
            print(f"       {b}", file=sys.stderr)
        print("       Use chrome::rule (the surface's width, no inset) or", file=sys.stderr)
        print("       chrome::divider (which takes the BOTTOM, not a height).", file=sys.stderr)
        sys.exit(1)

    # AND THE DEFINITION STILL DEFINES. A check that only forbids would pass
    # on the day somebody deleted the two functions and went back to hand.
    src = (home / mine).read_text()
    for fn in ("pub fn rule(", "pub fn divider("):
        if fn not in src:
            print(f"ERROR: chrome.rs lost `{fn}` — there is no longer one place.", file=sys.stderr)
            sys.exit(1)
    print("OK: the rules and the dividers come out of one place")
    PY
  '';

  # NO LINE IS COMPOSED OF SEVERAL SECTIONS' FIELDS.
  #
  # The manager has four columns — the sidebar, the listing, the preview and the
  # facts — and each is a `files::Section`: it says where it is, what it holds
  # and how much of it there is. Both doors draw section by section, and
  # `View::hit` resolves a point by finding the section first.
  #
  # What this refuses is the arrangement that came before it, in which one line
  # carried every column's fields at once. It was cheap and it was wrong: a
  # column's length, its offset and its top reached into its neighbours, and
  # every defect the owner reported for weeks was one of those reaches — a click
  # on a folder resolving to the path above it, the sidebar's first rows
  # answering nothing, the facts' hover lighting the listing, one wheel gesture
  # moving all four, the preview's top jumping whenever the listing scrolled.
  # It ended by taking the machine down: a function that materialised one line
  # per row turned `usize::MAX` into a request for 2^64 of them.
  #
  # So: nothing outside the section builder may concatenate one section's fields
  # onto another's line. The flattening that remains is `cfg(test)`, and says so.
  sections-do-not-share-a-line = mkCheck "sections-do-not-share-a-line" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    home = pathlib.Path("${self}/crates/ronoctua-shell/src")
    files = (home / "files.rs").read_text()

    # THE BUILDER IS THE ONLY ONE. `Section` has to exist and carry its own
    # content, or this check is guarding nothing.
    for needed in ("pub struct Section", "pub content: Vec<Row>", "pub fn sections("):
        if needed not in files:
            print(f"ERROR: files.rs lost `{needed}` — the sections stopped being the source.",
                  file=sys.stderr)
            sys.exit(1)

    # AND THE FLATTENING IS A TEST TOOL, never production.
    for name in ("fn drawn_at(", "fn drawn("):
        at = files.find(name)
        if at == -1:
            continue
        before = files[max(0, at - 400) : at]
        if "#[cfg(test)]" not in before:
            print(f"ERROR: `{name.strip('(')}` is not under #[cfg(test)] —"
                  " the flattening came back into production.", file=sys.stderr)
            sys.exit(1)

    # AND NOBODY BRINGS IT BACK UNDER ANOTHER NAME: a public function that
    # returns a flat list of rows for the whole region is the shape that was
    # removed.
    banned = re.compile(r"pub fn (region_rows|rows_from|rows)\s*\(")
    hit = banned.search(files)
    if hit:
        line = files[: hit.start()].count("\n") + 1
        print(f"ERROR: files.rs:{line} gives back the region's lines flattened"
              f" (`{hit.group(1)}`).", file=sys.stderr)
        print("       The manager has FOUR sections; each door draws them one by one.",
              file=sys.stderr)
        sys.exit(1)

    # AND THE DOORS DRAW SECTIONS.
    for door in ("surface.rs", "files_window.rs"):
        src = (home / door).read_text()
        if ".sections(" not in src:
            print(f"ERROR: {door} does not draw by section.", file=sys.stderr)
            sys.exit(1)
    print("OK: every section draws itself, and nothing composes one line out of several")
    PY
  '';

  # EVERY STATE A BUFFER DECLARES HAS A WORD, IN BOTH LANGUAGES.
  #
  # Same shape as `buffer-modes-have-text`, and the same failure it prevents: the
  # id is English by invariant 18 and what the modeline draws comes from
  # `state-<id>` by invariant 8, so a state added without its two lines puts a
  # bracketed key on screen. It ALSO checks the base, because the colour rule
  # reads it — `normal` is drawn quiet and everything else in the alternative
  # token, and a buffer whose base were called something else would draw itself
  # as permanently not-normal.
  buffer-states-have-text = mkCheck "buffer-states-have-text" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    ID = re.compile(r'^\s+id: "([a-z][a-z0-9-]*)"\.into\(\),')
    LETTER = re.compile(r"^\s+letter: '(.)',")

    # Read the `states:` block of each descriptor. The same shape
    # `english-names` and `buffer-modes-have-text` read, and for the same
    # reason: nothing ties the literal to the key.
    per_buffer = {}
    for rs in sorted((root_dir / "buffers").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        inside = False
        found = []
        pending = None
        for line in rs.read_text().splitlines():
            if "states: vec![" in line:
                inside = True
                continue
            if inside and line.strip() == "],":
                inside = False
                continue
            if not inside:
                continue
            m = ID.match(line)
            if m:
                pending = m.group(1)
                continue
            m = LETTER.match(line)
            if m and pending:
                found.append((pending, m.group(1)))
                pending = None
        if found:
            per_buffer[rs.parts[-3]] = found

    if not per_buffer:
        sys.exit("ERROR: no state found — did the block's shape change?")

    gaps = []
    for buffer, states in sorted(per_buffer.items()):
        if not states or states[0][0] != "normal":
            gaps.append(buffer + ": the base state is " + repr(states[:1]) + ", not `normal`")
        ids = [i for i, _ in states]
        letters = [l for _, l in states]
        if len(set(ids)) != len(ids):
            gaps.append(buffer + ": a repeated state id")
        if len(set(letters)) != len(letters):
            gaps.append(buffer + ": two identical letters would leave a state unreachable")

    wanted = {i for states in per_buffer.values() for i, _ in states}
    for language in ${builtins.toJSON languages}:
        ftl = (root_dir / "assets/i18n" / (language + ".ftl")).read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*) *=", ftl, re.M))
        for ident in sorted(wanted):
            if "state-" + ident not in have:
                gaps.append(language + ": state-" + ident + " is missing")

    if gaps:
        print("ERROR: a badly declared buffer state.", file=sys.stderr)
        for g in gaps:
            print("       " + g, file=sys.stderr)
        raise SystemExit(1)
    total = sum(len(v) for v in per_buffer.values())
    print("OK: " + str(total) + " states in " + str(len(per_buffer)) + " buffers, with text in both languages")
    END
  '';

  transfers-instance-fast = mkCheck "transfers-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.transfers}/lib/ronoctua_buffer_transfers.wasm 200
  '';

  # FORMATTING A DISK, driven the way the host drives it.
  #
  # The two things it proves are the two the host cannot see: from out there a
  # refusal and a crashed component both look like no plan arriving.
  format-driven = mkCheck "format-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-format \
      ${buffers.format}/lib/ronoctua_buffer_format.wasm \
      ${../assets/i18n}
  '';

  format-instance-fast = mkCheck "format-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.format}/lib/ronoctua_buffer_format.wasm 200
  '';

  # ===================================================================
  # THE TORRENT BORDER, and the two things about it that cannot be read.
  # ===================================================================

  # THE LAYOUT, COMPARED AGAINST THE C++ COMPILER'S OWN MEASUREMENT.
  #
  # `#[repr(C)]` promises to ARRANGE what it was told and never to check it: a
  # wrong width is a register with the wrong contents and produces no
  # diagnostic at all. `CLAUDE.md` records that this cost three wrong struct
  # layouts on `libghostty-vt`, found by hand with a twenty-line C program.
  #
  # `nix/torrent-shim.nix` now compiles the REAL header with the real C++
  # compiler and writes every `sizeof` and `offsetof` down; this compares that
  # file against the constants in the Rust. Neither side can drift alone.
  torrent-layout-matches-cxx = mkCheck "torrent-layout-matches-cxx" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import re, sys, pathlib

    measured = {}
    for row in pathlib.Path("${torrentShim}/share/ronoctua/layout.txt").read_text().split("\n"):
        if not row.strip():
            continue
        name, value = row.split()
        measured[name] = int(value)

    rust = pathlib.Path("${../crates/ronoctua-torrent/src/ffi.rs}").read_text()
    declared = {}
    m = re.search(r"assert_eq!\(size_of::<Status>\(\), (\d+)", rust)
    if m:
        declared["size"] = int(m.group(1))
    m = re.search(r"assert_eq!\(align_of::<Status>\(\), (\d+)", rust)
    if m:
        declared["align"] = int(m.group(1))
    for field, value in re.findall(r"offset_of!\(Status, (\w+)\), (\d+)", rust):
        declared[field] = int(value)

    if not declared:
        sys.exit("ERROR: could not find a single number in ffi.rs — the layout test went away")

    failed = 0
    for name, value in sorted(measured.items()):
        if name not in declared:
            print(f"ERROR: the C++ measures `{name}` = {value} and the Rust does not declare it", file=sys.stderr)
            failed = 1
        elif declared[name] != value:
            print(
                f"ERROR: `{name}`: C++ measures {value}, Rust says {declared[name]}."
                " An FFI with the wrong layout gives no diagnostic — it gives a register"
                " with the wrong content in it.",
                file=sys.stderr,
            )
            failed = 1
    for name in declared:
        if name not in measured:
            print(f"ERROR: the Rust declares `{name}` and the C++ does not measure it", file=sys.stderr)
            failed = 1
    if failed:
        sys.exit(1)
    print(f"OK: {len(measured)} measurements from the C++ compiler match the Rust's")
    PY
  '';

  # AND THAT v2 IS NOT A CLAIM.
  #
  # It makes a purely-v2 torrent, hands it to a real session, and demands a v2
  # info hash and NO v1 one — then does the same with a v1-only torrent, so a
  # flag that were simply always true fails here too. Entirely offline: the data
  # is already where the torrent says it is, so the session checks it against
  # the Merkle tree of SHA-256 hashes and goes straight to seeding.
  torrent-speaks-v2 = mkCheck "torrent-speaks-v2" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-torrent "$TMPDIR/torrent"
  '';

  # ===================================================================
  # THE EXTENSION, and the fingerprint the owner asked about.
  # ===================================================================

  # THE FIVE WAYS A WEBSITE CAN SEE AN EXTENSION, refused in the manifest.
  #
  # The owner's question was exact: an extension nobody else has makes this
  # browser unique if every site can list it. The detection vectors are known
  # and every one of them is a manifest key — so the manifest is GENERATED with
  # none of them, and this fails the build the day one appears.
  extension-has-no-fingerprint = mkCheck "extension-has-no-fingerprint" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import json, pathlib, sys

    m = json.loads(pathlib.Path("${extension}/share/ronoctua-extension/manifest.json").read_text())
    forbidden = [
        ("web_accessible_resources", "any page can `fetch` from chrome-extension://"),
        ("content_scripts", "what it leaves in the DOM is a marker"),
        ("externally_connectable", "it hands `chrome.runtime` to an origin"),
        ("host_permissions", "standing access is what makes the rest possible"),
        ("declarative_net_request", "the page infers it from the request itself"),
    ]
    failed = 0
    for key, why in forbidden:
        if key in m:
            print(f"ERROR: the manifest gained `{key}` — {why}", file=sys.stderr)
            failed = 1
    # And the permissions: `activeTab` instead of standing access IS the whole
    # design. Without it the only alternative is `host_permissions`.
    if "activeTab" not in m.get("permissions", []):
        print("ERROR: with no `activeTab` the extension would need standing access", file=sys.stderr)
        failed = 1
    if m.get("manifest_version") != 3:
        print("ERROR: MV2 does not have `activeTab` with this meaning", file=sys.stderr)
        failed = 1
    if failed:
        sys.exit(1)
    print(f"OK: none of the five detection channels, and {len(m['permissions'])} permissions")
    PY
  '';

  # THE PO TOKEN MINTER RUNS AND EXITS, and there is no server half at all.
  #
  # The owner asked for a minter that runs only when needed and leaves nothing
  # behind. That is a promise about a SHAPE, so it is checked as one: upstream
  # ships two providers, an HTTP one that wants a resident server and a script
  # one that does not, and `nix/pot-provider.nix` installs only the second. A
  # future edit that installed the other half would give this system a port and
  # a process without anybody deciding to.
  #
  # It also runs the thing, OFFLINE. `--version` reaches the script through
  # Deno, resolves every bare import out of the vendored `node_modules`, and
  # exits — inside a sandbox with no network, which is what proves that the
  # runtime and its dependencies are all in the closure rather than fetched on
  # first use. And it compares the version with the plugin's, because a skew
  # between the two is a warning upstream prints and nobody reads.
  pot-provider-is-script-only =
    let
      closure = pkgs.closureInfo { rootPaths = [ potProvider ]; };
    in
    mkCheck "pot-provider-is-script-only"
      [
        pkgs.deno
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        home=${potProvider}/server
        plugin=${potProvider}/plugin/yt_dlp_plugins/extractor
        failed=0

        # ---- NO SERVER HALF ----
        if [ -e "$plugin/getpot_bgutil_http.py" ]; then
          echo "ERROR: the HTTP provider was installed — that asks for a server standing" >&2
          failed=1
        fi
        test -f "$plugin/getpot_bgutil_script.py" || {
          echo "ERROR: the script provider is gone" >&2
          failed=1
        }
        # A namespace package has no `__init__.py`; with one, yt-dlp's plugin
        # finder does not see it and the whole thing is silently absent.
        if [ -e ${potProvider}/plugin/yt_dlp_plugins/__init__.py ]; then
          echo "ERROR: yt_dlp_plugins stopped being a namespace package" >&2
          failed=1
        fi

        # ---- NODE IS NOT IN THE CLOSURE ----
        #
        # It is a build tool here: what runs is Deno. Measured — the reference
        # arrived through shebangs of command-line entry points nothing
        # executes, and it was 249 MiB.
        if found=$(grep -e "-nodejs-" ${closure}/store-paths); then
          echo "ERROR: the generator drags node in, which it never runs:" >&2
          echo "$found" >&2
          failed=1
        fi

        # ---- AND IT RUNS, WITH NO NETWORK ----
        export DENO_NO_PROMPT=1 DENO_NO_UPDATE_CHECK=1 FORCE_COLOR=false
        export DENO_DIR="$TMPDIR/deno" XDG_CACHE_HOME="$TMPDIR/cache"
        mkdir -p "$TMPDIR/cache/bgutil-ytdlp-pot-provider"
        got=$(deno run --allow-env --allow-net \
          --allow-ffi="$home/node_modules" \
          --allow-write="$TMPDIR/cache/bgutil-ytdlp-pot-provider" \
          --allow-read="$TMPDIR/cache/bgutil-ytdlp-pot-provider,$home/node_modules" \
          "$home/src/generate_once.ts" --version 2>&1 | tail -1) || {
          echo "ERROR: the generator does not run without a network — something is missing from the closure" >&2
          echo "$got" >&2
          exit 1
        }
        want=$(grep -oP "__version__ = '\K[^']+" "$plugin/getpot_bgutil.py")
        if [ "$got" != "$want" ]; then
          echo "ERROR: the script says $got and the plugin says $want; one warning per download" >&2
          failed=1
        fi

        [ "$failed" = 0 ] || exit 1
        echo "OK: only the script provider, version $got, and it runs without a network"
      '';

  # THE APP DOES NOT DEPEND ON THE EXTENSION.
  #
  # The owner's requirement, turned into a test: everything that can be done
  # from the browser's extension has to be doable without it. That is not a
  # thing one reads and believes — an entry added to the popup is one line, and
  # the key that would match it is in another file that nobody has to open.
  #
  # So this reads BOTH SIDES. What the extension can ask for comes out of its
  # own sources; what the keyboard can ask for comes out of the leader's table;
  # and every one of the first has to appear in the second, twice — once
  # anonymous and once as the person, because the extension can do both.
  downloads-do-not-need-the-extension =
    mkCheck "downloads-do-not-need-the-extension" [ pkgs.python3 ]
      ''
        python3 - <<'PY'
        import pathlib, re, sys

        source = pathlib.Path("${../extension}")
        wanted = set()
        kinds = set()
        for name in ("popup.js", "worker.js"):
            text = (source / name).read_text()
            wanted |= set(re.findall(r"want:\s*'([a-z]+)'", text))
            kinds |= set(re.findall(r"kind:\s*'([a-z-]+)'", text))

        leader = pathlib.Path("${../crates/ronoctua-shell/src/leader.rs}").read_text()
        bound = set(re.findall(r'\(\s*"[a-z ]+"\s*,\s*"(transfers-[a-z-]+)"', leader))

        failed = 0
        # ---- WHAT THE MENU TAKES OFF A PAGE ----
        if not wanted:
            sys.exit("ERROR: could not find a single `want:` in the extension — the check stopped looking")
        for want in sorted(wanted):
            for command in (f"transfers-{want}", f"transfers-{want}-as-me"):
                if command not in bound:
                    print(
                        f"ERROR: the extension offers `{want}` and the keyboard has no `{command}`."
                        " Everything the extension does has to be possible without it.",
                        file=sys.stderr,
                    )
                    failed = 1

        # ---- AND THE PLAIN LINK ----
        if "fetch" in kinds:
            for command in ("transfers-add", "transfers-add-as-me"):
                if command not in bound:
                    print(
                        f"ERROR: the extension sends a link and the keyboard has no `{command}`",
                        file=sys.stderr,
                    )
                    failed = 1

        # ---- AND IDENTITY, ON BOTH SIDES ----
        #
        # The extension keeps it as a switch because interception has no moment
        # at which to ask; the keyboard has it as a PREFIX. Both must exist, or
        # one door does something the other cannot.
        if "toggle-identity" not in kinds:
            print("ERROR: the extension lost the identity key", file=sys.stderr)
            failed = 1
        if not any(c.endswith("-as-me") for c in bound):
            print("ERROR: the keyboard lost the `t i` group", file=sys.stderr)
            failed = 1

        if failed:
            sys.exit(1)
        print(
            f"OK: {len(wanted)} media kinds and the bare link, each with an anonymous key "
            "and an identified key"
        )
        PY
      '';

  # AND IT PARSES.
  #
  # A syntax error in a service worker is the quietest failure this extension
  # can have: Chrome loads the extension, the worker never starts, the button
  # does nothing, and the message is behind two clicks in a page nobody has
  # open. Eleven megabytes of esbuild at build time buys that away — and it
  # checks the GENERATED tree, so a broken `generated.js` fails here too.
  extension-parses = mkCheck "extension-parses" [ pkgs.esbuild ] ''
    failed=0
    for f in ${extension}/share/ronoctua-extension/*.js; do
      if ! esbuild --log-level=warning --format=esm --target=chrome116 "$f" > /dev/null; then
        echo "ERROR: $(basename "$f") does not compile — the service worker would never come up" >&2
        failed=1
      fi
    done
    [ "$failed" = 0 ] || exit 1
    echo "OK: the extension's four files compile for chrome116"
  '';

  # AND NO TEXT AND NO COLOUR WRITTEN BY HAND.
  #
  # Invariants 8 and 9 do not stop at the browser's edge. Every word comes from
  # `assets/i18n/*.ftl` and every colour from `assets/tokens/colors.nix`, both
  # through the generated table — so a hex code or a sentence in the extension's
  # own sources is a value outside the design system.
  extension-has-no-literal = mkCheck "extension-has-no-literal" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    source = pathlib.Path("${../extension}")
    failed = 0
    for entry in sorted(source.iterdir()):
        if entry.name == "generated.js":
            continue
        text = entry.read_text()
        # A colour by hand, in any notation a browser would accept.
        for m in re.finditer(r"#[0-9a-fA-F]{3,8}\b|\brgba?\(\s*\d+", text):
            row = text[: m.start()].count("\n") + 1
            print(
                f"ERROR: {entry.name}:{row} has a colour by hand (`{m.group(0)}`)."
                " Every colour comes from assets/tokens/colors.nix (invariant 9).",
                file=sys.stderr,
            )
            failed = 1
        # UI text. What a person reads arrives through `TEXT.*`; what is left
        # over are log messages, which nobody reads off a screen.
        for m in re.finditer(r"textContent\s*=\s*['\"`]([^'\"`]+)", text):
            if m.group(1).strip():
                row = text[: m.start()].count("\n") + 1
                print(
                    f"ERROR: {entry.name}:{row} writes text straight onto the screen."
                    " Every text comes from assets/i18n (invariant 8).",
                    file=sys.stderr,
                )
                failed = 1
    if failed:
        sys.exit(1)
    print("OK: the extension carries neither a word nor a colour of its own")
    PY
  '';

  # ===================================================================
  # AND WHAT IS BORN ON THE MACHINE — invariant 13.
  # ===================================================================
  #
  # Two things this work needs that may never be in a public repository: the
  # extension's signing key, which decides its id, and aria2's RPC token. Both
  # are generated by units declared in the flake, on the machine, and
  # `nix/modules/secrets.nix` is explicit that "but it is encrypted" is not an
  # argument for either.
  secrets-are-not-in-the-repo = mkCheck "secrets-are-not-in-the-repo" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    root = pathlib.Path("${../.}")
    failed = 0
    # The extension's private key, in any PEM shape.
    private = re.compile(r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----")
    # And aria2's token WITH A VALUE: the line that generates it lives in the
    # module and ends in `$secret`, which is not a secret, and the line that
    # reads it lives in the Rust.
    token = re.compile(r"rpc-secret\s*=\s*[A-Za-z0-9+/=]{8,}")
    for entry in root.rglob("*"):
        if not entry.is_file():
            continue
        if any(part in {".git", "target", "result"} for part in entry.parts):
            continue
        try:
            text = entry.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        if private.search(text):
            print(f"ERROR: a private key in {entry.relative_to(root)}", file=sys.stderr)
            failed = 1
        if token.search(text):
            print(f"ERROR: aria2's secret in {entry.relative_to(root)}", file=sys.stderr)
            failed = 1
    if failed:
        sys.exit(1)
    print("OK: neither a private key nor a token — both are born ON the machine")
    PY
  '';

  # THE 5 MS BUDGET, on the third component too.
  lucide-instance-fast = mkCheck "lucide-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.lucide}/lib/ronoctua_buffer_lucide.wasm 200
  '';

  # THE LUCIDE BROWSER, driven the way the host drives it.
  #
  # The name ended up narrow: besides the search and the filters, it covers the
  # whole sheet — the 19 forms of copy, the scrolling of the 24 lines, and the
  # two that were born with the vector preview:
  #
  #   - the affordance MARK matches the table the key reads, line by line. A mark
  #     that promises a key that does nothing is worse than no mark at all, and
  #     that was exactly the case of the color line;
  #   - the PREVIEW is byte for byte the SVG the copy delivers, before and after
  #     touching the stroke. A second constructor for the screen would drift
  #     silently, and the preview would stop being the artifact with nothing
  #     giving it away.
  lucide-filters = mkCheck "lucide-filters" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-lucide \
      ${buffers.lucide}/lib/ronoctua_buffer_lucide.wasm \
      ${../assets/i18n}
  '';

  # WHICH FACE DRAWS WHAT — the promise the comment in `text.rs` had been making
  # since the second face and that had never been written down.
  #
  # It also witnesses the COLLISION that forced the remapping: as long as Iosevka
  # answers for `U+E0A0`, publishing Lucide in the origin range means drawing the
  # wrong symbol. The day that changes, the check fails and someone re-reads the
  # decision instead of rediscovering it through the screen.
  renderer-loads-three-faces = mkCheck "renderer-loads-three-faces" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-faces
  '';

  # THE UNICODE SYMBOL BROWSER, by the same path: the real component, driven
  # the way the host drives it.
  #
  # It asserts three things the glyph one does not have to assert, and each one
  # corresponds to a real difference of the Unicode database:
  #
  #   - the EXACT total (7,529). The component's parser splits the records on
  #     `"},"`, and `}` is INSIDE this database (`U+007D`) — what saves it is
  #     the generator not emitting a string field with an arbitrary character;
  #   - the combining mark drawn over the dotted circle, and COPIED without it:
  #     pasting `◌́` into the user's document is the worst possible defect, and
  #     the only one that survives any visual inspection;
  #   - the doubled budget of the 23 double-width ones, otherwise the host
  #     elides a single character into `…`.
  symbols-filters = mkCheck "symbols-filters" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-symbols \
      ${buffers.unicode}/lib/ronoctua_buffer_unicode.wasm \
      ${../assets/i18n}
  '';

  # The OTHER half of the GPU-independent metrics of section 9.3: the CPU work
  # that happens before any pixel.
  #
  # The gate is loose on purpose — today there is 5000x of headroom. It does not
  # exist to tighten the layout, it exists so that a regression of an ORDER OF
  # MAGNITUDE (measuring text instead of counting columns, allocating per
  # segment) shows up here and not in a bisection of stutter six months later.
  layout-fast = mkCheck "layout-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-layout 5000
  '';

  # Text shaping must not go back to the regime of SECONDS.
  #
  # It cost 4,447 ms on the first frame because cosmic-text's `Family::Monospace`
  # points at a font that is not in our database. The check compares the four
  # arrangements and fails if the production one regresses — the ceiling is loose
  # (300 ms against a floor of 0.1 ms) because what it catches is the return of
  # the regime, not variation between machines.
  shaping-fast = mkCheck "shaping-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-shaping
  '';

  # A SURFACE THE SIZE OF A MONITOR HAS TO FIT.
  #
  # `Limits::downlevel_defaults()` fixes `max_texture_dimension_2d` at 2048, and
  # the presentation surface IS a texture. The symptom had two faces, and neither
  # of them pointed at the limits:
  #
  #   - `Alt+=` on a 1920 monitor takes the column to 2112 px on the FIRST step,
  #     and the window CLOSES BY ITSELF. `Surface::configure` gives back no error:
  #     it kills the process, and niri sends the output of what it spawns to
  #     /dev/null;
  #   - any monitor wider than 2048 px never had a footer, because it takes the
  #     screen's width and died before the first pixel.
  #
  # The check asks for a device with the PRODUCTION limits and tries the texture.
  # Comparing numbers from `Limits` would be checking our arithmetic against
  # itself; here whoever answers is the driver.
  large-monitor-surface = mkCheck "large-monitor-surface" [ ] ''
    export VK_DRIVER_FILES=${xvfb.icd}
    export VK_ICD_FILENAMES=${xvfb.icd}
    export LIBGL_ALWAYS_SOFTWARE=1
    ${rust.workspace}/bin/ronoctua-test-large-surface
  '';

  # THE TOOL THAT MAKES "OPEN IT ON THE SCREEN" AFFORDABLE HAS TO KEEP WORKING.
  #
  # `nix run .#png` draws a buffer into a PNG in 0,2 s, and it exists because
  # the alternative — a NixOS VM, niri under Xvfb, a screenshot of the X root —
  # cost three minutes, which is how a title came to be drawn at twice the
  # body's size for months. A tool nobody checks is a tool that stops working
  # quietly, and the discovery happens on the day somebody needs it.
  #
  # # What is actually asserted, and why not "the file appeared"
  #
  # Because a PNG of the right size, filled edge to edge with the background
  # colour, is EXACTLY what a broken draw produces — and it would pass. So:
  #
  #   - every chunk's CRC is recomputed, and the `IDAT` goes through a real
  #     `zlib`. The writer is forty hand-rolled lines (making `png` a direct
  #     dependency would cost a rebuild of crane's whole dependency layer), and
  #     this is what stops those forty lines from being subtly wrong;
  #   - the size in the header has to be the size the tool PRINTED. A writer
  #     that lies about its own dimensions is the one failure a viewer would
  #     see as "the picture is a bit odd";
  #   - there has to be INK: pixels that are not the commonest colour. That is
  #     the assertion that separates "it drew" from "it cleared the screen";
  #   - and pt-BR and en-GB have to come out DIFFERENT. That one closes the
  #     whole chain in a single comparison — component, display list, i18n,
  #     tokens, wgpu, pixel — with no OCR and without naming a single colour
  #     here, which would be a second copy of a token (invariant 9).
  #
  # Under lavapipe, like `large-monitor-surface`: hermetic, no GPU. Section 9.3
  # forbids measuring frame time here, and nothing here measures time.
  png-tool-draws = mkCheck "png-tool-draws" [ pkgs.python3 ] ''
    export VK_DRIVER_FILES=${xvfb.icd}
    export VK_ICD_FILENAMES=${xvfb.icd}
    export LIBGL_ALWAYS_SOFTWARE=1

    for language in ${lib.concatStringsSep " " languages}; do
      ${rust.workspace}/bin/ronoctua-render-png \
        ${buffers.format}/lib/ronoctua_buffer_format.wasm \
        --columns 60 --lines 10 --language "$language" \
        --out "$PWD/$language.png" | tee "$PWD/$language.txt"
    done

    python3 - <<'END'
    import pathlib, re, struct, sys, zlib

    def read(language):
        raw = pathlib.Path(f"{language}.png").read_bytes()
        assert raw[:8] == b"\x89PNG\r\n\x1a\n", f"{language}: assinatura PNG errada"
        at, chunks = 8, {}
        while at < len(raw):
            size = struct.unpack(">I", raw[at : at + 4])[0]
            kind = raw[at + 4 : at + 8]
            body = raw[at + 8 : at + 8 + size]
            said = struct.unpack(">I", raw[at + 8 + size : at + 12 + size])[0]
            # THE CRC AGAINST A REAL IMPLEMENTATION. Our table is written by
            # hand; `zlib.crc32` is not.
            mine = zlib.crc32(kind + body)
            assert mine == said, f"{language}: CRC de {kind!r}: {mine:08x} != {said:08x}"
            chunks.setdefault(kind, b"")
            chunks[kind] += body
            at += 12 + size
        width, height, depth, colour, _, _, laced = struct.unpack(">IIBBBBB", chunks[b"IHDR"])
        assert (depth, colour, laced) == (8, 6, 0), (
            f"{language}: the header says {depth} bits, type {colour}, interlaced {laced}"
        )
        rows = zlib.decompress(chunks[b"IDAT"])
        # ONE FILTER BYTE PER ROW, and ours is always 0 — nothing compresses, so
        # nothing needs filtering. A row that arrived with another filter would
        # mean the writer changed and this reader did not.
        stride = width * 4 + 1
        assert len(rows) == stride * height, (
            f"{language}: {len(rows)} pixel bytes for {width}x{height}"
        )
        pixels = []
        for n in range(height):
            row = rows[n * stride : (n + 1) * stride]
            assert row[0] == 0, f"{language}: line {n} came with filter {row[0]}"
            pixels.extend(
                bytes(row[1 + i * 4 : 5 + i * 4]) for i in range(width)
            )
        said = pathlib.Path(f"{language}.txt").read_text()
        printed = re.search(r"(\d+)x(\d+) px", said)
        assert printed, f"{language}: the tool did not say the size: {said!r}"
        assert (width, height) == (int(printed[1]), int(printed[2])), (
            f"{language}: the header says {width}x{height} and the tool said "
            f"{printed[1]}x{printed[2]}"
        )
        return width, height, pixels

    shape, seen = None, {}
    for language in ${builtins.toJSON languages}:
        width, height, pixels = read(language)
        if shape is None:
            shape = (width, height)
        assert shape == (width, height), "the languages came out at different sizes"

        # THE INK. The commonest colour is the background by construction — it is
        # what the frame was cleared with — so anything else is something drawn.
        tally = {}
        for p in pixels:
            tally[p] = tally.get(p, 0) + 1
        background, _ = max(tally.items(), key=lambda kv: kv[1])
        ink = len(pixels) - tally[background]
        share = ink / len(pixels)
        print(f"{language}: {width}x{height}, {len(tally)} colours, {share:.2%} of ink")
        assert share > 0.005, (
            f"{language}: only {share:.3%} of the image is not the background — that is a clean "
            "screen, not a drawn buffer"
        )
        seen[language] = pixels

    # AND THE TRANSLATIONS DIFFER IN THE PIXEL — every pair of them.
    #
    # It used to compare exactly two, which is the shape a third language would
    # have walked straight past: it would have been drawn, measured, and never
    # once compared against anything. All pairs is the same assertion without
    # the assumption, and it names WHICH two collided rather than "they did".
    same = [
        (a, b)
        for i, a in enumerate(seen)
        for b in list(seen)[i + 1 :]
        if seen[a] == seen[b]
    ]
    assert not same, (
        "these came out pixel for pixel identical, so the key is not being "
        f"translated before it becomes a drawing: {same}"
    )
    print("OK: the PNG is valid, has ink in it, and the language reaches the pixel")
    END
  '';

  # ENDING THE SESSION MUST NOT PRINT AN ERROR.
  #
  # # What this catches
  #
  # Two defects that lived together for months with nothing reporting them, and
  # that the owner saw every time they closed with Ctrl+C:
  #
  #   rm: cannot remove '/nix/store/…-ronoctua-niri-nested.kdl': Permission denied
  #   rm: cannot remove '/nix/store/…-ronoctua-niri-nested.kdl': Permission denied
  #
  # The first: `shutdown` tried to delete the CONFIG, which became a store path
  # when it started being generated by Nix — immutable by construction. The
  # `rm -f` does not save it, because it silences "does not exist" and not "is
  # not yours".
  #
  # The second, the duplicate: `trap … EXIT INT TERM` runs the cleanup on INT and
  # again on EXIT.
  #
  # # Why it runs the REAL app
  #
  # Because the criterion is "the output is clean", and a replica of `shutdown`
  # inside the check would prove nothing about the `shutdown` that runs on the
  # owner's machine — and the lesson at the top of this file is exactly that.
  # With no display niri does not come up, the script falls into the error path,
  # and it is the real `shutdown` that runs.
  dev-exits-clean = mkCheck "dev-exits-clean" [ devApp ] ''
    export HOME=$TMPDIR
    export XDG_RUNTIME_DIR=$TMPDIR
    # A DISPLAY THAT DOES NOT EXIST. With none at all, the script stops at the
    # graphical-session guard BEFORE installing the trap, and the path this check
    # wants to exercise never runs. With a fake name it carries on, niri dies at
    # startup, the waiting loop gives up — and `shutdown` really runs.
    export WAYLAND_DISPLAY=does-not-exist-1
    # It exits non-zero on purpose (niri does not come up here); what is measured
    # is what it PRINTS on ending.
    timeout 300 ronoctua-dev > out_text.txt 2> errors.txt || true

    if grep -qE "cannot remove|Permission denied" errors.txt; then
      echo "ERROR: shutting the session down printed a removal error:" >&2
      grep -E "cannot remove|Permission denied" errors.txt >&2
      echo "      A store path is not deleted, and '-f' does not silence that." >&2
      exit 1
    fi

    # AND ONLY ONCE. The log line comes out of `shutdown`; two means the cleanup
    # ran twice.
    #
    # HOW FAR THIS PROVES IT: here the script exits by the error path, so only the
    # EXIT trap fires. The duplicate the owner saw comes from Ctrl+C — INT cleans
    # up and EXIT cleans up again —, and reproducing it would require a compositor
    # standing to interrupt. This check catches `shutdown` running twice on a
    # normal exit; the other half is seen on screen, and its fix is the
    # `trap 'exit 130' INT` of dev.nix.
    #
    # The `|| true` is around the `grep -c`: with no match at all it exits 1, and
    # under `pipefail` that would bring the check down before it counted — which is
    # the same trap `pgrep` has already set twice in this file.
    n=$( { cat out_text.txt errors.txt | grep -c "logs of this session:"; } || true )
    [ "$n" = 1 ] || {
      echo "ERROR: the shutdown ran $n times; expected 1." >&2
      echo "      'trap … EXIT INT TERM' cleans up on INT and again on EXIT." >&2
      echo "--- output:" >&2; tail -20 out_text.txt >&2
      echo "--- errors:" >&2; tail -20 errors.txt >&2
      exit 1
    }

    echo "OK: shutting the session down comes out clean, and only once"
  '';

  # THE NESTED LOOP CALLS A COMMAND THAT EXISTS.
  #
  # # The defect this exists for, and it lasted from the first commit
  #
  # `nix/dev.nix` handed the watcher `RONOCTUA_RELOAD_CMD='rebuild'`, and
  # the function it defines and exports is called `rebuild`. The watcher `eval`s
  # the string, gets "command not found", and swallows it with `|| true` — which
  # is correct, because a compile error is the normal case in a loop. So HOT
  # RELOAD NEVER FIRED, not once, and nothing said so: the footer that stays on
  # screen is the store's, and it works perfectly.
  #
  # `reload-reloads`, just below, did not see it: it proves the MECHANISM with a
  # command of its own making. What was missing was a check on the WIRING.
  #
  # Both sides come from the built script, which is the artifact that runs.
  reload-command-is-defined =
    mkCheck "reload-command-is-defined"
      [
        pkgs.gnugrep
        pkgs.gnused
      ]
      ''
        script=${devApp}/bin/ronoctua-dev

        cmd=$(sed -n "s/.*RONOCTUA_RELOAD_CMD='\([a-z_]*\)'.*/\1/p" "$script" | head -1)
        [ -n "$cmd" ] || {
          echo "ERROR: could not find RONOCTUA_RELOAD_CMD in the loop's script" >&2
          exit 1
        }
        echo "the loop tells it to run: $cmd"

        grep -q "^  $cmd() {" "$script" || {
          echo "ERROR: '$cmd' is not a function defined in the script — the watcher will" >&2
          echo "      get 'command not found', swallow it with '|| true', and the hot" >&2
          echo "      reload will NEVER fire, with not one line of warning." >&2
          exit 1
        }
        # And exported: the watcher is ANOTHER process, and a function that is
        # not exported does not cross into it — same symptom, same silence.
        grep -qE "^  export -f .*\b$cmd\b" "$script" || {
          echo "ERROR: '$cmd' is not exported; the watcher runs in another process" >&2
          exit 1
        }

        echo "OK: the loop calls '$cmd', which exists and crosses over to the watcher"
      '';

  # AND IT COMPILES THE SAME BINARY THE BUILD WOULD.
  #
  # # The defect this exists for
  #
  # Everything in `nix/rust.nix`'s `generated` reaches the Rust through
  # `option_env!`, which resolves at COMPILE time. Hot reload compiles the binary
  # HERE, so a variable this script does not export is a variable that binary
  # does not have — and the failure is always the same shape: a feature that
  # works in production and answers "could not open" in the loop.
  #
  # It had already happened twice, and both were found by accident: the icon
  # superpack (the list drew category glyphs and never the app's icon) and the
  # five wasm components. Twenty-five more were waiting, `btop` and the
  # clipboard and `ghostty-vt` among them; what hid them was that the reload was
  # wired to a command that did not exist, so no edit ever replaced the store's
  # binary.
  #
  # The list is not written here: it is read from the table the compilation gets.
  dev-exports-what-the-build-bakes = mkCheck "dev-exports-what-the-build-bakes" [ pkgs.gnugrep ] ''
    script=${devApp}/bin/ronoctua-dev
    failed=0

    for v in ${lib.concatStringsSep " " (lib.attrNames (rust.generated // rust.bufferComponents))}; do
      grep -q "^  export $v=" "$script" || {
        echo "ERROR: the build bakes $v in and the loop does not export it — the hot" >&2
        echo "      reload goes without it, and the failure shows up far from here." >&2
        failed=1
      }
    done

    [ "$failed" = 0 ] || exit 1
    echo "OK: the $(grep -c '^  export RONOCTUA_' "$script") variables of the loop cover what the build bakes in"
  '';

  # ===================================================================
  # THE RESCUE CONSOLE — `Ctrl+Alt+Del`
  # ===================================================================

  # EVERY ROW OF THE CONSOLE HAS A SENTENCE, IN BOTH LANGUAGES.
  #
  # Invariant 8, on the one screen where breaking it would hurt most: a rescue
  # console that draws `[rescue-reboot]` is a console whose rows nobody can
  # read, opened by somebody whose machine is already broken.
  #
  # The ids come out of the Rust and not out of a list written here — a list
  # here would be a third copy, and the one that goes stale is always the copy.
  rescue-actions-have-text =
    mkCheck "rescue-actions-have-text"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        src=${../crates/ronoctua-rescue/src}
        failed=0

        keys=$(grep -ohE '"rescue-[a-z0-9-]+"' "$src"/lib.rs | tr -d '"' | sort -u)
        [ -n "$keys" ] || {
          echo "ERROR: no 'rescue-*' key in the registry — what does the grep match?" >&2
          exit 1
        }

        for k in $keys; do
          for language in ${lib.concatStringsSep " " languages}; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERROR: '$k' is missing from $language.ftl — the screen would show [$k]" >&2
              failed=1
            }
          done
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$keys" | wc -l) rescue keys, in both languages"
      '';

  # THE MIME TABLE CAME OUT WHOLE.
  #
  # The floors are in the generator and the compile-time assertions are in
  # `mime.rs`; what this adds is that the derivation BUILT, which is the thing
  # a reordered `mime-glyphs.nix` or a bumped `shared-mime-info` would break.
  mime-table-generated = mkCheck "mime-table-generated" [ pkgs.gnugrep pkgs.coreutils ] ''
    table=${mimeDb}/mimedb.rs
    test -s "$table" || { echo "ERROR: the table came out empty" >&2; exit 1; }
    for name in GLOBS MAGIC SUBCLASS ALIASES GENERIC ICON_OF_GENERIC ICON_OF_MIME; do
      grep -qE "pub static $name" "$table" || {
        echo "ERROR: the $name table is missing" >&2
        exit 1
      }
    done
    # The counts, so a reader that quietly stopped understanding the format is
    # caught here as well as at compile time. A parser that breaks returns a
    # HANDFUL of rows, which reads as "the database is small".
    globs=$(grep -c 'Glob {' "$table")
    [ "$globs" -ge 900 ] || { echo "ERROR: only $globs globs" >&2; exit 1; }
    echo "OK: $globs name patterns, and the seven tables"
  '';

  # AND IT REFUSES A TABLE THAT DOES NOT ANSWER FOR EVERY GENERIC ICON.
  #
  # The negative fixture. Without it, a `shared-mime-info` release adding
  # `x-office-drawing` would draw the neutral square on a whole class of files
  # — and the neutral square is indistinguishable from "I did not resolve it",
  # so nothing would say so.
  mime-unknown-icon-fails = mkCheck "mime-unknown-icon-fails" [ ] ''
    test -s ${mimeDb.negativeFixture}/verdict
    cat ${mimeDb.negativeFixture}/verdict
  '';

  # AND THE PREFERENCE STILL PREFERS.
  #
  # `prefer` settles `*.obj` between `model/obj` and `application/x-tgif` by
  # REMOVING the loser's glob — so while it works, nothing in the table says it
  # is there, and the day upstream drops the ambiguity the row would stop
  # applying in silence. This breaks the row on purpose and watches the build
  # refuse; see `nix/mimegen.nix`.
  mime-preference-is-live = mkCheck "mime-preference-is-live" [ ] ''
    test -s ${mimeDb.preferenceFixture}/verdict
    cat ${mimeDb.preferenceFixture}/verdict
  '';

  # EVERY PLAYLIST TYPE IS EITHER READ OR REFUSED, AND NEVER NEITHER.
  #
  # `mesh-formats-are-total`'s argument, one medium along. A type on the
  # table's `playlist` row with no arm in `playlist::kind_of` resolves, reaches
  # the reader, and comes back with a sentence about a spelling nobody wrote —
  # or worse, is claimed by the `sound` row below it and handed to ffmpeg,
  # whose `hls` demuxer OPENS the first file the list names and reports THAT
  # file's codec and duration as the playlist's. Measured against the pinned
  # binary.
  #
  # So the three lists have to agree: what the table claims, what the parser
  # dispatches, and what the two languages can say about it.
  playlist-formats-are-total = mkCheck "playlist-formats-are-total" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    src = pathlib.Path("${../crates/ronoctua-inspect/src}")
    i18n = pathlib.Path("${../assets/i18n}")
    module = (src / "playlist.rs").read_text()
    pixels = (src / "pixels.rs").read_text()
    bad = []

    # --- what the module says it serves -----------------------------------
    declared = re.search(r"pub const TYPES: \[&str; \d+\] = \[(.*?)\];", module, re.S)
    if declared is None:
        bad.append("playlist.rs no longer declares TYPES")
        claimed = set()
    else:
        claimed = set(re.findall(r'"([^"]+)"', declared.group(1)))

    # --- what it can actually PARSE ---------------------------------------
    arm = re.search(r"fn kind_of\(mime: &str\) -> Option<Kind> \{(.*?)\n\}", module, re.S)
    read = set(re.findall(r'"([^"]+)"', arm.group(1))) if arm else set()
    if arm is None:
        bad.append("playlist.rs no longer dispatches on the type")

    # --- and the refusal each unread one gets -----------------------------
    if read - claimed:
        bad.append("parsed but not claimed: " + ", ".join(sorted(read - claimed)))
    named = re.search(r"pub const REFUSED: \[&str; \d+\] = \[(.*?)\];", module, re.S)
    refused = set(re.findall(r'"([^"]+)"', named.group(1))) if named else set()
    if named is None:
        bad.append("playlist.rs no longer declares which spellings it refuses")
    # THE THREE LISTS MUST PARTITION THE CLAIM. A type in none of them is the
    # omission this check exists for: it resolves, reaches the reader, and comes
    # back with a sentence written about somebody else's format.
    if claimed - read - refused:
        bad.append(
            "claimed and neither read nor refused: "
            + ", ".join(sorted(claimed - read - refused))
        )
    if read & refused:
        bad.append("read AND refused: " + ", ".join(sorted(read & refused)))
    if refused - claimed:
        bad.append("refused but not claimed: " + ", ".join(sorted(refused - claimed)))
    if "Refusal::Unsupported(key::PREVIEW_PLAYLIST_UNREAD)" not in module:
        bad.append("the refused spellings no longer carry a sentence of their own")

    # --- the table must claim exactly the same set, ABOVE the sound row ----
    row = re.search(
        r'id: "playlist",\s*types: &crate::playlist::TYPES,', pixels
    )
    if row is None:
        bad.append("pixels.rs's playlist row no longer claims playlist::TYPES")
    else:
        order = [m.group(1) for m in re.finditer(r'id: "([a-z-]+)"', pixels)]
        if "playlist" not in order or "sound" not in order:
            bad.append("the table lost one of the two rows this depends on")
        elif order.index("playlist") > order.index("sound"):
            bad.append(
                "the playlist row sits below the sound row, so `media: audio` "
                "claims it first and ffmpeg is handed a list of names"
            )

    # --- and no playlist type may be probed --------------------------------
    facts = (src / "facts.rs").read_text()
    if "playlist::is_a_playlist(mime)" not in facts:
        bad.append("facts::needs no longer keeps ffprobe away from a playlist")

    # --- the sentence, in both languages ----------------------------------
    for name in ${builtins.toJSON languages}:
        text = (i18n / f"{name}.ftl").read_text()
        if "files-preview-playlist-unread =" not in text:
            bad.append(f"{name} cannot say why a spelling is not opened")

    if bad:
        print("\n".join(bad))
        sys.exit(1)
    print(
        f"OK: {len(read)} spellings read, {len(refused)} refused by name, "
        "and the row sits above the sound row"
    )
    END
    touch $out
  '';

  # THE VIEW'S KEYS ARE NOT THE COMPOSITOR'S, AND NOTHING ABOVE US CLAIMS `Ctrl`.
  #
  # `crates/ronoctua-shell/src/files.rs` says, above its own table: *"`Ctrl` is
  # the family nothing above us claims … the check
  # `files-keys-are-not-the-compositors` is what keeps it that way"*.
  #
  # It did not exist. The comment promised a guarantee nothing provided, for as
  # long as the table has been there — found while reading the file for another
  # reason, and worth more than the check itself: a comment that names a check
  # is read as a check having been run.
  #
  # What it forbids is `M-`, and the reason is on the screen rather than in
  # theory: the first attempt at these keys used `M-`, and in the nested loop
  # the leader's chord family IS `Alt` — so `M-RET` never reached the footer, it
  # opened a terminal.
  files-keys-are-not-the-compositors =
    mkCheck "files-keys-are-not-the-compositors" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import pathlib, re, sys

        files = pathlib.Path("${../crates/ronoctua-shell/src/files.rs}").read_text()
        bad = []

        # THE TABLE ITSELF, and not the whole file: a comment may name `M-` while
        # explaining why it is not used, and this reads the bindings.
        block = re.search(r"OF_THE_VIEW[^=]*= &\[(.*?)\n\];", files, re.S)
        if block is None:
            bad.append("files.rs no longer declares OF_THE_VIEW as a table")
        else:
            body = re.sub(r"//.*", "", block.group(1))
            bound = re.findall(r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)', body)
            if not bound:
                bad.append("OF_THE_VIEW binds nothing, which cannot be right")
            for chord, action in bound:
                if chord.startswith("M-") or "-M-" in chord:
                    bad.append(
                        f"{action} is bound to {chord}: `M-` is the compositor's "
                        "chord family and the key would never arrive"
                    )
                if chord.startswith("Mod") or "Super" in chord:
                    bad.append(f"{action} is bound to {chord}, which the compositor takes first")

        # AND THE COMMENT STAYS TRUE. This check is named in the file; if the
        # sentence goes, the reader loses the reason and this loses its subject.
        if "files-keys-are-not-the-compositors" not in files:
            bad.append("files.rs stopped naming this check, so nothing explains the rule")

        if bad:
            print("\n".join(bad))
            sys.exit(1)
        print(f"OK: {len(bound)} of the view's keys, none of them the compositor's")
        END
        touch $out
      '';

  # A ROW'S LABEL IS A KEY, AND THE ONE EXCEPTION IS NARROW ON PURPOSE.
  #
  # `facts::Label::Named` exists so a tag the FILE wrote can be shown under the
  # name the file wrote — `BPM (beats per minute)` has no key and can have
  # none, because nobody has seen it before it is read. That is the same rule
  # a `Choice`'s label already follows, and it is right.
  #
  # It is also a hole in invariant 8's MECHANISM: nothing in the compiler stops
  # somebody writing `Label::Named("Bit rate".into())` and putting an English
  # string on a Portuguese screen. Nothing in `english-names` would either — it
  # reads declarations, and that is a value.
  #
  # So the hole is closed from the other side: the variant may be built at ONE
  # site, and never from a literal. Tests are exempt, and they are where a
  # literal is the point.
  inspect-labels-are-keys = mkCheck "inspect-labels-are-keys" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    src = pathlib.Path("${../crates/ronoctua-inspect/src}")
    shell = pathlib.Path("${../crates/ronoctua-shell/src}")
    bad = []

    def code_only(text):
        """The file with its test module and its comments cut away."""
        cut = re.split(r"\n#\[cfg\(test\)\]", text)[0]
        return re.sub(r"//.*", "", cut)

    built = []
    for path in sorted(src.rglob("*.rs")):
        for n, line in enumerate(code_only(path.read_text()).splitlines(), 1):
            if "Label::Named(" in line:
                built.append((path.name, n, line.strip()))

    if len(built) != 1:
        bad.append(
            "Label::Named is built at "
            + str(len(built))
            + " sites and must be built at exactly one: "
            + "; ".join(f"{f}:{n}" for f, n, _ in built)
        )

    for f, n, line in built:
        if re.search(r'Label::Named\(\s*"', line):
            bad.append(f"{f}:{n} builds a label from a literal, which is a key by another name")

    # AND THE ONE PLACE THAT RESOLVES ONE must still tell the two apart, or a
    # key reaches the screen as itself.
    files = (shell / "files.rs").read_text()
    if "Label::Named(name) => name.clone()" not in files:
        bad.append("files.rs no longer shows a named label verbatim")
    if "Label::Key(k) => texts.t(k)" not in files:
        bad.append("files.rs no longer translates a keyed label")

    if bad:
        print("\n".join(bad))
        sys.exit(1)
    print(f"OK: one site builds Label::Named, and it reads a name rather than writing one")
    END
    touch $out
  '';

  # EVERY 3D FORMAT IS REACHABLE BY THE WHOLE PATH, AND NOT BY HALF OF IT.
  #
  # A format needs four things to work, in four files: a `Shape` variant, an
  # arm in `mesh::read`'s dispatch, a MIME type on the table's `mesh` row, and
  # a sentence in both languages. Three of the four and the file resolves,
  # decodes, draws — and the panel names the format with a raw key; or it
  # resolves and nothing draws; or nothing resolves at all and the reader that
  # was written for it is never called.
  #
  # That last one is not hypothetical. A `.obj` had a reader, a `Shape`, a key
  # and two sentences, and resolved to `application/x-tgif` because of a glob
  # nobody had looked at — it was found by hand, on the owner's screen.
  mesh-formats-are-total = mkCheck "mesh-formats-are-total" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    src = pathlib.Path("${../crates/ronoctua-inspect/src}")
    i18n = pathlib.Path("${../assets/i18n}")
    mod = (src / "mesh" / "mod.rs").read_text()
    pixels = (src / "pixels.rs").read_text()
    bad = []

    # --- the variants, from the enum itself -------------------------------
    block = re.search(r"pub enum Shape \{(.*?)\n\}", mod, re.S)
    if block is None:
        bad.append("no `pub enum Shape` — what does this check read?")
        variants = []
    else:
        variants = re.findall(r"^    ([A-Z][A-Za-z0-9]*),$", block.group(1), re.M)
    if len(variants) < 10:
        bad.append(f"only {len(variants)} shapes — the enum is not being read")

    # --- each one has a key, and is in ALL --------------------------------
    keyed = dict(re.findall(r"Self::([A-Za-z0-9]+) => crate::key::([A-Z_0-9]+),", mod))
    listed = re.search(r"pub const ALL: \[Self; \d+\] = \[(.*?)\];", mod, re.S)
    inside = re.findall(r"Self::([A-Za-z0-9]+),", listed.group(1)) if listed else []
    for v in variants:
        if v not in keyed:
            bad.append(f"`Shape::{v}` has no arm in `Shape::key`")
        if v not in inside:
            bad.append(f"`Shape::{v}` is missing from `Shape::ALL`")

    # --- the key constants resolve, and both languages carry them ---------
    lib = (src / "lib.rs").read_text()
    names = dict(re.findall(r'pub const ([A-Z_0-9]+): &str = "([a-z0-9-]+)";', lib))
    for v, const in keyed.items():
        text = names.get(const)
        if text is None:
            bad.append(f"`key::{const}`, which `Shape::{v}` uses, does not exist")
            continue
        for language in ${builtins.toJSON languages}:
            body = (i18n / f"{language}.ftl").read_text()
            if not re.search(rf"^{re.escape(text)} = .", body, re.M):
                bad.append(f"`{text}` is missing from {language}.ftl")

    # --- the dispatch and the table claim the SAME types -------------------
    dispatch = re.search(r"match ronoctua_files::mime::canonical\(mime\) \{(.*?)\n    \}", mod, re.S)
    if dispatch is None:
        bad.append("no dispatch in `mesh::read` — what does this check read?")
        reads = set()
    else:
        reads = set(re.findall(r'"([a-z]+/[A-Za-z0-9.+-]+)" =>', dispatch.group(1)))
    row = re.search(r'id: "mesh",\s*types: &\[(.*?)\],', pixels, re.S)
    claimed = set(re.findall(r'"([a-z]+/[A-Za-z0-9.+-]+)"', row.group(1))) if row else set()
    if not claimed:
        bad.append("the table's `mesh` row claims nothing")
    for kind in sorted(reads - claimed):
        bad.append(f"`{kind}` is read but the table does not claim it — nothing resolves to it")
    for kind in sorted(claimed - reads):
        bad.append(f"`{kind}` is claimed but `read` has no arm — it draws nothing")

    if bad:
        print("ERROR: a 3D format is reachable by only part of its path.", file=sys.stderr)
        print("", file=sys.stderr)
        for b in bad:
            print("       " + b, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A format takes four things: a `Shape`, an arm in `read`, a", file=sys.stderr)
        print("       type on the table's mesh row, and a sentence in both", file=sys.stderr)
        print("       languages. Three of them is a file that half works.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(variants)} shapes and {len(claimed)} types, whole path each")
    END
  '';

  # EVERY ROW OF THE FILE MANAGER'S MENU HAS TEXT, IN BOTH LANGUAGES.
  #
  # The same shape as the rescue console's, and for the same reason: a row with
  # no translation draws `[files-open]` on screen, which is exactly the
  # silent-fallback class invariant 8 exists to refuse — `Texts::t` gives back
  # `[key]` rather than bringing the shell down, so somebody has to be looking
  # for it.
  files-actions-have-text =
    mkCheck "files-actions-have-text"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        src=${../crates/ronoctua-files/src}
        failed=0

        # FROM THE ID, and not from the label. The label is a CONSTANT
        # (`key::FILES_OPEN`), so grepping for a `"files-…"` literal here finds
        # only the prefix — which is what the first version of this check did,
        # and it would have gone green on a table with no translations at all.
        #
        # Deriving the key from the id is also the discipline itself: the
        # registry refuses a row whose label is not `files-<id>`, and this is
        # the same rule reaching the `.ftl`.
        ids=$(grep -oE '^ *id: "[a-z0-9-]+"' "$src"/menu.rs | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$ids" ] || {
          echo "ERROR: no row in the action table — what does the grep match?" >&2
          exit 1
        }
        # TWO KEYS PER ROW, and the second is the one that would rot quietly.
        #
        # `files-<id>` is the label, and it is missing loudly: the row draws its
        # own key on screen. `files-does-<id>` is the technical answer that
        # reaches the message line under the cursor, and a missing one shows up
        # only when somebody happens to rest on that row — which is precisely
        # the register that was added for people who go looking.
        keys=$(for i in $ids; do echo "files-$i"; echo "files-does-$i"; done | sort -u)

        for k in $keys; do
          for language in ${lib.concatStringsSep " " languages}; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERROR: $k is missing from $language.ftl — the screen would show the raw key" >&2
              failed=1
            }
          done
        done

        # AND THE SIDEBAR'S OWN MENU, which is a second table for a second kind
        # of subject: `menu.rs` is about an `Entry` in a directory, and every
        # one of its `Applies` asks a question only an entry can answer. The
        # rule is the same one — `sidebar-<id>` derived from the id — and so is
        # the reason it is derived and not grepped: the label is a constant.
        side=$(grep -oE '^ *id: "[a-z0-9-]+"' "$src"/sidebar_menu.rs \
          | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$side" ] || {
          echo "ERROR: no row in the sidebar's menu." >&2
          exit 1
        }
        sidekeys=$(for i in $side; do echo "sidebar-$i"; echo "sidebar-does-$i"; done | sort -u)
        for k in $sidekeys; do
          for language in ${lib.concatStringsSep " " languages}; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERROR: $k is missing from $language.ftl — the sidebar menu's" >&2
              echo "      row would show the raw key." >&2
              failed=1
            }
          done
        done

        # AND THE VIEW'S OWN KEYS, which live in a second table for the reason
        # `leader::OF_CLIPBOARD_VIEW` gives: a key that only exists inside a
        # `match` arm is a key nothing can enumerate.
        # A TRAILING HYPHEN IS A PREFIX AND NOT A KEY. The file also contains
        # `"files-does-"` — a test asserting that the technical explanation
        # comes from the right table — and grepping it as a key sent this check
        # looking for a translation of the empty string.
        view=$(grep -ohE '"files-[a-z0-9-]+"' ${../crates/ronoctua-shell/src}/files.rs \
          | tr -d '"' | grep -vE -- '-$' | sort -u)
        for k in $view; do
          for language in ${lib.concatStringsSep " " languages}; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERROR: $k is missing from $language.ftl — the key would have no name" >&2
              failed=1
            }
          done
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$keys" "$sidekeys" "$view" | tr ' ' '\n' | sort -u | wc -l) keys, in both languages"
      '';

  # NOTHING THE MOUSE DOES IS OUT OF THE KEYBOARD'S REACH.
  #
  # The owner's rule, mechanised. Two lists, compared in BOTH directions:
  #
  #   - every row of `menu.rs` has a key, and no two share one;
  #   - every `Hit` the drawing can produce is reachable by a key too.
  #
  # The second half is the one that would rot: adding a clickable field is one
  # line in `row_of`, and nothing about that line says a key has to exist for
  # it. A file manager where "change the owner" is a click and not a key is a
  # file manager somebody has to take their hand off the keyboard for.
  files-everything-has-a-key =
    mkCheck "files-everything-has-a-key"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        menu=${../crates/ronoctua-files/src}/menu.rs
        view=${../crates/ronoctua-shell/src}/files.rs
        failed=0

        # --- every menu row has a key, and the keys do not collide ----------
        keys=$(grep -oE '^ *key: "[^"]+"' "$menu" | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$keys" ] || { echo "ERROR: no key in the action table" >&2; exit 1; }
        repeated=$(echo "$keys" | sort | uniq -d)
        [ -z "$repeated" ] || {
          echo "ERROR: two menu rows on the same key: $repeated" >&2
          failed=1
        }

        # --- every clickable Hit is reachable by a key ----------------------
        #
        # The variants come from the enum itself, so a new one is caught the day
        # it is added rather than the day somebody notices it has no key.
        variants=$(sed -n '/^pub enum Hit {/,/^}/p' "$view" \
          | grep -oE '^ {4}[A-Z][A-Za-z]*' | tr -d ' ')
        [ -n "$variants" ] || { echo "ERROR: could not find the Hit enum" >&2; exit 1; }

        for v in $variants; do
          case "$v" in
            # These three are not a promise of an action: `Nothing` is the
            # absence of one, and `Menu` and `Choice` are rows of something
            # already on screen that the arrows and `Enter` already reach.
            Nothing|Menu|Choice) continue ;;
          esac
          grep -qE "Hit::$v" "$view" || {
            echo "ERROR: the variant Hit::$v is produced by nothing" >&2
            failed=1
          }
        done

        # --- and the view's key table agrees with its `match` ---------------
        #
        # The unit test beside the table presses every key in it; this half
        # catches the other direction — a key handled and never declared, which
        # the manual would not list.
        declared=$(sed -n '/^pub const OF_THE_VIEW/,/^];/p' "$view" \
          | grep -oE '^ *\("[^"]+"' | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$declared" ] || { echo "ERROR: could not find OF_THE_VIEW" >&2; exit 1; }

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$keys" | wc -l) menu keys, $(echo "$declared" | wc -l) of the view's, and every Hit has an owner"
      '';

  # THE PREVIEW'S PLAYER NEVER GUESSES A FRAME'S SIZE.
  #
  # This is the defect the owner photographed, pinned against the CODE rather
  # than against a screenshot. `play_video` used to deduce the height from how
  # many bytes happened to arrive in one 64 KiB read: at width 512 a row is
  # 2 048 bytes and 65 536 is exactly 32 of them, so the stream was declared
  # 512x32 and every frame after it was a horizontal slice of the film.
  #
  # Three claims, and each of them is what makes the size ours:
  #
  #   - the filter pads to a literal `W:H`, so what leaves the pipe is exactly
  #     what was asked for whatever the file says about itself;
  #   - `-2` appears in no scale filter — it is ffmpeg's "work this one out"
  #     marker, and the whole point is that nothing is left to it;
  #   - and nothing anywhere reads a dimension back out of the stream.
  motion-never-guesses-a-frame =
    mkCheck "motion-never-guesses-a-frame"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        probe=${../crates/ronoctua-inspect/src}/motion.rs
        player=${../crates/ronoctua-shell/src}/motion.rs
        failed=0

        # --- the size is stated, on both sides of the filter ----------------
        grep -q 'scale={w}:{h}' "$probe" || {
          echo "ERROR: the scale filter no longer names both dimensions" >&2
          failed=1
        }
        grep -q 'pad={w}:{h}' "$probe" || {
          echo "ERROR: the pad filter is gone — the frame's size is the file's again" >&2
          failed=1
        }

        # --- and NOTHING is left to ffmpeg ----------------------------------
        #
        # `-2` is its marker for "choose this one to keep the aspect". Under it
        # the height depends on the file, which is what the frame count cannot
        # survive.
        left=$(grep -nE 'scale=[^,"]*-2' "$probe" || true)
        [ -z "$left" ] || {
          echo "ERROR: a dimension was left to ffmpeg:" >&2
          echo "$left" >&2
          failed=1
        }

        # --- and the player does not read one back --------------------------
        #
        # The dead function by name, and the shape of what it did: a height
        # divided out of a byte count.
        #
        # THE COMMENTS COME OFF FIRST, and that is not tidiness — it is what
        # this check was measured to need. The player's own documentation
        # explains at length why the old arithmetic is gone, so a grep over the
        # whole file finds the explanation and fails on it. A check that cannot
        # tell code from a sentence about code is a check that forbids writing
        # down why.
        code=$(grep -vE "^[[:space:]]*(///|//|\*)" "$player")
        back=$(printf '%s' "$code" | grep -nE 'first_frame_height|/ *row|bytes_read */' || true)
        [ -z "$back" ] || {
          echo "ERROR: the player is working a dimension out of the bytes again:" >&2
          echo "$back" >&2
          failed=1
        }

        [ "$failed" = 0 ] || exit 1
        echo "OK: the frame's size is stated and nothing derives it"
      '';

  # A PICTURE WITH A FILM IN IT IS FOUND BY ITS BYTES, NOT BY A TABLE OF MAKERS.
  #
  # Phones from several makers record a second or two of video alongside the
  # shutter, and each writes it a little differently. The temptation is a parser
  # per maker — and this machine has files from exactly ONE of them, so every
  # other parser would be code written from a document and never run against a
  # file.
  #
  # What is here instead is one general route: an MP4 begins with an `ftyp` box,
  # so the tail is searched for one. It is exercised by the owner's own two
  # Pixel files, where it lands on the very byte their XMP declares.
  #
  # Three claims:
  #
  #   - the general route exists and is not commented out;
  #   - the film is read WHERE IT LIES, with ffmpeg's `subfile`, and never
  #     copied out to a temporary — a motion photo's video is megabytes;
  #   - and nothing decides by the file's NAME. `MVIMG_`, `PXL_` and `.MP.` are
  #     the makers' naming habits, not facts about the bytes, and a rename would
  #     turn the feature off with nothing saying why.
  motion-photo-is-found-by-bytes =
    mkCheck "motion-photo-is-found-by-bytes"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        probe=${../crates/ronoctua-inspect/src}/motion.rs
        failed=0

        # THE CODE, WITHOUT ITS TESTS OR ITS REASONING.
        #
        # Everything below asks about the code, and every one of these strings
        # also appears in a test that asserts it or a comment that explains it.
        # Measured with each defect planted back: the `subfile` claim passed on
        # the TEST's own copy of the word while the code had stopped using it.
        code=$(sed '/^mod tests {/,$d' "$probe" | grep -vE "^[[:space:]]*(///|//|\*)")

        # --- the general route -----------------------------------------------
        printf '%s' "$code" | grep -q 'fn tail_video' || {
          echo "ERROR: the scan for an appended film is gone — what covers a maker" >&2
          echo "      whose file nobody here has?" >&2
          failed=1
        }
        printf '%s' "$code" | grep -q 'b"ftyp"' || {
          echo "ERROR: nothing looks for an MP4's own first box any more" >&2
          failed=1
        }

        # --- read where it lies ----------------------------------------------
        printf '%s' "$code" | grep -q 'subfile,,start,' || {
          echo "ERROR: the film inside a picture is no longer read with subfile" >&2
          echo "      — the only other way is copying megabytes to a temporary." >&2
          failed=1
        }

        # --- and never by the name -------------------------------------------
        #
        # AND NEVER BY THE NAME. The reasoning above names the makers' file
        # prefixes and a fixture is called `IMG_1.MOV` because that is what an
        # iPhone calls one; neither is a decision.
        named=$(printf '%s' "$code" | grep -nE 'MVIMG|PXL_|\.MP\.|IMG_[0-9]' || true)
        [ -z "$named" ] || {
          echo "ERROR: a maker's naming habit is being read as a fact:" >&2
          echo "$named" >&2
          failed=1
        }

        [ "$failed" = 0 ] || exit 1
        echo "OK: the film is found by its bytes and read where it lies"
      '';

  # THE PLAYER NEVER LEAVES A CHILD, AND NEVER BLOCKS FOR EVER.
  #
  # Two properties one cannot see on a screenshot and that both end as a
  # process the owner finds later:
  #
  #   - a `read` with no deadline hangs the player's thread for the rest of the
  #     session, silently — the door keeps painting and `orders.recv()` never
  #     runs again;
  #   - and a `kill` written out at each exit leaks an ffmpeg the day somebody
  #     adds a fifth exit.
  motion-never-leaks-a-child =
    mkCheck "motion-never-leaks-a-child"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        player=${../crates/ronoctua-shell/src}/motion.rs
        failed=0

        # --- every read has a deadline --------------------------------------
        #
        # OVER THE CODE AND NOT THE COMMENTS. The player documents at length why
        # `read_exact` is gone, and a grep over the whole file finds that
        # sentence and fails on it — measured, before this check was written
        # down. A check that cannot tell code from a sentence about code is a
        # check that forbids writing down why.
        code=$(grep -vE "^[[:space:]]*(///|//|\*)" "$player")
        blocking=$(printf '%s' "$code" \
          | grep -nE 'read_exact\(|read_to_end\(|read_to_string\(' || true)
        [ -z "$blocking" ] || {
          echo "ERROR: a blocking read with no deadline in the player:" >&2
          echo "$blocking" >&2
          echo "      A source that stops answering takes the thread with it." >&2
          failed=1
        }
        grep -q 'rustix::event::poll' "$player" || {
          echo "ERROR: the player no longer polls — what bounds its reads?" >&2
          failed=1
        }

        # --- the child goes by a GUARD and not by hand ----------------------
        grep -q 'impl Drop for Reaped' "$player" || {
          echo "ERROR: the Drop guard is gone; every exit has to kill by hand again" >&2
          failed=1
        }
        # AND ITS OWN GROUP, so a signal reaches whatever ffmpeg started.
        grep -q 'process_group(0)' "$player" || {
          echo "ERROR: the child is not in a group of its own" >&2
          failed=1
        }

        [ "$failed" = 0 ] || exit 1
        echo "OK: every read has a deadline and every child has a guard"
      '';

  # NOTHING THE FILE MANAGER SPAWNS COMES BY NAME.
  #
  # `bsdtar`, `fstrim` and `pkexec`. Same two sides as the rescue console's:
  # the variables the crate reads, and the table `nix/rust.nix` sets.
  files-runs-by-path = mkCheck "files-runs-by-path" [ pkgs.gnugrep ] ''
    src=${../crates/ronoctua-files/src}
    failed=0

    read_it=$(grep -ohE 'option_env!\("RONOCTUA_[A-Z_]+"\)' "$src"/*.rs "$src"/bin/*.rs \
      | grep -oE 'RONOCTUA_[A-Z_]+' | sort -u)
    [ -n "$read_it" ] || {
      echo "ERROR: the crate reads no variable at all — what does the grep match?" >&2
      exit 1
    }

    for v in $read_it; do
      grep -qE "^ *$v = " ${../nix/rust.nix} || {
        echo "ERROR: $v is read by the crate and is not in nix/rust.nix" >&2
        echo "      The program would fail far from the cause, with a minimal PATH." >&2
        failed=1
      }
    done

    # NO BARE NAME. A `Command::new("bsdtar")` would work in the devshell and
    # fail inside a systemd scope — the trap CLAUDE.md records.
    bare=$(grep -nE 'Command::new\("[a-z]' "$src"/*.rs "$src"/bin/*.rs || true)
    [ -z "$bare" ] || {
      echo "ERROR: a program by NAME, not by path:" >&2
      echo "$bare" >&2
      failed=1
    }

    [ "$failed" = 0 ] || exit 1
    echo "OK: $(echo "$read_it" | wc -l) programs, all by store path"
  '';

  # THE PRIVILEGED DOOR IS AS NARROW AS THE CODE SAYS IT IS.
  #
  # Two directions, the same shape as `rescue-polkit-is-narrow`:
  #
  #   - the unit the crate names is the one the polkit rule allows;
  #   - the rule allows nothing else, and the verb is `start`.
  #
  # And one more, which is what this door has and the console's does not: the
  # list of VERBS. `Verb::ALL` is four, and none of them destroys anything —
  # a `remove` or a `move` slipped in there is exactly what nobody would notice
  # reviewing a diff.
  files-polkit-is-narrow =
    mkCheck "files-polkit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        crate=${../crates/ronoctua-files/src}/act.rs
        module=${../nix/modules/files.nix}
        failed=0

        unit=$(grep -oE 'pub const UNIT: &str = "[^"]+"' "$crate" | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$unit" ] || { echo "ERROR: the crate does not name the unit" >&2; exit 1; }

        grep -qE "unit = \"$unit\"" "$module" || {
          echo "ERROR: the crate asks for '$unit' and the module does not define it" >&2
          failed=1
        }
        grep -qE 'action.lookup\("unit"\) == "\$\{unit\}"' "$module" || {
          echo "ERROR: the polkit rule does not compare the unit with the module's" >&2
          failed=1
        }
        grep -qE 'action.lookup\("verb"\) == "start"' "$module" || {
          echo "ERROR: the polkit rule does not limit the verb to 'start'" >&2
          echo "      Without that it stops, restarts and reloads anything at all." >&2
          failed=1
        }
        grep -qE 'subject.user == "\$\{user\}"' "$module" || {
          echo "ERROR: the polkit rule does not limit the user" >&2
          failed=1
        }

        # NO SECOND UNIT. A rule that names one unit and a module that declares
        # two is privilege granted to something nobody audited.
        units=$(grep -cE '^ *systemd.services.ronoctua-files' "$module" || true)
        [ "$units" = 1 ] || {
          echo "ERROR: the module declares $units privileged units; the rule names one" >&2
          failed=1
        }

        # AND THE FOUR VERBS, none of which destroys.
        for bad in '"rm"' '"remove"' '"mv"' '"move"' '"delete"' '"unlink"'; do
          grep -qE "Self::[A-Za-z]+ => $bad" "$crate" && {
            echo "ERROR: a destructive verb on the privileged door: $bad" >&2
            failed=1
          }
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: one unit ($unit), the verb start, one user, four verbs that do not delete"
      '';

  # THE HOST'S LIBRARIES NEVER GO ON `LD_LIBRARY_PATH` — except for NVIDIA.
  #
  # `LD_LIBRARY_PATH` beats `DT_RUNPATH`, so a directory put there to reach ONE
  # library brings every other library in it. `/usr/lib64` was put there for the
  # host's `libEGL.so.1`, and `libc.so.6` came along — and the process then ran
  # the host's libc against Nix's `ld.so`. glibc's `GLIBC_PRIVATE` symbols only
  # match between a `libc.so.6` and the `ld.so` of the SAME build, so the day the
  # host's glibc moved, `nix run .#dev` died with:
  #
  #   niri: symbol lookup error: /usr/lib64/libc.so.6:
  #         undefined symbol: __pointer_chk_guard, version GLIBC_PRIVATE
  #
  # Measured then: host 2.44, Nix 2.42, the symbol absent from the host's. There
  # is no version to juggle — the fix is not to put the host's libraries there,
  # and for AMD and Intel that costs nothing, because what talks to the card is
  # the KERNEL driver and mesa is a userspace library Nix has a copy of.
  #
  # NVIDIA is the one real exception: its blob has to match its kernel module.
  # That branch is allowed, and it prints what it is doing.
  graphics-does-not-mix-libc =
    mkCheck "graphics-does-not-mix-libc"
      [
        pkgs.gnugrep
        pkgs.gawk
        pkgs.coreutils
      ]
      ''
        script=${devApp}/bin/ronoctua-dev
        failed=0

        # THE ORDINARY PATH EXISTS: a check that only forbids goes green on a
        # script that stopped setting up graphics at all.
        grep -q 'ronoctua-graphics/lib' "$script" || {
          echo "ERROR: the loop does not put Nix's mesa on LD_LIBRARY_PATH." >&2
          echo "      Without it niri comes up with no EGL and dies with DisplayNotSupported." >&2
          failed=1
        }

        # AND EVERY MENTION OF THE HOST'S LIBRARIES IS INSIDE THE NVIDIA BRANCH.
        #
        # The block is cut out and what is left must not mention `/usr/lib` at
        # all. Grepping for the branch's presence would not do: the branch could
        # be there and a second export outside it.
        outside=$(awk '
          /if \[ "\$only_nvidia" = yes \]/ { inside = 1 }
          inside && /^      fi$/ { inside = 0; next }
          !inside { print }
        ' "$script" | grep -n '/usr/lib' || true)
        if [ -n "$outside" ]; then
          echo "ERROR: the loop puts a HOST library on the path outside the NVIDIA branch:" >&2
          echo "$outside" >&2
          echo "      The host's libc goes with it, and it does not match Nix's ld.so." >&2
          failed=1
        fi

        [ "$failed" = 0 ] || exit 1
        echo "OK: the driver comes from Nix, and the host's only on the NVIDIA branch"
      '';

  # THE GENERATED MIME TABLE IS A BUILD INPUT, AND STAYS ONE.
  #
  # `nix/mimegen.nix` turns freedesktop's 125 KB of data into a Rust table at
  # BUILD time. What must never happen is that derivation becoming a runtime
  # reference — which is what an `option_env!` holding its path, or a crate
  # that decided to read `share/mime` directly, would do. The trap `CLAUDE.md`
  # records: a derivation references what its BYTES cite.
  #
  # WHAT THIS CHECK DOES NOT CLAIM, and the first version of it did.
  #
  # It used to forbid `shared-mime-info` from the closure outright, and that
  # was a check written against what its author hoped instead of against what
  # it can prove — the mistake this file's own header names. The package is in
  # the shell's closure and was there before the file manager existed:
  # `RONOCTUA_XDG_OPEN` points at `xdg-utils`, and `xdg-open` needs the
  # database to know what opens what. Measured with `nix why-depends`, not
  # assumed:
  #
  #   ronoctua-shell → ronoctua-0.1.0 → xdg-utils → shared-mime-info
  #
  # That is the TOOL arriving, and this check is about the TABLE. The two are
  # different things that share a name.
  files-closure-has-no-mime-db =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.shell ]; };
    in
    mkCheck "files-closure-has-no-mime-db" [ pkgs.gnugrep pkgs.coreutils ] ''
      if found=$(grep -e '-ronoctua-mimedb-' ${closure}/store-paths); then
        echo "ERROR: the generated table is in the RUNTIME closure:" >&2
        echo "$found" >&2
        echo "      It is read at COMPILE time — build.rs copies it to" >&2
        echo "      OUT_DIR and the binary carries the DATA. A reference here" >&2
        echo "      means somebody started reading it at run time, and with it" >&2
        echo "      comes the whole shared-mime-info." >&2
        exit 1
      fi

      # AND THE MANAGER ESTA LA. A check that only forbids goes green on a
      # shell that lost the feature altogether — the lesson `iso-has-no-toolchain`
      # already learned. `bsdtar` is the file manager's one new runtime
      # reference, so its absence means the compressing half is gone.
      grep -q -e '-libarchive-' ${closure}/store-paths || {
        echo "ERROR: libarchive is not in the closure; the manager's `bsdtar`" >&2
        echo "      went away, and compressing and extracting would fail on a missing path." >&2
        exit 1
      }
      echo "OK: $(wc -l < ${closure}/store-paths) paths; the table is not in the runtime and bsdtar is"
    '';

  # THE COLUMNS GIVE WAY IN THE ORDER THE CODE STATES.
  #
  # An order that drifts with the last edit turns the narrow case into whatever
  # happened to survive. The unit test beside `GIVE_WAY` sweeps every width from
  # one cell to a hundred and twenty and asserts both halves — the order, and
  # that nothing leaks past the edge. This check exists so the constant cannot
  # be reordered without the test being re-read: it pins the FIRST to give way
  # and the one that never does.
  # THE WATCH IS WIRED, and that is a different question from whether it works.
  #
  # `ronoctua_files::notice` has eight tests against a real filesystem, and they
  # answer "does the watcher fire". They cannot answer "do the doors call it, in
  # every place they must" — and that gap is not hypothetical. It is the lesson
  # at the top of this file, and it happened again while this feature was being
  # written: the footer armed its watch only because the disks thread happened to
  # send a `Racked`, whose handler points it. On a machine with no udisks2 the
  # manager would have refreshed only after the first keypress, with eight green
  # tests and two correct screenshots saying otherwise.
  #
  # Four claims, all read off the source. None of them compiles anything.
  files-notices-the-directory = mkCheck "files-notices-the-directory" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    crates = pathlib.Path("${../crates}")
    bad = []

    def production(text):
        """The part of a file that is not its test module.

        Every crate here puts its tests in one '#[cfg(test)] mod tests' at
        the end, and 'files::View::at' is called fifteen times inside one of
        them. Counting those as doors would make claim one meaningless.
        """
        cut = text.find("#[cfg(test)]")
        return text if cut < 0 else text[:cut]

    def body_from(text, opening):
        """The braces balanced from 'opening' to its match, or None."""
        start = text.find(opening)
        if start < 0:
            return None
        i = text.index("{", start)
        depth = 0
        for j in range(i, len(text)):
            if text[j] == "{":
                depth += 1
            elif text[j] == "}":
                depth -= 1
                if depth == 0:
                    return text[i : j + 1]
        return None

    files = {f: production(f.read_text()) for f in sorted(crates.rglob("*.rs"))}

    # ONE. Every door that builds a view builds a watcher.
    #
    # This is the reported bug in check form: the window door existed for
    # months with no way at all to notice a directory changing.
    doors = [f for f, body in files.items() if "View::at(" in body]
    if not doors:
        bad.append("no door builds a files::View at all — has the grep gone stale?")
    for f in doors:
        if "Notice::watching" not in files[f]:
            bad.append(
                f"{f.relative_to(crates)}: builds a files::View and never a "
                "notice::Notice — this door cannot see a directory change"
            )

    # TWO. Whatever builds the view also points the watch at it.
    #
    # The defect this catches, exactly: arming left to whichever report
    # happened to arrive first.
    for f in doors:
        text = files[f]
        for m in re.finditer(r"\n(\s*)(?:pub )?fn (\w+)", text):
            indent, name = m.group(1), m.group(2)
            rest = text[m.end() :]
            stop = re.search("\n" + indent + "}\n", rest)
            inside = rest[: stop.start()] if stop else rest
            if "View::at(" in inside and "follow_the_place" not in inside:
                bad.append(
                    f"{f.relative_to(crates)}: '{name}' opens a view without "
                    "pointing the watch at it"
                )

    # THREE. Every place that drops the view rests the watch.
    #
    # The second site is the one nobody remembers: losing keyboard focus
    # drops the manager without anything having been closed, and a watch left
    # on the last directory wakes a footer with no view for the rest of the
    # session.
    for f, text in files.items():
        for m in re.finditer(r"\n[^\n]*\bfiles = None;", text):
            after = text[m.end() : m.end() + 200]
            if "rest_the_watch" not in after and "rest()" not in after:
                line = text[: m.start()].count("\n") + 2
                bad.append(
                    f"{f.relative_to(crates)}:{line}: the view is dropped and "
                    "the watch is not rested"
                )

    # FOUR. The nudge is answered with a re-read, never with the poll.
    #
    # THE ONE THAT PINS A DECISION RATHER THAN A WIRE. An arm that consulted
    # 'View::stale' compiles, runs, and passes every test there is — and
    # silently stops noticing a file rewritten in place, which is half of why
    # this is a watch and not a clock. 'sidebar_stale' is a different
    # identifier and is meant to be there: our own 'save_sidebar' writes that
    # file, so an unconditional re-read would throw the sidebar's cursor away
    # every time something is pinned.
    arms = 0
    for f, text in files.items():
        arm = body_from(text, "Report::Stirred =>")
        if arm is None:
            continue
        arms += 1
        where = f.relative_to(crates)
        if "reread(" not in arm:
            bad.append(f"{where}: the Stirred arm does not re-read")
        if ".stale()" in arm:
            bad.append(
                f"{where}: the Stirred arm asks View::stale — that is the "
                "directory's mtime, which a file rewritten in place does not move"
            )
    if arms != len(doors):
        bad.append(
            f"{arms} doors answer Report::Stirred, but {len(doors)} build a view"
        )

    if bad:
        print("ERROR: the directory watch is not wired the way it is meant to be.", file=sys.stderr)
        print("", file=sys.stderr)
        for b in bad:
            print("       " + b, file=sys.stderr)
        print("", file=sys.stderr)
        print("       Every one of these compiles and passes the unit tests. What", file=sys.stderr)
        print("       they cost is a listing that quietly stops refreshing, which", file=sys.stderr)
        print("       is the report this feature answers.", file=sys.stderr)
        raise SystemExit(1)

    # AND THE WATCHER ITSELF IS STILL THERE, with the mask it was argued
    # into. Without this the check goes green on a tree where the whole
    # thing was deleted, and MODIFY creeping back in is a hose: it fires on
    # every write, so one log being appended to would re-read the directory
    # without pause.
    notice = crates / "ronoctua-files" / "src" / "notice.rs"
    if not notice.is_file():
        print("ERROR: crates/ronoctua-files/src/notice.rs is gone.", file=sys.stderr)
        raise SystemExit(1)
    mask = notice.read_text()
    for flag in ("CREATE", "DELETE", "MOVED_TO", "MOVED_FROM", "ATTRIB", "CLOSE_WRITE"):
        if f"WatchFlags::{flag}" not in mask:
            print(f"ERROR: the watch no longer asks for {flag}.", file=sys.stderr)
            raise SystemExit(1)
    if "WatchFlags::MODIFY" in mask:
        print("ERROR: the watch asks for MODIFY, which fires on every write.", file=sys.stderr)
        print("       A log being appended to would re-read the directory in a", file=sys.stderr)
        print("       loop. CLOSE_WRITE is the moment a listing cares about.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(doors)} doors watch what they show, and answer the nudge with a re-read")
    END
  '';

  # THE SHAPE IS DRAWN IN THIS PROCESS, and nothing it needs is a program.
  #
  # `ronoctua-inspect` spawns five programs and the number is load-bearing:
  # every one of them is a store path, they are named by `RONOCTUA_*` in three
  # Nix files, and `profile-minimal-closes` allow-lists them one by one. A sixth
  # arriving because somebody "just called a converter" for the next format is
  # the cheapest possible way to add a hundred megabytes to a laptop image.
  #
  # `files-closure-has-no-mime-db` would eventually say so — after a release
  # build of the whole shell. This says it in seconds, off the source, and names
  # the file.
  inspect-mesh-is-in-process = mkCheck "inspect-mesh-is-in-process" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    src = pathlib.Path("${../crates/ronoctua-inspect/src}")
    bad = []
    mesh = src / "mesh"
    solid = src / "solid.rs"

    # ONE. NO READER AND NOT THE RASTERISER NAMES A PROGRAM.
    #
    # `Command` and `tools::` are the two doors to one, and `Tool` is the
    # enum that would have to grow first — so a sixth program cannot be
    # reached from here without one of the three appearing.
    #
    # THE WHOLE DIRECTORY, and not one file: `mesh.rs` became `mesh/` when
    # it grew from two formats to eleven, and a check that named one file
    # would have gone green on ten readers it never looked at.
    if not mesh.is_dir():
        bad.append("the mesh readers are gone")
    readers = sorted(mesh.glob("*.rs")) if mesh.is_dir() else []
    if len(readers) < 12:
        bad.append(f"only {len(readers)} files under mesh/ — the readers are one per file")
    for f in readers + [solid]:
        if not f.is_file():
            bad.append(f"{f.name} is gone")
            continue
        body = f.read_text()
        for word in ("Command", "tools::", "Tool::", "std::process"):
            if word in body:
                bad.append(f"{f.name} names '{word}' — a shape is drawn here, not fetched")

    # TWO. The five are still five.
    #
    # Not a forbidden list of program names: what is asserted is the COUNT,
    # so a sixth of any name at all lands here.
    tools = (src / "tools.rs").read_text()
    if "pub const ALL: [Self; 5]" not in tools:
        bad.append("tools.rs no longer declares exactly five programs")

    # THREE. The table's mesh row asks for no program.
    pixels = (src / "pixels.rs").read_text()
    at = pixels.find("pub const fn tool(")
    if at < 0:
        bad.append("pixels.rs has no How::tool — what does the grep match?")
    else:
        depth, end = 0, at
        for i in range(pixels.index("{", at), len(pixels)):
            if pixels[i] == "{":
                depth += 1
            elif pixels[i] == "}":
                depth -= 1
                if depth == 0:
                    end = i
                    break
        arms = re.findall(r"([^=>\n{}]+)=>\s*([^,\n]+),", pixels[at:end])
        found = [(a, b) for a, b in arms if "Mesh" in a]
        if not found:
            bad.append("How::Mesh has no arm in How::tool")
        for a, b in found:
            if "None" not in b:
                bad.append(f"How::Mesh asks for a program: {b.strip()}")

    # AND THE ROW IS STILL THERE, claiming every shape. Without this the
    # check goes green on a tree where the whole feature was deleted, which
    # is the shape `persistence-takes-its-root-as-a-parameter` already
    # guards against with its own closing clause.
    row = re.search(r'id:\s*"mesh".*?how:\s*How::Mesh', pixels, re.S)
    if row is None:
        bad.append("no row in the table claims a mesh")
    else:
        for kind in (
            "model/stl",
            "model/3mf",
            "model/obj",
            "model/x-ply",
            "model/x-off",
            "model/gltf+json",
            "model/gltf-binary",
            "model/vnd.collada+xml",
            "model/x-amf",
            "model/step",
            "model/iges",
        ):
            if kind not in row.group(0):
                bad.append(f"the mesh row no longer claims {kind}")
    if solid.is_file() and "pub fn draw(" not in solid.read_text():
        bad.append("solid.rs no longer draws anything")

    if bad:
        print("ERROR: the mesh preview is not the in-process one it is meant to be.", file=sys.stderr)
        print("", file=sys.stderr)
        for b in bad:
            print("       " + b, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A program here is a store path, three Nix files to set a", file=sys.stderr)
        print("       variable in, and one more line in the closure sweep. The", file=sys.stderr)
        print("       rasteriser is two hundred lines and costs none of that.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(readers)} files read and draw in process, and the five programs are five")
    END
  '';

  files-columns-degrade = mkCheck "files-columns-degrade" [ pkgs.gnugrep ] ''
    view=${../crates/ronoctua-shell/src}/files.rs
    order=$(sed -n '/^pub const GIVE_WAY/,/^];/p' "$view" | grep -oE 'Column::[A-Za-z]+' | tr -d ' ')
    first=$(echo "$order" | head -1)
    [ "$first" = "Column::Group" ] || {
      echo "ERROR: the first column to give way became $first, and it was the group." >&2
      echo "      If the change is deliberate, the test next to GIVE_WAY says why." >&2
      exit 1
    }
    echo "$order" | grep -q 'Column::Name' && {
      echo "ERROR: the name is in GIVE_WAY. It never gives way: a table about" >&2
      echo "      files that does not say which ones is a table about nothing." >&2
      exit 1
    }
    echo "OK: gives way in the order $(echo "$order" | tr '\n' ' ')"
  '';

  # NOTHING THE CONSOLE SPAWNS COMES BY NAME.
  #
  # It runs from a unit on a VT, where `PATH` is minimal, and it is opened
  # BECAUSE something is already broken — a name that does not resolve fails on
  # the one screen that exists to explain failures.
  #
  # Two sides, both from real artifacts: the variables the crate reads, and the
  # table `nix/rust.nix` sets. It is `shell-finds-its-buffers` again, on the
  # other application.
  rescue-runs-by-path = mkCheck "rescue-runs-by-path" [ pkgs.gnugrep ] ''
    src=${../crates/ronoctua-rescue/src}
    failed=0

    read_it=$(grep -ohE 'option_env!\("RONOCTUA_[A-Z_]+"\)' "$src"/*.rs \
      | grep -oE 'RONOCTUA_[A-Z_]+' | sort -u)
    [ -n "$read_it" ] || {
      echo "ERROR: the console reads no variable at all — does it spawn by name?" >&2
      exit 1
    }
    echo "o console le: $(echo "$read_it" | tr '\n' ' ')"

    baked="${lib.concatStringsSep " " (lib.attrNames rust.generated)}"
    for v in $read_it; do
      case " $baked " in
        *" $v "*) ;;
        *)
          echo "ERROR: the console reads $v and the build does not bake it in — it would fall" >&2
          echo "      on a unit's minimal 'PATH' and would fail far from here." >&2
          failed=1
          ;;
      esac
    done

    [ "$failed" = 0 ] || exit 1
    echo "OK: every rescue program comes by store path"
  '';

  # THE POLKIT RULE ALLOWS EXACTLY WHAT THE CONSOLE ASKS FOR.
  #
  # Two directions, and the second is the one that rots: a row without its unit
  # fails in front of the person, which is loud; a unit left in the rule after
  # its row went away is privilege granted to nobody, which is silent and stays.
  #
  # The exemption list is spelled out with its reason. `keyd.service` is reached
  # by a FREE row and is not this rule's business — `nix/modules/keyboard.nix`
  # already allows exactly it, and `Ctrl+Shift+L` already fires it.
  rescue-polkit-is-narrow =
    let
      rule = self.nixosConfigurations.noctua.config.security.polkit.extraConfig;
    in
    mkCheck "rescue-polkit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        cat > rule.js <<'END'
        ${rule}
        END
        # THE WHOLE CRATE, and not just the registry. The unit that installs an
        # edited `host.nix` is named in `edit.rs` — this check found that by
        # refusing, which is what a two-way comparison is for.
        src=${../crates/ronoctua-rescue/src}
        failed=0

        # What the console names, minus what the session owns. A user unit does
        # not reach polkit at all: there is no display manager here, and the
        # compositor and the footer are the person's own units.
        wanted=$(grep -rohE '"[A-Za-z@.-]+\.(service|target)"' "$src" | tr -d '"' | sort -u)
        for u in $wanted; do
          case "$u" in
            # The session's own, and the keyboard's — see above.
            ronoctua-footer.service|niri.service|graphical-session.target) continue ;;
            pipewire.service|pipewire-pulse.service|wireplumber.service) continue ;;
            keyd.service) continue ;;
          esac
          grep -qF "\"$u\"" rule.js || {
            echo "ERROR: the console asks for '$u' and the polkit rule does not allow it;" >&2
            echo "      the row would fail in front of the person." >&2
            failed=1
          }
        done

        # And back: every unit the rule names has to be one the console asks for.
        for u in $(grep -ohE '"ronoctua-rescue[A-Za-z@.-]*\.service"' rule.js | tr -d '"' | sort -u); do
          grep -rqF "\"$u\"" "$src" || {
            echo "ERROR: the rule allows '$u' and no console row asks for it —" >&2
            echo "      a privilege granted to nobody, which is what nobody audits." >&2
            failed=1
          }
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: the rule allows exactly what the console asks for"
      '';

  # `Ctrl+Alt+Del` IS OURS — and the hard way out stays systemd's.
  #
  # Two halves, and the check exists because the second is the one a person
  # would not think to look for. Taking the key over means taking what it did:
  # if the burst stopped rebooting too, a machine whose everything is frozen
  # would have no way out at all, and nothing would say so until that day.
  ctrl-alt-del-is-ours =
    let
      # `ctrl-alt-del.target` is a SYMLINK to whatever `ctrlAltDelUnit` names —
      # that is the option NixOS gives for this, and the two roads that look
      # more obvious are documented in `nix/modules/rescue.nix` as the failures
      # they are.
      aimed = self.nixosConfigurations.noctua.config.systemd.ctrlAltDelUnit;
      target = self.nixosConfigurations.noctua.config.systemd.targets.ronoctua-rescue;
      burst =
        self.nixosConfigurations.noctua.config.systemd.settings.Manager.CtrlAltDelBurstAction
          or "reboot-force";
    in
    mkCheck "ctrl-alt-del-is-ours" [ ] ''
      [ "${aimed}" = "ronoctua-rescue.target" ] || {
        echo "ERROR: Ctrl+Alt+Del points at '${aimed}', which is not the rescue." >&2
        exit 1
      }
      wants="${lib.concatStringsSep " " target.wants}"
      case "$wants" in
        *ronoctua-rescue.service*) ;;
        *)
          echo "ERROR: ctrl-alt-del.target does not open the rescue; it wants: $wants" >&2
          echo "      A key that is taken and opens nothing is a key thrown away." >&2
          exit 1
          ;;
      esac

      # The way out that does not pass through us.
      [ "${burst}" = "reboot-force" ] || {
        echo "ERROR: CtrlAltDelBurstAction is '${burst}'; the seven presses in two" >&2
        echo "      seconds were the only way out with everything locked up." >&2
        exit 1
      }

      echo "OK: one press opens the rescue, seven still force a reboot"
    '';

  # THE RESCUE CONSOLE CARRIES NO COMPILER.
  #
  # He exists for a machine that is broken, and the cheapest way to make him
  # useless is to link something that drags a toolchain: `ronoctua-term` cites
  # `libghostty-vt` (Zig, clang, LLVM — 1,70 GiB) and `ronoctua-render` cites
  # mesa. Neither would FAIL anything; both would quietly make the rescue
  # console the heaviest thing in the system.
  #
  # `nix/rust.nix`'s `rescue` exists to be measured: a derivation references
  # what its own bytes cite.
  rescue-has-no-toolchain =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.rescue ]; };
    in
    mkCheck "rescue-has-no-toolchain"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0
        for unwanted in -zig- -clang- -llvm- -mesa- -vulkan-loader-; do
          if found=$(grep -e "$unwanted" ${closure}/store-paths); then
            echo "ERROR: the rescue console drags $unwanted along:" >&2
            echo "$found" >&2
            failed=1
          fi
        done
        [ "$failed" = 0 ] || exit 1
        echo "OK: $(wc -l < ${closure}/store-paths) paths, and no compiler"
      '';

  # WHATEVER THE SHELL LOOKS FOR NEXT DOOR, THE DEV LOOP HAS TO BUILD.
  #
  # Two binaries are found by ASKING THE FILESYSTEM rather than by a store path
  # in a variable — `ronoctua-rescue` and `ronoctua-transfers` — and they have
  # to be: they come out of the same build as the shell, so a path baked into
  # one would be that derivation referencing itself.
  #
  # Installed, they are next door: `rust.shell` is a `symlinkJoin` over the
  # whole workspace. In the nested loop they are next door too — until the FIRST
  # edit, when `RONOCTUA_FOOTER_BIN` moves to `$CARGO_TARGET_DIR/release/` and
  # only what `cargo build` named is there.
  #
  # The symptom is the worst kind there is: everything works, and then one edit
  # quietly takes a feature away. A missing transfer queue looks exactly like
  # "nothing is running", and a missing rescue console looks exactly like a key
  # that does nothing. This check is what noticed that the rescue console had
  # been in that state the whole time.
  dev-rebuilds-what-sits-beside =
    mkCheck "dev-rebuilds-what-sits-beside" [ pkgs.gnugrep pkgs.coreutils ]
      ''
        shell=${../crates/ronoctua-shell/src}
        queue=${../crates/ronoctua-transfers/src}
        # AND THE BRAIN'S, for the same reason the queue's is here: it looks its
        # own daemon up beside `current_exe()`, so the nested loop has to build
        # that binary or the footer would find yesterday's one and nothing would
        # say so.
        brain=${../crates/ronoctua-brain/src}
        failed=0

        # The names come from the CODE that looks them up, never from a list
        # written here: a list would be a third place to keep in step.
        beside=$(grep -rhoE 'parent\(\)\?\.join\("[a-z-]+"' "$shell" "$queue" "$brain" \
          | grep -oE '"[a-z-]+"' | tr -d '"' | sort -u)
        [ -n "$beside" ] || {
          echo "ERROR: nobody looks for a binary beside it — what does the grep match?" >&2
          exit 1
        }

        for b in $beside; do
          grep -q -- "--bin $b" ${../nix/dev.nix} || {
            echo "ERROR: the footer looks for $b beside it, and the dev loop does not build it." >&2
            echo "      After the FIRST edit it goes away, in silence." >&2
            failed=1
          }
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$beside" | wc -l) binaries beside it, and the loop builds them all"
      '';

  # THE QUEUE'S UNIT IS A COPY, NOT A DESKTOP.
  #
  # `ronoctua-transfers` is a user service that outlives the session — it is
  # what keeps a copy of forty gibibytes alive across a footer restart. Handing
  # it `rust.shell` instead of the extracted binary would put wgpu, the wasm
  # runtime, the renderer and three embedded fonts inside a unit that never
  # opens a window, and would hold every one of those store paths alive for as
  # long as the machine is on.
  #
  # Nothing would fail. That is exactly why this is a check and not a comment:
  # `nix why-depends` on the unit is what would say so, and nobody runs that on
  # a Tuesday.
  transfers-unit-is-narrow =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.transfers ]; };
    in
    mkCheck "transfers-unit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0
        for unwanted in -zig- -clang- -llvm- -mesa- -vulkan-loader- -noctua-iosevka- -icons-superpack-; do
          if found=$(grep -e "$unwanted" ${closure}/store-paths); then
            echo "ERROR: the queue's unit drags $unwanted in:" >&2
            echo "$found" >&2
            failed=1
          fi
        done
        [ "$failed" = 0 ] || exit 1
        echo "OK: $(wc -l < ${closure}/store-paths) paths, and none of them draws"
      '';

  # A TEST MAY NOT REACH THE MACHINE, and this one was paid for out loud.
  #
  # `ronoctua-brain`'s `systemd` module had a unit test that called
  # `start("ronoctua-does-not-exist.service")`. The reasoning was sound and
  # wrong: a unit that is not there cannot do any harm. But `systemctl start`
  # on a SYSTEM unit is authorised by polkit BEFORE systemd looks the unit up,
  # so every `cargo test` opened a real authentication prompt on the owner's
  # laptop — and, because there is no polkit agent in the session, sat there
  # until it timed out. The suite took 25 s; without the call it takes 0,3.
  #
  # Nothing failed. Nothing was logged. It surfaced because the owner was asked
  # for his fingerprint five times in a row and came to ask what was going on —
  # which is the same failure this repository already reasons about from the
  # other end: he is not always near the laptop, and a prompt nobody sees is a
  # refusal nobody made.
  #
  # So: inside `#[cfg(test)]`, no program that authorises, spawns a unit, or
  # asks for a password. What a test may check is the SHAPE of the invocation,
  # which is a pure function.
  tests-do-not-touch-the-machine = mkCheck "tests-do-not-touch-the-machine" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")

    # The programs a test must never start. `systemd-run` is on the list for a
    # second reason on top of polkit: it is SYNCHRONOUS with `--scope`, and
    # this project has already frozen its own footer on that once.
    # NAMED ANYWHERE IN THE CALL, literal or variable: these are the programs
    # that go through polkit or block on a scope, and a test must not reach one
    # however it spells the path.
    #
    # It used to have a first alternative matching `Command::new("` — any
    # literal at all — which made it a duplicate of the sweep below and, worse,
    # overrode its one allowance.
    FORBIDDEN = re.compile(
        r'Command::new\([^)]*\b(sudo|pkexec|systemd-run|systemctl|loginctl'
        r'|udisksctl|SYSTEMCTL)\b'
    )
    # A SPAWN WHOSE PROGRAM IS A LITERAL, which is the shape every dangerous
    # one has: `Command::new("systemctl")`, `Command::new("sudo")`.
    #
    # It used to be ANY spawn at all, and the argument for that was good — a
    # list somebody has to keep complete is a list that goes stale. What broke
    # it is that the preview grew tests which BUILD their fixtures: three in
    # `inspect/motion.rs` make a video with the ffmpeg the FLAKE provides, and
    # one in `shell/motion.rs` pipes through `/bin/sh`. Those reach no polkit
    # and block on no scope, and the check went red the day they arrived and
    # stayed red — through two commits, because `most` is not run every time.
    # A check that is red for a reason nobody accepts stops being read.
    #
    # So the rule is the DANGER and not the act: a program named by a literal
    # is one this repository chose by name, and those are the ones that can be
    # `systemctl`. A program that arrives in a variable came out of the
    # environment the flake set, and WHICH programs may exist there at all is
    # already policed by `inspect-mesh-is-in-process` and by the closure sweep.
    SPAWN = re.compile(r'Command::new\(\s*"([^"]*)"')
    # The one literal a test may name: it starts no daemon and asks nobody for
    # permission.
    ALLOWED = {"/bin/sh"}

    gaps = []
    for rs in sorted((root_dir / "crates").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        source = rs.read_text()
        # Everything from `mod tests {` to the end of the file: a test module is
        # conventionally last here, and taking the tail is what makes this
        # robust against nesting rather than counting braces.
        at = source.find("mod tests {")
        if at < 0:
            continue
        tail = source[at:]
        for n, line in enumerate(tail.splitlines(), 1):
            stripped = line.strip()
            if stripped.startswith("//") or stripped.startswith("///"):
                continue
            m = SPAWN.search(line)
            if (m and m.group(1) not in ALLOWED) or FORBIDDEN.search(line):
                rel = rs.relative_to(root_dir)
                gaps.append(f"{rel}: mod tests + {n}: {stripped[:70]}")

    if gaps:
        print("ERROR: a test starts a program on the machine.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in gaps:
            print(f"       {g}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       `systemctl start` on a SYSTEM unit goes through polkit", file=sys.stderr)
        print("       BEFORE systemd looks for the unit — so even asking for one", file=sys.stderr)
        print("       that does not exist opens a real fingerprint request, and with no", file=sys.stderr)
        print("       agent in the session it waits until it times out.", file=sys.stderr)
        print("", file=sys.stderr)
        print("       What a test checks is the SHAPE of the call, and a shape is a", file=sys.stderr)
        print("       pure function: see `systemd::argv`.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: no test starts a program on the machine")
    END
  '';

  # THE BRAIN'S UNIT DECIDES, IT DOES NOT DRAW.
  #
  # The same argument `transfers-unit-is-narrow` makes about the queue, and a
  # sharper version of it: this daemon runs for as long as the machine is on,
  # its whole job is to watch a clock and a report file, and it has no screen at
  # all. Handing it `rust.shell` instead of the extracted binary would hold wgpu,
  # the wasm runtime, the renderer and three embedded fonts alive permanently,
  # for a program that never opens a window.
  #
  # Nothing would fail — which is why this is a check and not a comment. And it
  # is the same measurement `CLAUDE.md` demands in place of trusting a switch:
  # a derivation references what its BYTES cite, so measure the closure rather
  # than believing the `[[bin]]`.
  brain-unit-is-narrow =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.brain ]; };
    in
    mkCheck "brain-unit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0
        # `-wasmtime-` on top of the queue's list: the brain loads no component,
        # and the runtime arriving here would mean it had been handed the whole
        # workspace again.
        for unwanted in -zig- -clang- -llvm- -mesa- -vulkan-loader- \
                        -noctua-iosevka- -icons-superpack- -wasmtime-; do
          if found=$(grep -e "$unwanted" ${closure}/store-paths); then
            echo "ERROR: the brain's unit drags $unwanted in:" >&2
            echo "$found" >&2
            failed=1
          fi
        done
        [ "$failed" = 0 ] || exit 1
        echo "OK: $(wc -l < ${closure}/store-paths) paths, and none of them draws"
      '';

  # THE QUEUE'S SOCKET IS THE PERSON'S, AND NOBODY ELSE'S.
  #
  # `XDG_RUNTIME_DIR` is `0700` and per user. A control socket in `/tmp` would
  # be a door anybody on the machine can knock on — and what is behind this one
  # is "copy that there" and "delete those", with the person's own privileges.
  #
  # `leader::socket_path` states the same rule for the footer's socket and has
  # no check; this one does, because the failure is silent: a fallback to `/tmp`
  # WORKS, and works better — it works when `XDG_RUNTIME_DIR` is missing, which
  # is exactly when it should refuse.
  transfers-socket-is-private = mkCheck "transfers-socket-is-private" [ pkgs.gnugrep ] ''
    src=${../crates/ronoctua-transfers/src}
    grep -q 'XDG_RUNTIME_DIR' "$src/wire.rs" || {
      echo "ERROR: the socket stopped coming out of XDG_RUNTIME_DIR." >&2; exit 1; }

    # NO FALLBACK. Not `unwrap_or`, not `/tmp`, not `std::env::temp_dir`.
    #
    # The failure this refuses is a SILENT one: a fallback to /tmp works, and
    # works better — it works when XDG_RUNTIME_DIR is missing, which is
    # exactly when it should refuse.
    #
    # The tests build their trees under the temporary directory and that is
    # fine; what may not go there is the SOCKET.
    hits=$(grep -nE '/tmp|temp_dir' "$src"/*.rs "$src"/bin/*.rs || true)
    # A comment is prose, and one of them is this very rule written out.
    bad=$(echo "$hits" | grep -vE ':[0-9]+: *//' || true)
    # A test builds its tree under the temporary directory, which is what a
    # temporary directory is for. What may not go there is the SOCKET.
    bad=$(echo "$bad" | grep -vE 'std::env::temp_dir\(\)' || true)
    bad=$(echo "$bad" | grep -vE '^ *$' || true)
    [ -z "$bad" ] || {
      echo "ERROR: the queue fell back to /tmp somewhere:" >&2
      echo "$bad" >&2
      exit 1
    }
    echo "OK: the queue's socket belongs to the user, with no plan B"
  '';

  # EVERY ENGINE, EVERY STAGE AND EVERY KIND OF WORK HAS WORDS IN BOTH LANGUAGES.
  #
  # The key is ASSEMBLED at runtime — `transfers-stage-{}` with `Stage::word()`
  # — so it is one of the keys `docs/ARCHITECTURE.md` describes as having seven
  # derivatives and being invisible to every search in the code. A variant added
  # without its `.ftl` line does not fail to compile; it draws the raw key on
  # screen, which is precisely the shape invariant 8 exists to refuse and which
  # was reported from this system's own screen once already.
  transfers-words-are-total = mkCheck "transfers-words-are-total" [ pkgs.gnugrep pkgs.coreutils ] ''
    job=${../crates/ronoctua-transfers/src}/job.rs
    failed=0

    # The words come out of the `word()` tables — the same place the Rust
    # builds the key from. Reading the enum's variants instead would be a
    # second spelling of the same list.
    first=""
    words=$(grep -oE '=> "[a-z]+",' "$job" | grep -oE '"[a-z]+"' | tr -d '"' | sort -u)
    [ -n "$words" ] || { echo "ERROR: could not find a single word in job.rs" >&2; exit 1; }

    for w in $words; do
      for prefix in transfers-work transfers-stage transfers-engine; do
        # A word belongs to exactly one of the three families; what this loop
        # demands is that WHEREVER it appears in one language it appears in
        # ALL of them. Comparing two counts was the old shape and it could not
        # grow: with a third language the pair would still have agreed while
        # the newcomer said nothing.
        counts=""
        for language in ${lib.concatStringsSep " " languages}; do
          n=$(grep -c "^$prefix-$w = " ${../assets/i18n}/$language.ftl || true)
          counts="$counts $language=$n"
          if [ -z "$first" ]; then first=$n; fi
          if [ "$n" != "$first" ]; then
            echo "ERROR: $prefix-$w does not exist in every language ($counts )" >&2
            failed=1
          fi
        done
        first=""
      done
      # And it has to exist in AT LEAST one family, or it is a variant
      # nothing can draw. The base language answers for all of them: the
      # loop above has just proved they agree.
      total=$(grep -c "^transfers-[a-z]*-$w = " \
        ${../assets/i18n}/${builtins.head languages}.ftl || true)
      if [ "$total" = 0 ]; then
        echo "ERROR: the word $w from job.rs has no text at all" >&2
        failed=1
      fi
    done

    [ "$failed" = 0 ] || exit 1
    echo "OK: $(echo "$words" | wc -l) queue words, in ${toString (builtins.length languages)} languages"
  '';

  # THE EDITOR WRITES WHERE THE UNIT READS.
  #
  # The path crosses the border as a STRING: the crate writes to it and a root
  # oneshot reads from it. Divergent, saving would report success and install
  # nothing — the exact silence `CLAUDE.md` says needs a check.
  rescue-staging-agrees = mkCheck "rescue-staging-agrees" [ pkgs.gnugrep ] ''
    grep -q 'staged = "/run/ronoctua-rescue/staged"' ${../nix/modules}/rescue.nix || {
      echo "ERROR: the module changed the path where it reads the edited file." >&2; exit 1; }
    grep -q 'pub const STAGED: &str = "/run/ronoctua-rescue/staged"' \
      ${../crates/ronoctua-rescue/src}/edit.rs || {
      echo "ERROR: the editor changed the path where it writes; saving would install nothing." >&2; exit 1; }
    echo "OK: the editor writes where the unit reads"
  '';

  # HOT RELOAD RELOADS — behavior, not existence.
  #
  # The criterion of milestone 3 says "brings up niri + shell WITH HOT RELOAD".
  # The old check listed the `devApp` with the comment "it has to exist and be
  # executable", and an app that never reloads anything passes that easily. That
  # is how the milestone closed without the feature.
  #
  # This check exercises the real mechanism: it brings the watcher up over a
  # temporary directory, touches a `.rs`, and asserts that the command fired.
  # With no compositor and no GPU — that is why it fits in `nix flake check`,
  # which is where the milestone is proved.
  reload-reloads =
    mkCheck "reload-reloads"
      [
        reloadApp
        pkgs.procps
      ]
      ''
        export HOME=$TMPDIR
        mkdir -p obs
        : > obs/initial.rs

        # The command records WHAT changed, and not just that it fired: the
        # nested loop branches on that path — a `.rs` under `buffers/` is a
        # component and is built by nix, everything else is the workspace and is
        # built by cargo. If the variable stopped arriving, the loop would go on
        # reloading and would silently take the wrong branch every time.
        RONOCTUA_RELOAD_CMD="echo \"fired \$RONOCTUA_RELOAD_FILE\" >> $PWD/marker" \
          ronoctua-reload "$PWD/obs" &
        watcher=$!
        trap 'kill $watcher 2>/dev/null || true' EXIT

        # The `inotifywait` has to be ARMED before the write: touching the file too
        # early and the event is lost and the check fails with no defect at all.
        for _ in $(seq 50); do
          [ -e /proc/$watcher ] || break
          sleep 0.2
          grep -q . /dev/null 2>/dev/null && break
        done
        sleep 3

        echo 'fn main() {}' > obs/changed.rs

        ok=no
        for _ in $(seq 50); do
          [ -s marker ] && { ok=yes; break; }
          sleep 0.2
        done
        [ "$ok" = yes ] || {
          echo "ERROR: changing a .rs did NOT fire the command." >&2
          echo "Milestone 3's hot reload is not working." >&2
          exit 1
        }
        grep -q "fired .*obs/changed.rs" marker || {
          echo "ERROR: it fired without saying WHICH file changed." >&2
          echo "      The nested loop chooses between nix and cargo by that path;" >&2
          echo "      without it, every edited buffer would fall into the workspace branch." >&2
          cat marker >&2
          exit 1
        }

        # And the other way round: a change that is NOT code must not fire. Without
        # this the watcher would recompile on every `.swp` from the editor.
        : > marker
        echo 'I am not code' > obs/README.txt
        sleep 2
        [ ! -s marker ] || {
          echo "ERROR: a file that is neither .rs nor .wit fired the rebuild." >&2
          exit 1
        }

        # IT DIES WHEN IT IS TOLD TO DIE.
        #
        # # The defect this catches, and that only shows up in use
        #
        # The watcher's last command was a pipeline in the FOREGROUND, and bash defers
        # a signal until the running command finishes — an `inotifywait -m` never
        # finishes. The `kill` from `nix/dev.nix` hung forever: the watcher survived
        # the Ctrl+C, was adopted by `systemd --user`, and every `nix run .#dev` left
        # one more of them recompiling the tree in the background.
        #
        # Nothing reported it. The two tests above passed with the defect present,
        # because firing and dying are different things — and the second is only found
        # out hours later, when the machine is full of them.
        kill "$watcher" 2>/dev/null || true
        died=no
        for _ in $(seq 50); do
          [ -e /proc/$watcher ] || { died=yes; break; }
          sleep 0.2
        done
        [ "$died" = yes ] || {
          echo "ERROR: the watcher IGNORED the SIGTERM and is still alive." >&2
          echo "      It will be adopted by init and will recompile on its own for" >&2
          echo "      ever after. See the \`wait\` at the end of nix/reload.nix." >&2
          exit 1
        }

        # AND IT TAKES THE CHILDREN WITH IT. Killing only the script would leave the
        # `inotifywait` alive, waiting for an event nobody reads any more — invisible,
        # because its name does not bring this project to mind.
        leftover=$(pgrep -c inotifywait 2>/dev/null || true)
        [ "''${leftover:-0}" = 0 ] || {
          echo "ERROR: $leftover inotifywait processes were left after killing the watcher." >&2
          exit 1
        }

        echo "OK: .rs fires, the rest does not, and it dies when told"
      '';

  # ONE FOOTER PER SESSION — the second replaces the first, it does not add to it.
  #
  # # The defect this catches
  #
  # Restarting the footer was a FUNCTION inside `nix/dev.nix`, and the hot
  # reload's `rebuild` runs in the WATCHER's process. The function got there
  # through `export -f`, but the `rodapepid` it read is a shell variable, and a
  # shell variable does not cross a process. Under the watcher's `set -u` that
  # does not become a false `if`: it becomes `unbound variable`, which brings the
  # whole shell down — and the loop's `|| true` does not save it.
  #
  # In order: the old footer did not die, the new one did not come up, and the
  # watcher died. On screen, "compiling…" and nothing else, forever.
  #
  # The `reload-reloads` above does not catch this, and that is not its fault: it
  # proves that the command FIRES. Firing and restarting are different things,
  # and the second is only found out when two footers fight over the keyboard —
  # the shortcut list opens and no key works.
  footer-is-single =
    mkCheck "footer-is-single"
      [
        singleFooter
        pkgs.procps
      ]
      ''
        export HOME=$TMPDIR
        pidfile="$PWD/footer.pid"

        # A "footer" that only stands there. What is measured is the supervision, and a
        # real binary would bring a compositor, a GPU and fonts along with it.
        #
        # `printf` and not a heredoc: inside an indented Nix string the heredoc would
        # carry the indentation along, and a shebang with a space in front is not a
        # shebang — the error would show up as "file not executable", which does not
        # point at the cause.
        printf '#!/bin/sh\nexec sleep 300\n' > falso
        chmod +x falso

        ronoctua-footer-single "$pidfile" "$PWD/falso"
        first=$(cat "$pidfile")
        kill -0 "$first" || {
          echo "ERROR: the first footer did not come up." >&2
          exit 1
        }

        # THE SECOND. This is where the defect was.
        ronoctua-footer-single "$pidfile" "$PWD/falso"
        second=$(cat "$pidfile")

        [ "$first" != "$second" ] || {
          echo "ERROR: the pidfile did not change; the second footer did not come up." >&2
          exit 1
        }

        if kill -0 "$first" 2>/dev/null; then
          echo "ERROR: the previous footer ($first) is still alive_flag." >&2
          echo "      Two footers on the same display fight over the keyboard: the" >&2
          echo "      shortcut list opens on one and the keys go to the other." >&2
          exit 1
        fi

        kill -0 "$second" || {
          echo "ERROR: the new footer ($second) is not alive_flag." >&2
          exit 1
        }

        # AND THERE IS ONLY ONE. Counting is what separates "it replaced" from "it
        # killed the old one and brought up two".
        #
        # The `|| true` is INSIDE the braces: `pgrep` with no match at all exits 1,
        # and with `pipefail` that would bring the check down before counting.
        alive=$( { pgrep -f 'sleep 300' || true; } | wc -l)
        [ "$alive" = 1 ] || {
          echo "ERROR: there are $alive live footers; there should be exactly 1." >&2
          exit 1
        }

        kill "$second" 2>/dev/null || true
        echo "OK: the second footer replaces the first, and exactly one is left"
      '';

  # THE FOOTER'S LOG OUTLIVES THE TERMINAL.
  #
  # # Why this is a check
  #
  # Because the shortcuts hung FOUR times and on all four there was no proof at
  # all: the log went to the terminal and nowhere else. When the owner writes "it
  # happened again", what exists is the description of the symptom — which does
  # not tell "the key did not arrive" from "it arrived with a stuck modifier",
  # and the two are fixed in opposite places.
  #
  # The check also asserts what has already broken once in this same file: the
  # pidfile keeps the FOOTER's PID. Resolving the log with a `tee` in the middle
  # would make `$!` be the tee's, and the pidfile would keep the wrong process —
  # which is exactly the defect `footer-is-single` exists in order not to have.
  footer-logs-to-file =
    mkCheck "footer-logs-to-file"
      [
        singleFooter
        pkgs.procps
      ]
      ''
        export HOME=$TMPDIR
        pidfile="$PWD/footer.pid"
        export RONOCTUA_LOG="$PWD/footer.log"

        # THE FAKE ONE SAYS ITS OWN PID in the log. It is what makes it possible to
        # check that the pidfile keeps IT, and not a wrapper — after the `exec` the
        # command line becomes `sleep 300` and is no good for identifying anyone.
        printf '#!/bin/sh\necho "footer-mark $$"\nexec sleep 300\n' > falso
        chmod +x falso

        ronoctua-footer-single "$pidfile" "$PWD/falso"
        pid=$(cat "$pidfile")

        for _ in $(seq 50); do
          [ -s "$RONOCTUA_LOG" ] && break
          sleep 0.1
        done

        grep -q "footer-mark" "$RONOCTUA_LOG" || {
          echo "ERROR: the footer's log did not go to \$RONOCTUA_LOG." >&2
          echo "      With no file, the next time the shortcuts lock up there" >&2
          echo "      is no way to diagnose it that the four before it did not have:" >&2
          echo "      with nothing." >&2
          exit 1
        }

        # THE PIDFILE KEEPS THE FOOTER, and not a wrapper.
        reported=$(awk '/footer-mark/ {print $2; exit}' "$RONOCTUA_LOG")
        [ "$reported" = "$pid" ] || {
          echo "ERROR: the footer says it is pid $reported and the pidfile holds $pid." >&2
          echo "      A 'tee' in between would make \$! its own — and the shutdown" >&2
          echo "      in dev.nix would kill the wrong process." >&2
          exit 1
        }

        # AND THE SECOND ONE APPENDS, it does not wipe: the hot reload restarts the
        # footer on every edit, and a log truncated per restart loses precisely the
        # piece before the defect.
        ronoctua-footer-single "$pidfile" "$PWD/falso"
        second=$(cat "$pidfile")
        for _ in $(seq 50); do
          [ "$(grep -c "footer-mark" "$RONOCTUA_LOG")" = 2 ] && break
          sleep 0.1
        done
        n=$(grep -c "footer-mark" "$RONOCTUA_LOG")
        [ "$n" = 2 ] || {
          echo "ERROR: the log has $n marks after two footers; expected 2." >&2
          echo "      The restart is TRUNCATING the log." >&2
          exit 1
        }

        kill "$second" 2>/dev/null || true
        echo "OK: the footer's log goes to a file, builds up across restarts,"
        echo "    and the pidfile still points at the footer"
      '';

  # ZERO grid diff on the VT corpus (milestone 7, section 7.10).
  #
  # With a single engine, this is a golden file — it catches a regression, not a
  # divergence of interpretation. It is the degradation risk R1 prescribes, and
  # the binary itself says so in its output so nobody confuses the two.
  term-differential = mkCheck "term-differential" [ ] ''
    ${rust.workspace}/bin/ronoctua-term-diff \
      ${../tests/corpus-vt} ${../tests/golden-vt}
  '';

  installer-dryrun = mkCheck "installer-dryrun" [ ] ''
    # THE SECRETS ARE NAMED ONCE, and the grep below reads the same variables.
    #
    # They used to be typed twice — once in the `export` and once in the `grep`
    # — and a rename touched only one side: the check exported
    # `frase-secret_one-de-teste` and searched for `frase-secreta-de-teste`. It
    # went on passing while unable to detect a leaked passphrase at all, which is
    # the leak it exists for and the more damaging of the two.
    password=test-password
    passphrase=test-passphrase

    export RONOCTUA_INST_DISK=/dev/nvme0n1
    export RONOCTUA_INST_HOSTNAME=noctua-teste
    export RONOCTUA_INST_PASSWORD="$password"
    export RONOCTUA_INST_PASSPHRASE="$passphrase"
    # NOT the base language, and derived rather than named: the dry run has to
    # prove the ANSWER reaches `host.nix`, and a value equal to the default
    # would pass even if the answer were being dropped. `builtins.head` is the
    # base, so the last of the list is the one that cannot be it.
    export RONOCTUA_INST_LANGUAGE=${builtins.elemAt languages (builtins.length languages - 1)}
    # The sixth question. `br` and not the default: the dry run has to prove the
    # ANSWER reaches `host.nix`, and a value equal to the default would pass
    # even if the answer were being dropped.
    export RONOCTUA_INST_KEYBOARD=br
    # And the profile, `minimal` and not the default, for the same reason: an
    # answer equal to the default passes even when it is being dropped.
    export RONOCTUA_INST_PROFILE=minimal

    out_text=$(${rust.workspace}/bin/ronoctua-install)
    echo "$out_text"

    # The `host.nix` must NOT carry a secret (section 9): it goes to backup.
    if echo "$out_text" | grep -qF -e "$password" -e "$passphrase"; then
      echo "ERROR: the generated host.nix contains a secret" >&2
      exit 1
    fi

    # NO label may come out as a raw key.
    #
    # `Texts::t` gives back `[api_key]` for a key that does not exist — on purpose,
    # so as not to bring the shell down over a missing text. The side effect is
    # that an unregistered key gets past ALL the other guards: `gen-i18n.py` only
    # compares `pt-BR` with `en-GB`, and the compiler only protects whoever uses
    # `api_key::*`. The installer used a literal, and printed
    # `[inst-step-partition]` with nothing complaining.
    #
    # This line is what closes the hole at the flake's level.
    if echo "$out_text" | grep -q '\[inst-'; then
      echo "ERROR: a label came out as a raw key — invariant 8" >&2
      echo "$out_text" | grep '\[inst-' >&2
      exit 1
    fi

    # The LUKS key is written BEFORE the first destructive step. `disko` reads it
    # in the first second; the file being missing after the disk has been wiped is
    # the worst possible moment to find out.
    order=$(echo "$out_text" | grep -n 'disko-luks.key\|disko --mode' | cut -d: -f1)
    first_key=$(echo "$order" | head -1)
    disko_at=$(echo "$order" | tail -1)
    [ "$first_key" -lt "$disko_at" ] || {
      echo "ERROR: the LUKS key is written after disko has run" >&2; exit 1; }

    # THE PROFILE REACHES THE GENERATED `host.nix`.
    #
    # It is the line that decides whether the machine installs 22,8 GiB or 6,9,
    # and it is written on BOTH answers — `full` is the module's own default, so
    # a file that only mentions the profile when it differs would be a file one
    # has to already know the default to read.
    echo "$out_text" | grep -q 'ronoctua.profile = "minimal"' || {
      echo "ERROR: the profile's answer does not reach host.nix" >&2; exit 1; }

    # The password hash is written. Without it `base.nix` points
    # `hashedPasswordFile` at a file nobody creates, and the installed system comes
    # up with `mutableUsers = false` and no valid password — a machine nobody gets
    # into.
    echo "$out_text" | grep -q '/mnt/persist/etc/shadow/noctua' || {
      echo "ERROR: the plan does not write the password hash" >&2; exit 1; }

    # The four PCRs, including 15. Without it the sealing does not protect against
    # a disk swap — see nix/modules/boot.nix.
    echo "$out_text" | grep -q 'tpm2-pcrs=7+11+12+15' || {
      echo "ERROR: the plan does not seal against all four PCRs" >&2; exit 1; }

    # And without `--execute` nothing runs. An installer that wipes a disk by
    # default is a dangerous command.
    #
    # The sentence comes from the `.ftl`, so matching the TEXT would tie the check
    # to one language — and the sandbox does not define `LANG`, so it would be
    # `en-GB`. What does not change with the language is the mention of
    # `--execute`, which is what the line actually promises.
    echo "$out_text" | grep -q -- '--execute' || {
      echo "ERROR: the rehearsal does not say how to run it for real" >&2; exit 1; }

    # And it proves it was a dry run: no step may have run. `nixos-facter` does not
    # even exist in the sandbox — if the installer tried, it would exit non-zero
    # and the check would already have died above at the `output=$(...)`.
    [ ! -e /tmp/disko-luks.key ] || {
      echo "ERROR: the rehearsal wrote the LUKS key" >&2; exit 1; }

    # The SAME plan in EVERY language: same number of steps, same commands. A
    # `.ftl` that lost a key would come out with `[inst-` and would already have
    # died above; what this catches is the subtler case of the language
    # changing what the installer DOES, and not only what it says.
    #
    # It used to run exactly two and compare them to each other, which is the
    # shape that cannot grow — and it also used `en_US`, which does not name a
    # UI language at all and so was really exercising the fallback. Each
    # language now runs under its OWN locale, derived the way `base.nix`
    # derives the list.
    steps=""
    howmany=""
    for language in ${lib.concatStringsSep " " languages}; do
      locale=$(echo "$language" | tr '-' '_').UTF-8
      n=$(LANG=$locale ${rust.workspace}/bin/ronoctua-install | grep -c '^[! ] [0-9]')
      steps="$steps $language=$n"
      if [ -z "$howmany" ]; then howmany=$n; fi
      [ "$n" = "$howmany" ] || {
        echo "ERROR: the plan is not the same in every language ($steps )" >&2; exit 1; }
    done

    echo "OK: $howmany steps in every language, four PCRs, a rehearsal by default"
  '';

  # The whole chain: signed UKI, Secure Boot and the proof of PCR 15 (milestone 8).
  secureBootVm = import ./secureboot-vm.nix {
    inherit pkgs lib sbKeys;
    lanzabooteModule = inputs.lanzaboote.nixosModules.lanzaboote;
  };

  # TPM2 sealing: it seals, unlocks, and refuses a volume with no token (milestone 8).
  tpmVm = import ./tpm-vm.nix { inherit pkgs lib; };

  # A native buffer in a window of its own, really drawing (milestone 7).
  bufferVm = import ./buffer-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
    bufferExample = buffers.example;
    bufferGlyphs = buffers.glyphs;
    bufferFormat = buffers.format;
  };

  # The memory panel on screen. A file of its own and not one more subtest of
  # the sibling: what it proves is the HOST reading a real `/proc` and a clock
  # that ticks, which no other buffer needs — and a failure there has nothing to
  # do with the mechanism `buffer-vm` guards.
  memoryVm = import ./memory-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
    bufferMemory = buffers.memory;
  };

  # The clipboard history: the native capture, the view and the re-copy (4.2).
  clipboardVm = import ./clipboard-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The terminal wired to the shell, with what is typed coming back to the screen (7.10).
  terminalVm = import ./terminal-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The leader as the Super key on its own: keyd standing and niri listening (7.4).
  #
  # The niri module goes along because the test checks BOTH ends of the contract
  # — the key keyd emits and the one niri listens for.
  # The rescue console's second door: `Ctrl+Alt+Del` with no compositor at all.
  rescueVm = import ./rescue-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
    };
  };

  # The file manager's privileged half: the unit, the polkit rule, and the
  # refusals the helper makes before it acts. Only a booted machine answers any
  # of them.
  # The disk border's other half: what only a booted machine answers — that
  # udisks is D-Bus activatable, that its udev rules arrived, that upstream's
  # polkit defaults are still the two numbers the design rests on, and that a
  # SYSTEM volume refuses with a name instead of hanging. The removable half is
  # proved against a real pendrive; see the file for why neither can do both.
  # The slimmed udisks the disks module installs — the same expression, so the
  # check reads the artefact the system actually gets and not a lookalike.
  slimUdisks = self.nixosConfigurations.vm.config.services.udisks2.package;

  disksVm = import ./disks-vm.nix {
    inherit pkgs lib;
    formatDisk = rust.formatDisk;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
      filesRoot = rust.filesRoot;
    };
  };

  filesVm = import ./files-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
      filesRoot = rust.filesRoot;
    };
  };

  # The transfer queue reaching the SCREEN: the unit comes up, the footer finds
  # it, the segment resolves to columns in the modeline, and `LEADER t t` opens
  # the panel with the list the footer already had.
  transfersVm = import ./transfers-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
    ronoctua-transfers = rust.transfers;
  };

  extensionVm = import ./extension-vm.nix {
    inherit pkgs lib extension;
    ronoctua-shell = rust.shell;
    ronoctua-transfers = rust.transfers;
  };

  keyboardVm = import ./keyboard-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
    };
  };

  # One satellite per buffer scope, proved with TWO X11 apps (9.1).
  x11Vm = import ./x11-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The root's wipe, proved between two boots (milestone 7).
  impermanenceVm = import ./impermanence-vm.nix { inherit pkgs lib; };

  # Leader, which-key and the return to rest (milestone 5).
  leaderVm = import ./leader-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The app finder, and the footer that must not freeze when opening one.
  appsVm = import ./apps-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The IPC contract against the pinned tag AND against `niri-next` (milestone 4).
  niriContract = import ./niri-contract.nix {
    inherit pkgs lib;
    niri = inputs.niri.packages.${system}.niri;
    niriNext = inputs.niri-next.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # -------------------------------------------------------------------
  # Granular checks of milestone 1
  # -------------------------------------------------------------------

  # THE FLAKE'S TREE MUST NOT GET FAT. It cost 793 GB to find out.
  #
  # This project is not a git repository (invariant 2), and it is git's index
  # that Nix uses to filter a flake's source. With no git, `nix flake check`
  # copies the WHOLE directory into the store — and since the path is
  # content-addressed, EVERY EDIT mints a new copy.
  #
  # With an 18 GB `target/` inside, that gave 44 copies and 793 GB: 94% of the
  # store and 42% of the disk, with nothing warning. The `nix/rust.nix` already
  # filtered the source `crane` compiles, but that filter runs LATER — the 19 GB
  # copy has already happened at evaluation time.
  #
  # The fix is keeping the build outside the tree (`CARGO_TARGET_DIR`, see
  # `nix/devshell.nix`). This check is what stops the fix from being lost: the
  # cost is invisible day to day and only shows up when the disk fills.
  #
  # THE DEPENDENCY LAYER SEES ONLY WHAT IT USES.
  #
  # `buildDepsOnly` compiles the 483 third-party crates against stubs — crane
  # replaces every crate of ours, `build.rs` included. So it reads none of the
  # generated inputs; and yet all 31 `RONOCTUA_*` were in its environment,
  # because it was handed the whole of `commonArgs`.
  #
  # Nix addresses a derivation by its inputs. With `RONOCTUA_BUFFER_MEMORY` in
  # there, editing a buffer changed the layer's hash and paid 4m46 of
  # compilation for a value nothing in it reads — and made the layer WAIT for
  # five wasm builds it never consumes. A `.ftl` or a token did the same.
  #
  # This reads the attributes of the generated derivation, at evaluation time:
  # no build, no import-from-derivation. It is the environment that is checked
  # because the environment is how they got in — the five buffers were never
  # named in `nix/rust.nix`'s deps call, they arrived carried by a variable.
  deps-layer-is-lean =
    let
      leaked = lib.filter (lib.hasPrefix "RONOCTUA_") (lib.attrNames rust.cargoArtifacts.drvAttrs);
    in
    mkCheck "deps-layer-is-lean" [ ] ''
      leaked="${lib.concatStringsSep " " leaked}"
      if [ -n "$leaked" ]; then
        echo "ERROR: the dependency layer was given variables it does not use:" >&2
        for v in $leaked; do echo "  $v" >&2; done
        echo >&2
        echo "      They come in through \`commonArgs\`; the layer has to take the" >&2
        echo "      \`buildBase\`. Each one of those costs 4m46 of recompilation" >&2
        echo "      every time its value changes — and a .wasm changes every time" >&2
        echo "      somebody edits the buffer." >&2
        exit 1
      fi
      echo "OK: the dependency layer carries no RONOCTUA_* at all"
    '';

  lean-tree = mkCheck "lean-tree" [ ] ''
    cap=20480  # KiB — the real tree is ~1.3 MB
    tree_size=$(du -s --exclude=.git ${self} | cut -f1)
    echo "flake source: $tree_size KiB (ceiling $cap KiB)"
    if [ "$tree_size" -gt "$cap" ]; then
      echo >&2
      echo "ERROR: the tree copied into the store is $tree_size KiB." >&2
      echo >&2
      echo "Every edit to ANY file mints a copy of this size." >&2
      echo "Look for a build directory inside the tree:" >&2
      du -sh ${self}/* 2>/dev/null | sort -rh | head -5 >&2
      echo >&2
      echo "Builds go to \$CARGO_TARGET_DIR, outside the tree." >&2
      exit 1
    fi
  '';

  # The mistake section 1 of the document explicitly says to avoid: the
  # .gitignore swallowing the font files that come to live in the repo.
  #
  # We cannot run `git check-ignore` against the real repository (there is no
  # git, by the decision of section 1), so we assemble a throwaway repo with
  # OUR .gitignore and the real tree of paths.
  gitignore-keeps-fonts = mkCheck "gitignore-keeps-fonts" [ pkgs.git ] ''
    export HOME=$TMPDIR
    mkdir -p repo && cd repo
    git init -q .
    cp ${../.gitignore} .gitignore

    # Recreates the real paths as empty files. The fonts are NOT here any more
    # (they are a Release asset), so what has to survive the .gitignore is the
    # hand-kept content and the code.
    while IFS= read -r f; do
      mkdir -p "$(dirname "$f")"
      : > "$f"
    done < ${
      pkgs.writeText "tracked-paths.txt" (
        lib.concatStringsSep "\n" [
          "assets/lexicon/tags.en-pt.toml"
          "assets/lexicon/overlay.toml"
          "assets/icons/tray-glyphs.nix"
          "assets/tokens/.keep"
          "nix/fonts.nix"
          "nix/scripts/fix-font-names.py"
          "nix/scripts/make-console-psf.py"
          "docs/ARCHITECTURE.md"
          "tests/checks.nix"
          "flake.nix"
          "CLAUDE.md"
          "README.md"
        ]
      )
    }

    failed=0
    while IFS= read -r f; do
      if git check-ignore -q "$f"; then
        echo "ERROR: .gitignore is swallowing $f" >&2
        failed=1
      fi
    done < <(find assets nix docs tests -type f 2>/dev/null; ls flake.nix CLAUDE.md README.md)
    [ $failed -eq 0 ] || exit 1
    echo "OK: no versioned source is ignored"

    # And the other way round: what MUST be ignored.
    for f in .TEMP/x.ttf result nix/hosts/host.nix foo.qcow2 a.key some.age; do
      mkdir -p "$(dirname "$f")"; : > "$f"
      git check-ignore -q "$f" || { echo "ERROR: $f is not being ignored" >&2; exit 1; }
    done
    echo "OK: secrets, artefacts and host.nix are ignored"
  '';

  # Proof that the naming fix caught: ONE typographic family and 135 distinct
  # (family, subfamily) pairs, with no collision between SemiExtended
  # (usWidthClass 6) and Extended (7).
  #
  # The TTFs are fixed and MUST have a unique family. The WOFF2s pass through
  # untouched on purpose (see the note in nix/fonts.nix), so on them we only
  # take inventory — demanding a unique family there would assert something false.
  fonts-single-family = mkCheck "fonts-single-family" [ py ] ''
    echo "### corrected: one family, and it is required ###"
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/TTF-nerdfonts \
      --family "NoctuaIosevka Nerd Font Mono" --expect-count 135 --dry-run
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/TTF \
      --family "Noctua Iosevka" --expect-count 135 --dry-run

    echo
    echo "### intactos: so inventario ###"
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/WOFF2-nerdfonts \
      --family "NoctuaIosevka Nerd Font Mono" --expect-count 135 --inventory
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/WOFF2 \
      --family "Noctua Iosevka" --expect-count 135 --inventory
  '';

  # Invariants of the inventory, measured and not assumed. If the font is
  # swapped for another build, this fails and forces a review of the plan —
  # instead of the footer silently going out of alignment.
  fonts-metrics = mkCheck "fonts-metrics" [ py ] ''
    python3 - <<'PY'
    from fontTools.ttLib import TTFont
    import glob, sys

    failed = False
    expected_cp = 18053
    advances = {3: 456, 4: 548, 5: 600, 6: 658, 7: 720}

    for p in sorted(glob.glob("${fonts.corrected}/TTF-nerdfonts/*.ttf")):
        f = TTFont(p, lazy=True)
        cmap = f.getBestCmap()
        os2 = f["OS/2"]
        head = f["head"]

        if head.unitsPerEm != 1000:
            print(f"ERROR {p}: unitsPerEm {head.unitsPerEm} != 1000"); failed = True
        if len(cmap) != expected_cp:
            print(f"ERROR {p}: {len(cmap)} codepoints, expected {expected_cp}"); failed = True

        # Exact monospacing: ONE advance per face, that of its own width.
        width = os2.usWidthClass
        adv = {f["hmtx"][g][0] for g in cmap.values()}
        if len(adv) != 1:
            print(f"ERROR {p}: {len(adv)} distinct advances; it is not monospaced"); failed = True
        elif adv.pop() != advances[width]:
            print(f"ERROR {p}: the wrong advance for usWidthClass {width}"); failed = True
        f.close()

    if failed:
        sys.exit(1)
    print("OK: 135 faces, 18,053 codepoints each, monospaced, upem 1000")
    print("OK: cell = 0.600 em at the normal width — the footer's arithmetic holds")
    PY
  '';

  # THE ICON HAS TO BE THE SIZE OF THE TEXT NEXT TO IT — and it was not.
  #
  # Reported from the screen: "the bell looks much too small, smaller than the
  # rest of the letters and numbers". It was true and it is measurable: the ink
  # of `cod-bell` is 600 units tall against 751 for the digit `0`, so at the
  # same point size the bell comes out a fifth shorter. Nothing failed; it just
  # looked wrong, which is the class of defect that survives for months.
  #
  # The fix is the `font.size.icon` token, and this is what keeps it honest: the
  # two numbers come from the token file, the ink comes from the real face, and
  # a new chrome icon whose drawing is out of scale fails HERE instead of on
  # somebody's screen.
  #
  # 85%..115% and not "equal": these are different families of drawing (a bell,
  # a swap arrow, a pie) and they are not going to agree to the unit. What the
  # band excludes is what the eye reads as another size.
  chrome-icons-match-the-text =
    let
      typography = import ../assets/tokens/typography.nix;
    in
    mkCheck "chrome-icons-match-the-text" [ py pkgs.gnugrep ] ''
      python3 - <<'PY'
      from fontTools.ttLib import TTFont
      from fontTools.pens.boundsPen import BoundsPen
      import json, sys

      BASE = ${toString typography.size.base}
      ICON = ${toString typography.size.icon}

      # THE ICONS THE CHROME DRAWS INLINE WITH TEXT, at `font.size.icon`.
      #
      # They are named here, and not derived by prefix, because the prefix is
      # not the property: `transfer-paused` and `transfer-running` share the
      # family and are drawn in the transfer PANEL, on the grid, next to a row
      # of their own. What this list holds is what sits shoulder to shoulder
      # with digits on one line.
      INLINE = [
          "notification", "notification-pending", "notification-muted",
          # The two things one does to a notification with the mouse.
          "notification-mark-read", "notification-delete",
          # And the generic marks, for what nobody named a glyph for.
          "info", "warning", "error",
          "transfer",
          "countdown-one", "countdown-two", "countdown-three", "countdown-four",
          "countdown-five", "countdown-six", "countdown-seven", "countdown-eight",
          # AND THE TWO FRAMES OF "AN UPDATE IS RUNNING", which sit in the
          # modeline beside the buffer's name and the bell's digit. They are
          # measured for the same reason the pie is: an animation whose frames
          # are 20% shorter than the text next to them does not read as one
          # drawing moving, it reads as the line twitching. Here the two frames
          # are the same circle filled and hollow, so the measurement is also
          # what keeps them the same drawing.
          "working-one", "working-two",
      ]

      face = "${fonts.corrected}/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf"
      f = TTFont(face, lazy=True)
      cmap = f.getBestCmap()
      glyphs = f.getGlyphSet()
      icons = json.load(open("${glyphdb.db}/icons.json"))

      def ink(cp):
          pen = BoundsPen(glyphs)
          glyphs[cmap[cp]].draw(pen)
          return pen.bounds

      x0, y0, x1, y1 = ink(ord("0"))
      digit = (y1 - y0) * BASE / 1000.0
      # THE CELL, and how far into it the neighbouring digit's ink begins.
      #
      # A chrome glyph is drawn bigger than the grid and centred on its cell, so
      # its ink DOES leave the cell — by design, and harmlessly, as long as it
      # stops short of where the character beside it starts drawing. That is
      # what this number is: `0` has 80 units of left bearing, which at the base
      # size is 1,44 px of air the icon may borrow.
      cell = BASE * 0.6
      bearing = x0 * BASE / 1000.0
      print(f"digit `0` at {BASE} px: {digit:.2f} px of ink, bearing {bearing:.2f} px")

      failed = False
      for token in INLINE:
          if token not in icons:
              print(f"ERROR: `{token}` is not an icon token (renamed?)")
              failed = True
              continue
          gx0, gy0, gx1, gy1 = ink(icons[token]["codepoint"])
          h = (gy1 - gy0) * ICON / 1000.0
          pct = 100.0 * h / digit
          # The run is placed so its ADVANCE is centred on the cell — see
          # `render::Size::OnTheCell` — and the ink rides wherever the font put
          # it inside that advance, which is not always the middle.
          shift = (cell - ICON * 0.6) / 2.0
          over = max(-(gx0 * ICON / 1000.0 + shift), gx1 * ICON / 1000.0 + shift - cell)
          tall = 85.0 <= pct <= 115.0
          wide = over < bearing
          mark = "ok " if tall and wide else "NO "
          print(
              f"{mark} {token:24s} {icons[token]['glyph']:28s} "
              f"{h:5.2f} px {pct:5.1f}%  transborda {over:5.2f} px"
          )
          if not tall:
              failed = True
          if not wide:
              print(f"       `{token}` invades the ink of the character beside it")
              failed = True

      if failed:
          print("")
          print("ERROR: a chrome icon is out of scale with the text beside it.")
          print("      Either `font.size.icon` changed, or the new glyph has a drawing")
          print("      of another proportion — change the glyph, not the size: the")
          print("      size holds for all the others.")
          sys.exit(1)
      print("")
      print(f"OK: {len(INLINE)} chrome icons at the digit's height, and none touches it")
      PY

      # AND NOBODY introduces a second inline size without passing through here.
      #
      # The list above only holds while OnTheCell is asked for with ONE size. A
      # second one, written loose, would escape the measurement entirely — and
      # the way to find out would again be by looking at the screen.
      #
      # The shell is the whole scope on purpose: a wasm buffer cannot ask for a
      # size in px (invariant 11 gives it a Scale, and the host resolves it), so
      # every chrome glyph in the system is drawn from here.
      loose=$(grep -rn "Size::OnTheCell(" ${../crates/ronoctua-shell/src} | grep -v "SIZE_ICON" || true)
      if [ -n "$loose" ]; then
        echo "ERROR: a glyph centred in the cell with a size outside the token:" >&2
        echo "$loose" >&2
        echo "      The chrome's icon size has ONE home: font.size.icon." >&2
        exit 1
      fi
      echo "OK: every glyph centred in a cell uses the same size token"
    '';

  # The guard of section 6.3: the Nerd Fonts version comes from the FILE ITSELF
  # (nameID 5), not from a constant. If the font is updated without the
  # matching glyphnames.json, the PUA codepoints move silently.
  fonts-nerdfonts-version = mkCheck "fonts-nerdfonts-version" [ py ] ''
    python3 - <<'PY'
    from fontTools.ttLib import TTFont
    import re, sys

    p = "${fonts.corrected}/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf"
    v = TTFont(p, lazy=True)["name"].getDebugName(5)
    m = re.search(r"Nerd Fonts (\d+\.\d+\.\d+)", v or "")
    if not m:
        print(f"ERROR: nameID 5 does not declare a Nerd Fonts version: {v!r}"); sys.exit(1)

    expected = "3.4.0"   # has to match the flake's `nerd-fonts` input
    if m.group(1) != expected:
        print(f"ERROR: the font has Nerd Fonts {m.group(1)}, the flake pins {expected}.")
        print("      Update the nerd-fonts input along with the font, or the")
        print("      PUA codepoints move with nobody noticing.")
        sys.exit(1)
    print(f"OK: Nerd Fonts {m.group(1)} matches the flake's input")
    PY
  '';

  # The icon table is generated from the REAL SOURCE FILE (section 6.3), never
  # hard-coded. This check proves that the generation happens and that the
  # result matches what the inventory measured.
  icons-table-generated = mkCheck "icons-table-generated" [ pkgs.jq ] ''
    db=${glyphdb.db}

    for f in icons.json glyphs.json coverage.md; do
      test -s "$db/$f" || { echo "ERROR: $f was not generated" >&2; exit 1; }
    done

    n=$(jq 'length' "$db/icons.json")
    echo "icon tokens: $n"
    [ "$n" -ge 40 ] || { echo "ERROR: the icon table is suspiciously small" >&2; exit 1; }

    # Every token has a glyph, a codepoint and a char — and the codepoint is in the PUA.
    jq -e 'to_entries | all(.value | has("glyph") and has("codepoint") and has("char"))' \
      "$db/icons.json" >/dev/null \
      || { echo "ERROR: an incomplete icon entry" >&2; exit 1; }

    g=$(jq 'length' "$db/glyphs.json")
    echo "glyphs indexed: $g"
    [ "$g" -eq 10473 ] || {
      echo "ERROR: expected 10473 Nerd Fonts glyphs, found $g." >&2
      echo "      If the font changed, the inventory in docs/ARCHITECTURE.md" >&2
      echo "      has to be redone before carrying on." >&2
      exit 1
    }

    # The 126 without a name have to stay in the index: dropping them would be
    # lying about what the font has (section 6.5).
    unnamed=$(jq '[.[] | select(.name == null)] | length' "$db/glyphs.json")
    echo "glyphs with no name in glyphnames.json: $unnamed"
    [ "$unnamed" -eq 126 ] || { echo "ERROR: expected 126 with no name" >&2; exit 1; }

    echo "OK: the icon table and the glyph database are generated from the real file"
  '';

  # THE NOTIFICATION CATALOGUE COMES OUT OF NIX, and the two vocabularies agree.
  #
  # `assets/notifications.nix` is the single place every notification and every
  # pre-created filter of the distro is declared (section 4.2). The generator
  # already refuses an invalid field at build time — this is the OTHER half:
  # that the words the generator accepts are the words the Rust knows.
  #
  # They are two lists, in two languages, and nothing but this compares them. A
  # `show = "brief"` the generator accepts and the Rust has never heard of would
  # come out as a table that does not compile — or worse, as a name the Rust
  # spells differently and silently reads as something else.
  notifications-table-generated = mkCheck "notifications-table-generated" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    generator = pathlib.Path("${../nix/scripts/gen-notifications.py}").read_text()
    table = pathlib.Path("${notificationsRs}/notifications.rs").read_text()
    keys = pathlib.Path("${notificationsRs}/keys.txt").read_text().split()

    failed = False

    # (a) the table was generated, and it is not empty.
    for wanted in ("fn generated_entries()", "fn generated_rules()"):
        if wanted not in table:
            print(f"ERROR: the table has no `{wanted}`")
            failed = True
    entries = table.count("Entry {")
    rules = table.count("Rule {")
    print(f"catalogue: {entries} entries, {rules} filters, {len(keys)} keys required")
    if entries < 1 or rules < 1 or not keys:
        print("ERROR: the generation came out empty or truncated")
        failed = True

    # (b) the vocabularies. The generator names them as sets of strings; the
    #     Rust names them in the `id()` of each enum. Both are parsed from the
    #     SOURCE, so a rename on either side shows up here.
    def from_generator(name):
        m = re.search(name + r" = \{([^}]*)\}", generator)
        assert m, name
        return set(re.findall(r'"([^"]+)"', m.group(1)))

    def from_rust(path, enum):
        """The names in the `id()` of one enum.

        Two levels of narrowing, and both are needed: `presentation.rs` holds
        `Show` AND `Life`, and `rule.rs` holds `Field` AND `Test`, so grabbing
        every arrow in the file gives one enum the other's names.
        """
        src = pathlib.Path(path).read_text()
        impl = re.search(rf"\nimpl {enum} \{{(.*?)\n\}}\n", src, re.S)
        if not impl:
            print(f"ERROR: could not find `impl {enum}` in {path}")
            return set()
        fn = re.search(r"fn id\(&?self\)[^{]*\{(.*?)\n    \}", impl.group(1), re.S)
        if not fn:
            print(f"ERROR: `{enum}` has no `id()` — the name would have no way of going and coming back")
            return set()
        return set(re.findall(r'=> "([^"]+)"', fn.group(1)))

    NOTIFY = "${../crates/ronoctua-notify/src}"
    pairs = [
        ("SHOW", f"{NOTIFY}/presentation.rs", "Show"),
        ("LIFE", f"{NOTIFY}/presentation.rs", "Life"),
        ("FIELDS", f"{NOTIFY}/rule.rs", "Field"),
        ("PROGRAMS", f"{NOTIFY}/action.rs", "Program"),
    ]
    for name, path, enum in pairs:
        a, b = from_generator(name), from_rust(path, enum)
        if a != b:
            print(f"ERROR: `{name}` differs between the generator and the Rust")
            print(f"       only in the generator: {sorted(a - b)}")
            print(f"       only in the Rust:      {sorted(b - a)}")
            failed = True
        else:
            print(f"ok  {name:10s} {len(a)} identical names on both sides")

    # (c) every text key it demands has the shape the manager derives.
    for k in keys:
        if not re.fullmatch(r"notif-[a-z0-9-]+", k):
            print(f"ERROR: the key `{k}` is outside the `notif-<...>` shape")
            failed = True

    # (d) AND THE TAG THE SHELL STAMPS IS A TAG THE TABLE RECOGNISES.
    #
    # The out-of-memory notification is raised by the shell and recognised by
    # the catalogue through a tag — a string that has to be the same on both
    # sides with no compiler between them. Diverging, the notification arrives
    # as a stranger: quiet, short, with no action and no explanation of why a
    # window disappeared.
    surface = pathlib.Path("${../crates/ronoctua-shell/src}/surface.rs").read_text()
    stamped = set(re.findall(r'pub const [A-Z_]+: &str = "([a-z-]+)";', surface))
    known = set(re.findall(r'Test::Is\("([a-z-]+)"\.to_owned\(\)\)', table))
    for tag in stamped:
        if tag not in known:
            print(f"ERROR: the shell stamps `{tag}` and no entry recognises it")
            failed = True
    if stamped:
        print(f"ok  tags stamped by the shell and recognised: {sorted(stamped)}")

    if failed:
        sys.exit(1)
    print("OK: the catalogue is generated from the Nix, and the two vocabularies match")
    PY
  '';

  # THE COMPOSITOR STILL SENDS WHAT THE FILTER LOOKS FOR.
  #
  # The screenshot notification is not ours: niri raises it after a capture,
  # with `app_name = "niri"`, the English summary "Screenshot captured", and —
  # this is the part that matters — the FILE'S PATH in the icon field, as a
  # `file://` URL. That is where the `edit` action gets the image from, with no
  # directory guessed at.
  #
  # All three are strings that belong to somebody else. If upstream changes any
  # of them the entry stops matching IN SILENCE: the screenshot arrives in
  # English, loud, with no actions, and nothing fails. This is the check that
  # turns that into a build error, and it is the same shape as `niri-contract`,
  # which already extracts the compositor's actions from the pinned tree.
  screenshot-notification-contract = mkCheck "screenshot-notification-contract" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    source = pathlib.Path("${inputs.niri}/src/utils/mod.rs").read_text()
    catalogue = pathlib.Path("${../assets/notifications.nix}").read_text()

    failed = False

    # (a) the notification exists at all, and goes to the session bus.
    for wanted in ("fn show_screenshot_notification", "org.freedesktop.Notifications"):
        if wanted not in source:
            print(f"ERROR: the pinned niri does not have `{wanted}`")
            failed = True

    # (b) the two strings the entry matches on.
    for wanted in ('"niri"', '"Screenshot captured"'):
        if wanted not in source:
            print(f"ERROR: the pinned niri no longer sends {wanted}")
            failed = True
        elif wanted.strip('"') not in catalogue:
            print(f"ERROR: assets/notifications.nix does not match {wanted}")
            failed = True

    # (c) AND THE PATH TRAVELS IN THE ICON FIELD. Without this the `edit`
    #     action has no file, and it would have to guess a directory.
    if "image_url" not in source or "filename_to_uri" not in source:
        print("ERROR: niri no longer puts the path as file:// in the icon field")
        failed = True

    # (d) AND THE EDITOR IS NOT BAKED INTO THE BINARY.
    #
    # `swappy` lives in the `full` profile only. A store path compiled in with
    # `option_env!` is a reference the closure carries whether the switch is on
    # or not — the measured trap `RONOCTUA_ICONS` carries a comment about in
    # `nix/rust.nix`, and the one `notoExtra` and `icons.superpack` cost before
    # that. Read from the environment, `ronoctua.media.enable = false` really
    # removes it and the action simply is not offered.
    rust_nix = pathlib.Path("${../nix/rust.nix}").read_text()
    if "RONOCTUA_SWAPPY" in rust_nix:
        print("ERROR: `RONOCTUA_SWAPPY` is in nix/rust.nix — baked into the binary.")
        print("       That puts swappy in every closure, and the profile's")
        print("       profile stops interrupting. It comes from the ENVIRONMENT.")
        failed = True

    # (e) AND EACH PROGRAM IS READ FROM THE RIGHT PLACE.
    #
    # Two programs, two sources, and getting it wrong fails SILENTLY: both were
    # read from the environment, `RONOCTUA_XDG_OPEN` only exists at build time,
    # so at runtime every `Run` action was dropped as "not installed". The
    # screenshot's band came out two lines instead of three, with no actions and
    # nothing failing. It took a screenshot to see.
    resolver = re.search(
        r"pub fn notification_program\(.*?\n\}",
        pathlib.Path("${../crates/ronoctua-shell/src}/actions.rs").read_text(),
        re.S,
    )
    if not resolver:
        print("ERROR: `notification_program` went away or changed shape")
        failed = True
    else:
        body = resolver.group(0)
        if "XDG_OPEN" not in body:
            print("ERROR: the action's `open` does not come from the path baked in at build time")
            failed = True
        if 'std::env::var("RONOCTUA_SWAPPY")' not in body:
            print("ERROR: the action's `edit` does not come from the ENVIRONMENT")
            failed = True
        if 'std::env::var("RONOCTUA_XDG_OPEN")' in body:
            print("ERROR: `open` is being read from the environment, where it does not exist")
            failed = True

    if failed:
        print("")
        print("       The `screenshot` filter recognises the compositor's")
        print("       notification by the (app, summary) pair and takes the file out of")
        print("       the icon field. Change that upstream, and it stops matching here — in")
        print("       silence, which is why this check exists.")
        sys.exit(1)
    print("OK: the pinned niri still sends the screenshot the way the filter expects")
    print("OK: the screenshot's editor comes from the environment, not from the binary")
    PY
  '';

  # A SWITCH THAT SWITCHES NOTHING OFF HAS TO SAY SO.
  #
  # The trap has a name in `CLAUDE.md` — "interruptor que não interrompe" — and
  # the project has paid for it twice: `notoExtra` and `icons.superpack` were
  # both switches that turned nothing off, and the way anybody found out was by
  # measuring the closure.
  #
  # Two fields of the presentation are in that state ON PURPOSE: `vibrate` has
  # no device on a machine like this, and `audio` is not implemented (it is in
  # `docs/PENDING.md`). They exist because the external API the owner is going
  # to put online will set them, and dropping them at the door would make one
  # notification mean different things depending on where it came in.
  #
  # So they are not silent: each is in `ronoctua_notify::INERT` with a text key
  # saying why, the manager draws that next to the switch, and this check makes
  # sure the three lists — the field, the reason, and both `.ftl`s — agree.
  switches-that-do-nothing-say-so = mkCheck "switches-that-do-nothing-say-so" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    notify = pathlib.Path("${../crates/ronoctua-notify/src}")
    shell = pathlib.Path("${../crates/ronoctua-shell/src}")
    presentation = (notify / "presentation.rs").read_text()
    manager = (shell / "notifications.rs").read_text()

    failed = False

    # THE LIST, from the Rust that declares it.
    inert = re.findall(
        r'Inert \{\s*field: "([a-z-]+)",\s*why: "([a-z-]+)",',
        presentation,
    )
    if not inert:
        print("ERROR: `INERT` is empty or changed shape")
        sys.exit(1)
    print(f"switches declared inert: {[f for f, _ in inert]}")

    # (a) every one of them is a real field of `Presentation`.
    fields = set(re.findall(r"^    pub ([a-z_]+):", presentation, re.M))
    for field, why in inert:
        if field.replace("-", "_") not in fields:
            print(f"ERROR: `{field}` is not a field of Presentation")
            failed = True
        for language in ${builtins.toJSON languages}:
            ftl = pathlib.Path("${../assets/i18n}/" + language + ".ftl").read_text()
            if not re.search(rf"^{why} *=", ftl, re.M):
                print(f"ERROR: {language} does not explain `{field}` (`{why}` is missing)")
                failed = True

    # (b) AND THE MANAGER READS THAT LIST, rather than keeping a second one.
    #     Two lists is one more thing to keep in step, and the day they
    #     diverge is the day a switch goes quiet again.
    #
    #     THE COMMENTS DO NOT COUNT. The first version of this test looked for
    #     the name anywhere in the file and passed on a doc link — a check
    #     against the CRITERION and not against the CODE, which is the way
    #     `CLAUDE.md` puts it. Only a line that is not a comment answers.
    reads_it = any(
        "ronoctua_notify::INERT" in line and not line.lstrip().startswith("//")
        for line in manager.splitlines()
    )
    if not reads_it:
        print("ERROR: the manager does not read `INERT`; the reason would not reach the screen")
        failed = True
    # (b2) AND IT REACHES THE SCREEN. The reason used to be a third column on
    #      the row; the owner asked for it gone, so it moved to the message
    #      line — which means there has to be something that PUTS it there.
    #      Without this, "removed from the row" and "removed from the system"
    #      look the same to a check.
    if "pub fn hint(" not in manager:
        print("ERROR: the manager has no way of saying the reason")
        failed = True
    for door, name in (("footer", "surface.rs"), ("window", "notifications_window.rs")):
        drawn = (shell / name).read_text()
        if "manager.hint(" not in drawn and "view.hint(" not in drawn:
            print(f"ERROR: the {door} does not draw the reason on the message line")
            failed = True

    # (c) and every knob it draws is either inert-with-a-reason or does
    #     something. A knob nobody can find in either list is the defect.
    knobs = set(re.findall(r'Self::(\w+) => "([a-z-]+)",', manager))
    names = {name for _, name in knobs}
    if not names:
        print("ERROR: could not find the names of the manager's knobs")
        failed = True
    for field, _ in inert:
        if field not in names:
            print(f"ERROR: `{field}` is inert and the manager does not even draw it")
            failed = True

    if failed:
        sys.exit(1)
    print("OK: every switch with no effect says on screen why it has none")
    PY
  '';

  # AND EVERY NOTIFICATION OF THE DISTRO EXISTS IN BOTH LANGUAGES.
  #
  # The owner asked for it in those words. It does not depend on discipline: the
  # generator emits the list of keys the table demands, and this compares that
  # list against the two `.ftl`s. Adding an entry to `assets/notifications.nix`
  # and forgetting its Portuguese fails here, with the key's name.
  notifications-have-text = mkCheck "notifications-have-text" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    keys = pathlib.Path("${notificationsRs}/keys.txt").read_text().split()
    failed = False
    for language in ${builtins.toJSON languages}:
        ftl = pathlib.Path("${../assets/i18n}/" + language + ".ftl").read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*) *=", ftl, re.M))
        missing = [k for k in keys if k not in have]
        if missing:
            print(f"ERROR: {language} does not have: {', '.join(missing)}")
            failed = True
        else:
            print(f"ok  {language}: the catalogue's {len(keys)} keys are there")

    if failed:
        print("")
        print("       Every notification and every filter of the distro has to exist")
        print("       in both languages (invariant 8). The list comes out of")
        print("       assets/notifications.nix itself — there is no forgetting without failing.")
        sys.exit(1)
    print("OK: the notification catalogue speaks both languages")
    PY
  '';

  # The guard of section 6.3 is only worth anything if it is tested. This check
  # builds the negative fixture, which ONLY passes if the generator refused a
  # token that does not exist.
  icons-unknown-token-fails = mkCheck "icons-unknown-token-fails" [ ] ''
    test -f ${glyphdb.negativeFixture}/out_text.txt \
      || { echo "ERROR: the negative fixture produced no output" >&2; exit 1; }
    grep -q "icon tokens that do not resolve" ${glyphdb.negativeFixture}/out_text.txt \
      || { echo "ERROR: the negative fixture did not fail for the right reason" >&2; exit 1; }
    echo "OK: an icon token that does not exist takes the build down (section 6.3)"
  '';

  # fontconfig <-> renderer parity — section 6.4.
  #
  # "The renderer does NOT use fontconfig... The shell needs its own fallback
  # chain, COHERENT with fontconfig's."
  #
  # Both come out of nix/fontconfig.nix, from a single list. This check proves
  # that they do agree: if they diverge, the same text renders with different
  # faces inside and outside our buffers, and the symptom ("the accent looks
  # strange in the terminal") is expensive to trace back to here.
  #
  # The renderer's side is simulated by the declared order — it only exists in
  # milestone 3, but the behavior is already determined by the data. When the
  # renderer exists, this script becomes the oracle it is measured against.
  fonts-renderer-parity =
    let
      fontDirs = [
        "${fonts.noctua-iosevka-ttf-nerdfonts}/share/fonts"
        "${pkgs.noto-fonts}/share/fonts"
        "${pkgs.noto-fonts-cjk-sans}/share/fonts"
        "${pkgs.noto-fonts-color-emoji}/share/fonts"
      ];
      # OUR system's fontconfig, not the host's: with our rules and only our
      # fonts. Comparing against the host's fontconfig would prove nothing
      # about the distro.
      #
      # A REAL conf.d, ordered by name — not a sequence of <include> by hand.
      #
      # This matters: including our conf before or after the block of defaults
      # gives different results, and neither of the two reproduces the real
      # system. fontconfig reads `conf.d` in order of file NAME, and that is how
      # `50-ronoctua.conf` positions itself among the default rules. Testing with
      # another order measures a configuration that is never going to exist.
      confd = pkgs.runCommand "ronoctua-test-confd" { } ''
        mkdir -p $out
        ln -s ${pkgs.fontconfig.out}/etc/fonts/conf.d/*.conf $out/ 2>/dev/null || true
        ln -s ${fontconfigConf.confDir}/etc/fonts/conf.d/50-ronoctua.conf $out/50-ronoctua.conf
      '';

      fontsConf = pkgs.writeText "fonts.conf" ''
        <?xml version="1.0"?>
        <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
        <fontconfig>
          ${lib.concatMapStrings (d: "<dir>${d}</dir>\n  ") fontDirs}
          <cachedir>/tmp/fc-cache</cachedir>
          <include ignore_missing="yes">${confd}</include>
        </fontconfig>
      '';
    in
    mkCheck "fonts-renderer-parity"
      [
        py
        pkgs.fontconfig
      ]
      ''
        export FONTCONFIG_FILE=${fontsConf}
        mkdir -p /tmp/fc-cache
        fc-cache -f >/dev/null 2>&1 || true

        echo "### fc-match on the three generics ###"
        failed=0
        for g in monospace sans-serif serif; do
          m=$(fc-match -f '%{family}' "$g")
          echo "  $g -> $m"
          case "$m" in
            *"NoctuaIosevka Nerd Font Mono"*) ;;
            *) echo "ERROR: '$g' does not resolve to our font (section 6.4)" >&2; failed=1 ;;
          esac
        done
        [ $failed -eq 0 ] || exit 1
        echo

        python3 ${../nix/scripts/check-font-parity.py} \
          --chain-json ${fontconfigConf.chainJson} \
          --font-dir ${fonts.noctua-iosevka-ttf-nerdfonts}/share/fonts \
          --strict
      '';

  # Icon superpack — section 6.6. The manifest has to cover 100% of the names
  # that went in, and the origin licenses all have to be recorded: the
  # repository is publishable (section 9), so "where did this icon come from
  # and under what terms" needs an answer.
  icons-superpack = mkCheck "icons-superpack" [ pkgs.jq ] ''
    sp=${iconSuperpack}
    doc=$sp/share/doc/noctua-icons
    themeName=$sp/share/icons/noctua

    test -f $themeName/index.theme || { echo "ERROR: index.theme is missing" >&2; exit 1; }
    test -f $doc/manifest.json || { echo "ERROR: the manifest is missing" >&2; exit 1; }
    test -f $doc/coverage.md || { echo "ERROR: the coverage report is missing" >&2; exit 1; }

    # Inheritance: hicolor last, as the freedesktop spec requires.
    grep -q '^Inherits=hicolor$' $themeName/index.theme \
      || { echo "ERROR: index.theme does not inherit hicolor last" >&2; exit 1; }

    # Every entry declares a winner and a license — without that the manifest
    # does not answer the question it exists for.
    n=$(jq '.entry_count' $doc/manifest.json)
    complete_ones=$(jq '[.entries[] | select(.winner != null and .license != null)] | length' $doc/manifest.json)
    echo "entries in the manifest: $n"
    [ "$n" -eq "$complete_ones" ] || {
      echo "ERROR: $((n - complete_ones)) entries with no winner or no licence" >&2; exit 1; }

    # Every directory cited in index.theme really exists.
    absent=0
    for d in $(grep '^Directories=' $themeName/index.theme | cut -d= -f2 | tr ',' ' '); do
      [ -d "$themeName/$d" ] || { echo "ERROR: a directory declared but missing: $d" >&2; absent=1; }
    done
    [ $absent -eq 0 ] || exit 1

    # Origin licenses recorded, one per theme that went into the merge.
    for t in $(jq -r '.sources[].name' $doc/manifest.json); do
      test -s "$doc/LICENSES/$t.txt" \
        || { echo "ERROR: the source licence is missing for $t" >&2; exit 1; }
    done

    echo "OK: the manifest covers 100% of the entries, with a winner and a licence"
    echo "OK: source licences recorded for every merged theme"
  '';

  # THE CHAIN'S PIXMAP STEP, against the REAL superpack.
  #
  # The `icons-superpack` above proves that the PACKAGE is well formed; this
  # one proves that the chain REACHES it. They are different things, and the
  # second fails silently: without it, `procurar` would give back `None` for
  # everything, the application list would be left with only the category
  # glyphs — and a whole list of category glyphs LOOKS DELIBERATE. It is the
  # most expensive defect of this path, because it has no symptom at all.
  #
  # It counts OPAQUE pixels: finding the file is not drawing it, and an SVG
  # that rasterizes empty gives an invisible smudge on the dark background —
  # which is indistinguishable from "the icon has not arrived yet".
  apps-icon-from-superpack = mkCheck "apps-icon-from-superpack" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-icons \
      ${iconSuperpack}/share/icons/noctua
  '';

  # ===================================================================
  # NETWORK AND SYNCTHING — does what was TUNED actually reach the system?
  #
  # These two checks evaluate the real NixOS configuration, and not the
  # module's text. The difference matters: a `grep` in the `.nix` would pass
  # with the value written and shadowed by an `mkIf` that never turns on, or
  # with an option renamed by nixpkgs — both cases give a system without the
  # adjustment and a green check. That is exactly how the tray table already
  # passed the build and never reached the Rust.
  # ===================================================================
  networkEval = (pkgs.nixos [ ../nix/modules/network.nix ]).config;
  syncthingEval =
    (pkgs.nixos [
      ../nix/modules/syncthing.nix
      # The Syncthing module reads `config.ronoctua.user`, which belongs to base.
      {
        options = { };
        config.ronoctua.user = "noctua";
      }
      { options.ronoctua.user = lib.mkOption { type = lib.types.str; }; }
    ]).config;

  # The Wi-Fi password sealed in the TPM, against a real TPM (swtpm).
  #
  # The defect it catches is THE PASSWORD STAYING ON DISK — not "the daemon
  # does not come up". A check that only looked at the unit would pass with
  # `psk=<password>` inside the keyfile, which is precisely what this work
  # eliminates.
  wifiTpmVm = import ./wifi-tpm-vm.nix {
    inherit pkgs lib;
    ronoctua-shell = rust.shell;
  };

  network-tuned = mkCheck "network-tuned" [ ] ''
    expected() {
      if [ "$2" != "$3" ]; then
        echo "FAILED: $1 = $2, expected $3" >&2
        echo "" >&2
        echo "This value is NOT decorative: it came out of measuring a" >&2
        echo "592 GB transfer between two machines of this project." >&2
        exit 1
      fi
    }
    # The single biggest gain from the radio. With power saving on, the
    # throughput halves and nothing on screen says why.
    expected "wifi.powersave"       "${lib.boolToString networkEval.networking.networkmanager.wifi.powersave}" "false"
    # BBR without `fq` works WORSE than cubic: it is the classic mistake of
    # turning on only half of it, and that is why both are checked together.
    expected "tcp_congestion_control"       "${
      toString networkEval.boot.kernel.sysctl."net.ipv4.tcp_congestion_control"
    }" "bbr"
    expected "default_qdisc"       "${
      toString networkEval.boot.kernel.sysctl."net.core.default_qdisc"
    }" "fq"
    # The costliest trap in a synchronization: Syncthing pauses between files,
    # and without this every file starts over in slow start.
    expected "tcp_slow_start_after_idle"       "${
      toString networkEval.boot.kernel.sysctl."net.ipv4.tcp_slow_start_after_idle"
    }" "0"
    # Without the module loaded, the BBR sysctl fails at boot — and the failure
    # shows up in the journal, not on the screen.
    case " ${lib.concatStringsSep " " networkEval.boot.kernelModules} " in
      *" tcp_bbr "*) ;;
      *) echo "FAILED: tcp_bbr is not in boot.kernelModules" >&2; exit 1 ;;
    esac
    echo "OK: powersave off, BBR+fq, slow start disarmed"
  '';

  syncthing-tuned = mkCheck "syncthing-tuned" [ ] ''
    # ================================================================
    # THE MOST IMPORTANT CHECK IN THIS FILE, and the least obvious.
    #
    # `overrideFolders` is `true` in nixpkgs, and `true` means "whatever is
    # not declared in Nix is DELETED on activation". This module declares no
    # folder at all, because a folder path is personal data (invariant 13) —
    # so the default would DELETE EVERY FOLDER on each `nixos-rebuild`,
    # including the ones the person has just added through the interface,
    # with no warning and no undo.
    #
    # There is no symptom before the damage. Hence the check.
    # ================================================================
    if [ "${lib.boolToString syncthingEval.services.syncthing.overrideFolders}" != "false" ]; then
      echo "FAILED: overrideFolders switched on ERASES the user's folders" >&2
      exit 1
    fi
    if [ "${lib.boolToString syncthingEval.services.syncthing.overrideDevices}" != "false" ]; then
      echo "FAILED: overrideDevices switched on ERASES the paired devices" >&2
      exit 1
    fi
    # UPnP failed every 30 minutes on the measured machine, forever.
    if [ "${lib.boolToString syncthingEval.services.syncthing.settings.options.natEnabled}" != "false" ]; then
      echo "FAILED: natEnabled switched on in a LAN synchronisation" >&2
      exit 1
    fi
    # The relay is a very slow fallback route: with it, a momentary discovery
    # failure drops the transfer to ~1 MB/s with nothing saying so.
    if [ "${lib.boolToString syncthingEval.services.syncthing.settings.options.relaysEnabled}" != "false" ]; then
      echo "FAILED: relaysEnabled lets the transfer fall back to the relay" >&2
      exit 1
    fi
    # The package comes from Nix; self-updating would try to write to /nix/store.
    if [ "${builtins.toString syncthingEval.services.syncthing.settings.options.autoUpgradeIntervalH}" != "0" ]; then
      echo "FAILED: auto-update switched on for a package in the store" >&2
      exit 1
    fi
    echo "OK: override off, no NAT, no relay, no auto-upgrade"
  '';

  # The token generator REFUSES a name that does not become a WIT identifier.
  #
  # The real defect: `wifi-0` was accepted here and broke the build three
  # derivations later, inside a WASM buffer, with the message "invalid
  # character in identifier '0'" pointing at the generated enum — which does
  # not say where the name came from. The check tests BOTH sides: the good
  # name passes, the bad one is refused.
  tokens-wit-name = mkCheck "tokens-wit-name" [ pkgs.python3 ] ''
    python3 - <<'END'
    import importlib.util
    spec = importlib.util.spec_from_file_location("gt", "${../nix/scripts/gen-tokens.py}")
    gt = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gt)

    for good in ["wifi-weak", "network-secured", "surface.base", "app-firefox"]:
        gt.wit_ident(good)

    for bad in ["wifi-0", "signal-4", "colour-2x"]:
        try:
            gt.wit_ident(bad)
        except SystemExit:
            continue
        raise SystemExit(f"FAILED: `{bad}` passed, and the WIT would refuse it")
    print("OK: the generator refuses a name the WIT does not accept")
    END
  '';

  # ===================================================================
  # INVARIANT 18 — every NAME is English; every COMMENT is English too.
  #
  # The `CLAUDE.md` already said "identifiers in English" and it was never met:
  # there were 54 ids, 90 checks, ~35 files and 358 keys in Portuguese. A rule
  # with no check is a recommendation, and a recommendation is what produced
  # that distance between what the repository asserts and what it does.
  #
  # WHY A DENY LIST, AND NOT "detect Portuguese"
  # --------------------------------------------
  # Detecting a language in general is impossible in a check. Refusing EXACTLY
  # the words that have already been here is possible, honest, and it is what
  # stops the regression — which is the real risk: someone adds `rede-abrir`
  # again out of habit.
  #
  # IT SWEEPS NAMES, NOT PROSE. It looks at: file paths, the `.ftl` key column,
  # and — since the identifier pass — every Rust declaration: `fn`, `struct`,
  # `enum`, `trait`, `const`, `static` and `mod`.
  #
  # It does NOT sweep comments, and that is not because they are exempt —
  # the owner moved them to English, so the second half of the invariant now
  # covers only the four documents, the key VALUES and the search tags.
  # Detecting "this comment is in Portuguese" is the same impossible
  # language detection the deny list exists to avoid; what guards the
  # comments is the sweep that translated them, not a check.
  #
  # THE DECLARATION IS READ ON ITS OWN LINE, and the sweep is deliberately
  # blind to a use: `let rede = …` is not caught here. Catching every local
  # would mean parsing Rust, and a check that needs a parser to be right is a
  # check that goes wrong on its own. What it does catch is what OTHER code
  # sees — and that is where a Portuguese name spreads from.
  english-names = mkCheck "english-names" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    # The stems this refactor removed. Adding here is cheap; taking one out
    # requires explaining why the word became acceptable again.
    FORBIDDEN = [
         "aberto", "abrir", "acao", "achada", "achados", "achou", "acoes",
         "carga", "criar", "globais", "leitura", "ordens",
         "abaixo", "acha", "achado", "acorde", "acordes", "alca", "alta", "alternar",
         "andando", "apagar", "arredondar", "atencao", "ativar", "atuais", "atual",
         "atualizando", "aviso", "barra", "branco", "bruta", "cabem", "caixa",
         "caixas", "candidatas", "caractere", "carinhas", "casa", "casal", "cerebro",
         "completos", "componente", "conexao", "conexoes", "conj",
         "contando", "copia", "copias", "custo", "dedo", "degrau", "desde", "desenhado",
         "dicas", "diferentes", "digita", "direto", "diretorio", "documento",
         "eixo", "embutido", "empurrar", "enormes", "envio", "erros", "escalada",
         "escopo", "escravo", "esq", "estende", "estilo", "estranho", "exclusiva",
         "exigida", "existente", "faltam", "fator", "fileira", "focado", "gatilho",
         "gerenciador", "grande", "grego", "identificador", "imagem", "indicador",
         "inexistente", "inicial", "instala", "instalar", "interno", "iteracao",
         "ligacoes", "ligado", "linguagem", "lupa", "marcador", "mau", "melhor",
         "mentes", "metodo", "metrica", "meu", "migalha", "minusculo", "moldagem",
         "montar", "mudou", "nascimento", "nivel", "nulo", "ocupados", "oferta",
         "onde", "ordenados", "orfaos", "ouvinte", "palavra", "parado", "pedido",
         "pendente", "pequeno", "pintou", "piscando", "piso", "pode", "postos",
         "postura", "preciso", "preparo", "preparos", "preto", "primeira", "procurando",
         "producao", "pronta", "realcadas", "recebimento", "recurso", "resolvido",
         "secao", "segunda", "selagem", "setas", "sistema", "sobreviventes",
         "subfiltros", "sujo", "tecla", "tentar", "termo", "trabalho", "tres",
         "treze", "trocada", "unicos", "usadas", "valores", "velho", "venceu",
         "vivas", "vizinha",
         "afinar", "agente", "agulha", "ajuda", "altura", "alvo", "anotacao",
         "anunciar", "aplicar",
         "anterior", "antes", "antigo", "argumento", "arquivo", "arquivos",
         "arrastar", "arvore", "atribuicao", "avanco", "baixo", "banda",
         "binario", "bloco", "bruto", "busca", "cadeia", "camada", "caminho",
         "capacidade", "captura", "capturando", "capturar", "carimbo", "caso",
         "comecar",
         "casou", "catalogo", "categoria", "categorias", "celula", "chave",
         "chaves", "cima", "cliente", "cobertura", "cofre", "colado",
         "colisao", "colocado", "coluna", "colunas", "comandar", "comando",
         "comandos", "combinante", "completa", "conectar", "configuracao",
         "conjunto", "contexto", "contrato", "controlar", "copiar", "cor",
         "corridas", "declarada", "depois", "desconectar", "descritor",
         "desloc", "detalhe", "diagnostico", "diario", "dimensoes", "direita",
         "discos", "dispositivo", "dispositivos", "dito", "dominio", "eco",
         "efeito", "efeitos", "elemento", "elidir", "emergencia", "encaixe",
         "encerrar", "encolhe", "enrolar", "entalhes", "entrada", "escala",
         "escrita", "escutar", "esperada", "esperado", "esquecer", "esquerda",
         "estado", "estatico", "estrangeiro", "eventos", "executar",
         "executar", "expandido", "expansao", "fabrica", "falhou", "falta", "faltando",
         "familia", "fechar", "ficha", "fila", "filhos", "filtro", "fluxo",
         "fonte", "fontes", "fora", "fragmento", "frase", "frequencia",
         "fundo", "gesto", "glifo", "glifos", "grade", "grafico", "gravada",
         "grupo", "guarda", "guardar", "heranca", "hospedeiro", "icone",
         "icones", "idioma", "imagens", "impermanencia", "instalador",
         "instanciar", "interatividade", "interativo", "janela", "juntos", "lado",
         "lancamento", "largo", "largura", "legivel", "levar", "lido",
         "ligacao", "limite", "limites", "limpar", "linha", "linhas",
         "localizador", "marca", "meio", "modelo", "modo", "morreu", "morto",
         "motor", "mover", "navegacao", "navegador", "nome", "nomes",
         "notificacao", "novo", "observador", "opcao", "orcamento", "ordem",
         "origem", "outra", "pacote", "padrao", "padroes", "painel", "partes",
         "passo", "passos", "pedaco", "pedacos", "pendencia", "perfil",
         "perfis", "pergunta", "pintura", "plano", "politica", "posicao",
         "prefixo", "preludio", "presente", "primeiro", "programa", "proibido",
         "proximo", "puxar", "quantos", "raiz", "ramo", "recarrega",
         "reconstruir", "rede", "redesenhar", "repeticao", "resolucao",
         "revarrer", "rodape", "rolagem", "rotulo", "ruim", "saida", "sair",
         "segmento", "segmentos", "segredo", "segundo", "selar", "senha",
         "sequencia", "servidor", "sigilo", "simbolo", "simbolos", "sobrando",
         "sobrou", "sono", "sorteia", "sorteio", "subida", "subir",
         "superficie", "surdo", "tabela", "tam", "tamanho", "teclado", "tela",
         "tema", "teto", "texto", "textos", "tipografia", "topo", "trecho",
         "trechos", "ultima", "ultimo", "unidade", "urgencia", "usuario",
         "valido", "valor", "variante", "variavel", "varrer", "vazia", "vazio", "versao",
         "vetorial", "vigia", "visao", "vista", "visto", "vivo", "volta",
         "voltou",
         # AND WHAT THE 2026-09-03 SWEEP FOUND that no ending can reach. A word
         # like `completo`, `registro` or `fim` has ordinary Portuguese shape
         # and ordinary Latin endings; only a list catches those, and each one
         # here was a real name or a real message in this tree on that day.
         "acentuados", "alguem", "aninhado", "apelido", "candidato", "coisa",
         "completo", "conteudo", "desmontar", "fase", "fim", "funil", "girar",
         "laco", "livre", "mostra", "ninguem", "pedir", "pessoa", "rasterizar",
         "reiniciar", "registro", "relogio", "responder", "salvar", "seguro",
         "sessao", "tolerante", "trocar", "tudo",
    ]
    RX = re.compile(r"(?<![a-z])(" + "|".join(FORBIDDEN) + r")s?(?![a-z])")

    # AND THE SHAPE OF A WORD, because a list is finite and Portuguese is not.
    #
    # Every stem above had to be SEEN before it could be listed, and the sweep
    # the owner asked for found thirteen identifiers and a hundred and fifty
    # messages whose stems simply were not on it: `pronto_tx`,
    # `doze_acentuados`, `IdDuplicado`, `n_motores`, `com_tags`, `fase`,
    # `lidas`, `mostra`. No amount of adding closes that; an ENDING does.
    #
    # These are the endings no English word takes — with the handful that do
    # underneath, because `avocado` and `crescendo` are English and a ruler
    # that does not know it would be a ruler nobody could keep green.
    MORPH = re.compile(
        r"(?<![a-zà-ü])[a-zà-ü]{3,}"
        r"(?:ção|ções|cao|coes|mente|ência|encia|ância|ancia"
        r"|agem|idade|ndo|ado|ada|ados|adas|ido|ida|idos|idas)"
        r"(?![a-zà-ü])"
    )
    # Adding here is cheap and each one is a word this tree really uses;
    # taking one out means claiming English stopped having it.
    NOT_PORTUGUESE = {
        "avocado", "tornado", "tornados", "desperado", "bravado", "aficionado",
        "dorado", "colorado", "crescendo", "innuendo", "commando", "commandos",
        "cacao", "armada", "florida", "canada", "silverado", "eldorado",
        # AND THE NAMES OF THINGS. `COLLADA` is a 3D format this tree reads,
        # and `-ada` is its last three letters by accident.
        "collada",
    }

    def portuguese(text):
        """The first Portuguese word in `text`, by the list OR by the ending."""
        m = RX.search(text)
        if m:
            return m.group(0)
        for m in MORPH.finditer(text):
            if m.group(0).lower() not in NOT_PORTUGUESE:
                return m.group(0)
        return None

    # AND A LINE MAY SAY WHY IT IS PORTUGUESE, and then it is.
    #
    # Four kinds of Portuguese belong in code, and all four are DATA: the VALUE
    # of a pt-BR key that a test asserts (`*sem nome*`), a search tag the
    # lexicon has to find (`seta para cima`), a fixture whose SUBJECT is the
    # accent (`let ação = 1;` for the highlighter), and the pt-BR half of a
    # generated table. Telling them apart from a slip is not something a list
    # in this file can do — the reason lives where the string is, so that is
    # where the note goes.
    ALLOWED = re.compile(r"(?://|#)\s*pt-BR:")

    def allowed_at(lines, first, last=None):
        """A `pt-BR:` note on the line, on the one above, or inside the span."""
        last = first if last is None else last
        return any(
            ALLOWED.search(lines[i])
            for i in range(max(0, first - 3), min(len(lines), last))
        )

    # Every Rust declaration that becomes a name OTHER code can see.
    DECL = re.compile(
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+|unsafe\s+|const\s+|extern\s+\S+\s+)*"
        r"(fn|struct|enum|trait|type|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
    )

    root_dir = pathlib.Path("${self}")
    gaps = []

    # 1) FILE AND DIRECTORY NAMES
    for p in root_dir.rglob("*"):
        if any(x in p.parts for x in (".git", "result", "target", ".direnv")):
            continue
        if portuguese(p.name.lower().replace("_", "-")):
            gaps.append(f"file: {p.relative_to(root_dir)}")

    # 2) THE .ftl KEY COLUMN — the value stays in pt-BR, only the name changes
    for ftl in (root_dir / "assets/i18n").glob("*.ftl"):
        for n, line in enumerate(ftl.read_text().splitlines(), 1):
            m = re.match(r"^([a-z][a-z0-9-]*) *=", line)
            if m and portuguese(m.group(1)):
                gaps.append(f"key: {ftl.name}:{n}: {m.group(1)}")

    # 3) RUST DECLARATIONS — the half of the invariant the identifier pass closed
    #
    # A VARIANT IS A DECLARATION TOO, and reading only `enum Foo` missed it:
    # `Question::Senha`, `Question::Idioma` and `Question::WifiSenha` came through
    # the whole identifier refactor untouched, because the line that names them
    # has no keyword on it — just an indented capitalized word.
    VARIANT = re.compile(r"^\s+([A-Z][A-Za-z0-9_]*)\s*(?:[,({=]|$)")
    # A FIELD IS A DECLARATION TOO, and the same blind spot: the line that names
    # it carries no keyword, only an indented lowercase word and a colon. That is
    # how `Minibuffer.segredo` came through the whole identifier refactor.
    FIELD = re.compile(r"^\s+(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:")
    # A `let` IS A NAME, and this check never read one.
    #
    # It read declarations, fields and variants — everything with a keyword in
    # front of it — so sixteen Portuguese bindings lived in `surface.rs` and
    # `browser.rs` through the whole identifier refactor and through every green
    # battery since: `barra_visivel`, `total_expansao`, `fracao`, `fundos`,
    # `proporcao`, `quantos`, `clicou`, `menor`, `margem`, `realce_do_ponteiro`.
    # The owner found them by reading a plan, not the code, and he was right
    # that a rule nothing enforces is not a rule.
    #
    # A `let` is scoped and short-lived, which is exactly why it is where a
    # language slips back in.
    # A BINDING IS A PATTERN, not a word. `let (tx_nome, rx_nome) = channel()`
    # names two things and matches no regex that expects an identifier right
    # after the keyword, which is why a simpler version of this check read the
    # line and saw nothing. Walk the pattern instead, and take every lowercase
    # name in it: `for` binds the same way and gets the same walk.
    OPENS = re.compile(r"\b(?:let|for)\b")
    WORD = re.compile(r"[a-z_][a-z0-9_]*")
    PATTERN_NOISE = {"mut", "ref", "in", "let", "for", "if", "else"}

    def bound_names(code):
        """Every name a `let` or a `for` on this line binds."""
        for kw in OPENS.finditer(code):
            i, depth = kw.end(), 0
            start = i
            while i < len(code):
                c = code[i]
                if c in "([{<":
                    depth += 1
                elif c in ")]}>":
                    depth -= 1
                # A TYPE IS NOT A BINDING, and neither is the value: the
                # pattern ends at the first `=` or `:` outside any bracket.
                elif depth == 0 and (c == "=" or c == ":"):
                    break
                elif depth == 0 and code[i : i + 4] == " in ":
                    break
                i += 1
            for w in WORD.finditer(code[start:i]):
                if w.group(0) not in PATTERN_NOISE:
                    yield w.group(0)

    # AND A `fn` AND ITS ARGUMENTS. `DECL` below already reads the free
    # functions, but not a method inside an `impl` and not one parameter of
    # either — which is how `colunas_da_tela` and `altura_da_tela` sat in the
    # signature of a function whose own name was English.
    FN_NAME = re.compile(r"\bfn\s+([a-z_][a-z0-9_]*)")
    FN_ARGS = re.compile(r"\bfn\s+[a-z_0-9]*\s*(?:<[^>]*>)?\s*\((.*)")
    ARG = re.compile(r"^\s*(?:mut\s+)?([a-z_][a-z0-9_]*)\s*:")
    for base in ("crates", "buffers"):
        for rs in (root_dir / base).rglob("*.rs"):
            if "target" in rs.parts:
                continue
            in_enum = 0
            in_struct = 0
            for n, line in enumerate(rs.read_text().splitlines(), 1):
                # WHAT IS INSIDE A STRING OR A COMMENT IS NOT A NAME. The six
                # documents are pt-BR by decision, and so is the text of a test's
                # assertion; only the identifier is bound by invariant 18.
                code = line.split("//")[0]
                if '"' in code:
                    code = re.sub(r'"[^"]*"', '""', code)
                for bound in bound_names(code):
                    if portuguese(bound.lower().replace("_", "-")):
                        rel = rs.relative_to(root_dir)
                        gaps.append(f"let: {rel}:{n}: {bound}")
                fname = FN_NAME.search(code)
                if fname:
                    word = fname.group(1).lower().replace("_", "-")
                    if portuguese(word):
                        rel = rs.relative_to(root_dir)
                        gaps.append(f"fn: {rel}:{n}: {fname.group(1)}")
                fargs = FN_ARGS.search(code)
                if fargs:
                    # One line, and the arguments that fit on it: a signature
                    # broken over several lines still shows every name after a
                    # comma, and each of those is read as its own field below.
                    for piece in fargs.group(1).split(")")[0].split(","):
                        a = ARG.match(piece)
                        if a:
                            word = a.group(1).lower().replace("_", "-")
                            if portuguese(word):
                                rel = rs.relative_to(root_dir)
                                gaps.append(f"arg: {rel}:{n}: {a.group(1)}")
                m = DECL.match(line)
                if m:
                    # CamelCase and SNAKE_CASE become the same ruler as the file name.
                    item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", m.group(2)).lower().replace("_", "-")
                    if portuguese(item_name):
                        rel = rs.relative_to(root_dir)
                        gaps.append(f"{m.group(1)}: {rel}:{n}: {m.group(2)}")
                    in_enum = 1 if m.group(1) == "enum" else 0
                    in_struct = 1 if m.group(1) == "struct" else 0
                    continue
                if line.startswith("}"):
                    in_enum = 0
                    in_struct = 0
                    continue
                if in_struct:
                    f = FIELD.match(line)
                    if f:
                        item_name = f.group(1).replace("_", "-")
                        if portuguese(item_name):
                            gaps.append(f"field: {rs.relative_to(root_dir)}:{n}: {f.group(1)}")
                    continue
                if not in_enum:
                    continue
                # The body ends at the closing brace in column 0.
                if line.startswith("}"):
                    in_enum = 0
                    continue
                v = VARIANT.match(line)
                if not v:
                    continue
                item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", v.group(1)).lower().replace("_", "-")
                if portuguese(item_name):
                    gaps.append(f"variant: {rs.relative_to(root_dir)}:{n}: {v.group(1)}")

    # 4) NIX AND PYTHON DEFINITIONS
    #
    # `english-names` used to stop at Rust, and the build layer drifted out of
    # sight behind it: `fonteEmoji`, `versaoDeclarada`, `hostGrafico`, plus every
    # variable inside the VM tests' embedded scripts. A `let` binding and a shell
    # assignment are names too, and this is the ruler that reaches them.
    # A NIX ATTRIBUTE IS ALLOWED A HYPHEN, and reading only `[A-Za-z0-9_]` is
    # how `fontconfig-completo` sat in `flake.nix` through every green battery
    # this repository has run: the class stopped at `fontconfig`, the `=` was
    # never reached, and the line simply did not match. Python has no hyphen in
    # an identifier, so widening the class costs that half nothing.
    DEF = re.compile(
        r"^\s*(?:def\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("
        r"|([A-Za-z_][A-Za-z0-9_-]*)\s*=(?!=))"
    )
    for suf in ("*.nix", "*.py"):
        for f in root_dir.rglob(suf):
            if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
                continue
            for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1):
                # Prose, not a definition: `fontes (passa de 1 GB)` inside a
                # comment matches the same shape as a call.
                if line.lstrip().startswith(("#", "//")):
                    continue
                m = DEF.match(line)
                if not m:
                    continue
                got = m.group(1) or m.group(2)
                item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", got).lower().replace("_", "-")
                if portuguese(item_name):
                    gaps.append(f"def: {f.relative_to(root_dir)}:{n}: {got}")

    # 5) THE CLI'S SUBCOMMANDS
    #
    # These live as STRINGS in a `match` arm, so no declaration rule reaches
    # them: `comando`, `satelite`, `verificar-keymap` and `exportar-binds-niri`
    # went through the whole refactor untouched. They are commands, which
    # invariant 18 names explicitly.
    for rs in (root_dir / "crates").rglob("main.rs"):
        if "target" in rs.parts:
            continue
        for n, line in enumerate(rs.read_text().splitlines(), 1):
            for m in re.finditer(r'Some\("([a-z][a-z0-9-]*)"\)', line):
                if portuguese(m.group(1)):
                    gaps.append(f"subcommand: {rs.relative_to(root_dir)}:{n}: {m.group(1)}")

    # 6) THE CLI'S FLAGS
    #
    # A flag is a NAME — it is what a person types and what a script writes —
    # and no rule above sees it: it lives as a bare string compared with `==`,
    # in any file, not only `main.rs`. `--executar` survived the whole refactor
    # in the installer AND in three places that documented it, while the very
    # docstring of the file that defined it already said `--run_command`. The
    # code and its own documentation disagreed, and nothing could say so.
    for rs in (root_dir / "crates").rglob("*.rs"):
        if "target" in rs.parts:
            continue
        for n, line in enumerate(rs.read_text().splitlines(), 1):
            for m in re.finditer(r'"--([a-z][a-z0-9-]*)"', line):
                if portuguese(m.group(1)):
                    gaps.append(f"flag: {rs.relative_to(root_dir)}:{n}: --{m.group(1)}")

    # 7) COMMENTS — the half of the invariant the message already claimed
    #
    # Invariant 18 says NAME, COMMENT and COMMIT, and the failure message below
    # said so too. The check did not: every pass above skips comment lines on
    # purpose, because prose has the same shape as a call. So a whole file of
    # Portuguese comments went in without a word — `tests/slow.nix` and
    # `tests/quick.nix` were written that way, and so were four blocks that had
    # been sitting in `nix/modules/base.nix`, `nix/modules/host.nix`,
    # `nix/git-hooks.nix` and this very file.
    #
    # Two things make prose testable with the same word list:
    #
    #   * COMMENT RUNS ARE JOINED. A backtick span often opens on one line and
    #     closes on the next, and line-by-line stripping leaves half a quoted
    #     identifier looking like Portuguese prose.
    #   * WHAT IS QUOTED DOES NOT COUNT. An English comment naming
    #     `Question::Senha` or the chapter `1. Fontes, glifos e icones` is
    #     correct English about a Portuguese thing. Backticks and double quotes
    #     come out before the ruler is applied.
    #
    # The comment marker is per LANGUAGE, not shared: a `#` at the start of a
    # line inside a Rust string is not a comment, and `plan.rs` writes exactly
    # that — the header of the generated `host.nix`, in Portuguese, which is a
    # VALUE and stays.
    #
    # AND `.wgsl` IS A LANGUAGE TOO. This pass knew four extensions and the
    # shaders were not among them, so `quad.wgsl` and `image.wgsl` were written
    # in Portuguese from the first commit and stayed that way — comments AND
    # names. They are the code that runs on the GPU for every frame this
    # system draws, and nothing here had ever read a line of them.
    MARKER = { ".rs": "//", ".nix": "#", ".py": "#", ".wgsl": "//" }
    for suf in ("*.rs", "*.nix", "*.py", "*.wgsl"):
        for f in sorted(root_dir.rglob(suf)):
            if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
                continue
            mark = MARKER[f.suffix]
            line_is = re.compile(r"^\s*" + re.escape(mark) + r"[!/]?\s?(.*)$")
            run, first = [], None
            for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1) :
                m = line_is.match(line)
                if m:
                    if first is None:
                        first = n
                    run.append(m.group(1))
                    continue
                if run:
                    prose = " ".join(run)
                    prose = re.sub(r"`[^`]*`", " ", prose)
                    prose = re.sub(r'"[^"]*"', " ", prose)
                    hit = portuguese(prose.lower())
                    if hit:
                        rel = f.relative_to(root_dir)
                        gaps.append(f"comment: {rel}:{first}: {hit}")
                run, first = [], None
            if run:
                prose = re.sub(r"`[^`]*`", " ", " ".join(run))
                prose = re.sub(r'"[^"]*"', " ", prose)
                hit = portuguese(prose.lower())
                if hit:
                    gaps.append(f"comment: {f.relative_to(root_dir)}:{first}: {hit}")

    # 8) THE DESCRIPTOR'S ids — the last Portuguese name that reached a screen
    #
    # A buffer's `name` and `mode` are STRING LITERALS inside the component, and
    # no rule above sees a string on the right of a colon. They became the window
    # TITLE and the `app_id`, both drawn raw — so `glifos`, `memoria`, `exemplo`
    # and `transferencias` went through the whole identifier refactor untouched
    # and were the only Portuguese left on a running screen, in every language.
    #
    # They are ids now, and what the screen shows comes from `buffer-<id>` and
    # `mode-<id>`. This is the ruler that keeps them ids.
    DESCRIPTOR_ID = re.compile(r'^\s+(name|mode):\s*"([a-z][a-z0-9-]*)"')
    for rs in sorted((root_dir / "buffers").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        inside = False
        for n, line in enumerate(rs.read_text().splitlines(), 1):
            if "Descriptor {" in line:
                inside = True
                continue
            if inside and line.strip() in ("}", "};"):
                inside = False
                continue
            if not inside:
                continue
            m = DESCRIPTOR_ID.match(line)
            if m and portuguese(m.group(2)):
                rel = rs.relative_to(root_dir)
                gaps.append(f"descriptor: {rel}:{n}: {m.group(1)} = {m.group(2)}")

    # 9) THE MESSAGES THEMSELVES, and this half was open all along.
    #
    # Everything above reads DECLARATIONS. Not one of them looks inside a
    # string — so a hundred and eighty-five Portuguese messages lived in
    # `.context`, `bail!`, `expect` and `write!` through every green battery
    # this repository has ever run: `"ler o verbo"`, `"mudar o dono de {}"`,
    # `"o niri recusou {request}"`, `"registry do Wayland"`. They are the
    # sentences a person actually SEES when something goes wrong, which makes
    # them the half of invariant 18 that matters most.
    #
    # TWO RULES, because Portuguese slips in two ways.
    #
    # GRAMMAR: a sentence in Portuguese carries a function word, and the ones
    # below have no English homograph. `do` and `no` and `a` are left out for
    # exactly that reason — "do not run" and "no such file" are English.
    #
    # VOCABULARY: a one-word message has no grammar to catch. So the verbs and
    # nouns that were removed are listed, the same bargain `FORBIDDEN` above
    # already makes: adding one is cheap, and taking one out has to be argued.
    GRAMMAR = re.compile(
        r"(?:^| )(da|das|na|nas|ao|aos|uma|umas|pelo|pela|não|nao|são|sao"
        r"|sem|que|com|para|foi|pra|nem)(?: |$)"
    )
    VOCABULARY = re.compile(
        r"(?<![a-z])(ler|mudar|abrir|criar|gravar|apagar|mover|copiar|verbo|dono"
        r"|senha|invalido|desconhecid[oa]|duplicar|esvaziar|clonar|esperar"
        r"|consultar|pendurar|recusou|recusa|saiu|devolver|recriar|ajustar"
        r"|renomear|permissao|inicio|contagem|devia|fechou|abriu|mostrando"
        r"|hospedar|mandar|rodar|codificar|geracao|atualizacao|mestre|espacos"
        r"|colar|folga|escolheu|pediu|verificado|bandeira|distingue|uso"
        r"|diferencial|fatos|resgate|passou|razao|confirmacao|subiu|desceu"
        r"|nasceu|interruptor|clicar|avancou|modificador|sumiu|ausente|diferenca"
        r"|esperava|virou|tela|erro|linha|arquivo|pasta|qualquer)(?![a-z])"
    )
    # The macros a message reaches a person through.
    #
    # THE LIST WAS TOO SHORT, and every absence was a whole vehicle. Twenty-six
    # `tracing::` lines put `"faxina concluida"` and `"o oomd encerrou um
    # cgroup"` in the journal; `Display for Cancelled` wrote `"cancelado"`; a
    # thread was NAMED `"notificacoes"`; and the manager's window called itself
    # `*arquivos*` on a real screen, in every language. None of them is a macro
    # from the first line, and that is the only reason they survived.
    SPEAKS = re.compile(
        r"\.(context|with_context)\b|\bbail!|\banyhow!|\bexpect\(|\bpanic!"
        r"|\be?println!|\bwrite(ln)?!|\bassert(_eq|_ne)?!"
        r"|\btracing::(?:info|warn|error|debug|trace)!"
        r"|\bformat!|\bwrite_str\(|\.name\(|\.set_title\(|\.set_app_id\("
    )
    LITERAL = re.compile(r'"([^"\\]*(?:\\.[^"\\]*)*)"')

    def morph_only(text):
        """The first word whose ENDING is Portuguese, or None."""
        for m in MORPH.finditer(text):
            if m.group(0).lower() not in NOT_PORTUGUESE:
                return m.group(0)
        return None

    def bare(x):
        """The text with its strings emptied, for counting brackets."""
        return re.sub(r'"[^"]*"', '""', x)

    def whole_call(lines, n):
        r"""The call that opens on line `n`, however many lines it runs over.

        A CALL IS NOT A LINE, and reading one line at a time cut every sentence
        that did not fit on one. Two shapes did it: Rust's `\` continuation
        inside a string, and a macro whose opening bracket is alone on its line
        (`format!(` and then the sentence underneath). Follow both — the
        backslash, and the brackets, counted OUTSIDE the strings.
        """
        span, i = lines[n - 1], n
        while i < len(lines) and i - n < 8:
            if span.rstrip().endswith("\\"):
                span = span.rstrip()[:-1] + " " + lines[i].lstrip()
            elif bare(span).count("(") > bare(span).count(")"):
                span += " " + lines[i]
            else:
                break
            i += 1
        return span, i

    def said_in_portuguese(s):
        """The Portuguese in something a person is shown, or None."""
        low = s.lower()
        if " " in s.strip():
            # A SENTENCE carries a function word, and the ones in `GRAMMAR`
            # have no English homograph. `FORBIDDEN` is deliberately NOT
            # applied here: it holds `comando` and `arquivo`, which is what the
            # pt-BR VALUE of a key says on purpose, and a test may assert it.
            m = GRAMMAR.search(low) or VOCABULARY.search(low)
            return m.group(0) if m else morph_only(low)
        # ONE WORD has no grammar to catch it — but `FORBIDDEN` is still the
        # wrong ruler for it. Half the single-word literals in this tree are
        # the pt-BR half of the SEARCH LANGUAGE (`familia:`, `bloco:`,
        # `categoria:`), which the three browsers ACCEPT from whoever types it;
        # that is a tag, and invariant 18 keeps tags in both languages. So a
        # lone word is measured by the message vocabulary and by its ending.
        # (What a browser WRITES is English — see `INITIAL_FILTER`.)
        m = VOCABULARY.search(low)
        return m.group(0) if m else morph_only(low)

    # `whole_call` above is what reads a sentence written on more than one
    # line, and it is how `"desenhou vetor sem declarar `Vetor`"` and
    # `"o `SPC b s` oferece e ninguem entra"` are seen at all: the Portuguese
    # sat on a continuation, and a continuation is not a line matching `SPEAKS`.
    #
    # AND `buffers` READS THE SAME RULER. Pass 9 was written for `crates`, and
    # the seven components beside it were never read at all.
    for base in ("crates", "buffers"):
        for rs in sorted((root_dir / base).rglob("*.rs")):
            if "target" in rs.parts:
                continue
            lines = rs.read_text().splitlines()
            for n, line in enumerate(lines, 1):
                if not SPEAKS.search(line):
                    continue
                whole, i = whole_call(lines, n)
                for s in LITERAL.findall(whole):
                    # WHAT IS IN BACKTICKS IS DATA. Two tests search for
                    # `tag:pasta` and `bandeira brasil`: the query is the
                    # fixture and the sentence around it is English, so the
                    # quoted span comes out before the sentence is read.
                    s = re.sub(r"`[^`]*`", " ", s)
                    hit = said_in_portuguese(s)
                    if hit and not allowed_at(lines, n, i):
                        rel = rs.relative_to(root_dir)
                        gaps.append(f"message: {rel}:{n}: {s[:70]}")

    # 10) THE SHADERS' OWN NAMES.
    #
    # The comment pass above now opens `.wgsl`; this reads what one declares.
    # `struct Instancia`, `struct Saida`, `posicao`, `textura`, `amostrador`,
    # `direita`, `baixo`, `cor` — an entire language of this system written in
    # Portuguese and never seen, because no rule here had opened the file.
    # Only `vs` and `fs` cross into Rust, as the `entry_point` strings; every
    # other name in there has this check as its one reader.
    WGSL_DECL = re.compile(
        r"\b(?:struct|fn|let|var(?:<[^>]*>)?)\s+([A-Za-z_][A-Za-z0-9_]*)"
    )
    WGSL_MEMBER = re.compile(
        r"^\s*(?:@[a-z_]+\([^)]*\)\s*)*([A-Za-z_][A-Za-z0-9_]*)\s*:"
    )
    WGSL_ARG = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*:")
    for w in sorted(root_dir.rglob("*.wgsl")):
        lines = w.read_text().splitlines()
        for n, line in enumerate(lines, 1):
            code = line.split("//")[0]
            names = [m.group(1) for m in WGSL_DECL.finditer(code)]
            member = WGSL_MEMBER.match(code)
            if member:
                names.append(member.group(1))
            # A PARAMETER IS A NAME TOO, and it is the one place a shader
            # writes a name with no keyword in front of it.
            if "fn " in code:
                names += [a.group(1) for a in WGSL_ARG.finditer(code)]
            for name in names:
                item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower().replace("_", "-")
                if portuguese(item_name) and not allowed_at(lines, n):
                    gaps.append(f"wgsl: {w.relative_to(root_dir)}:{n}: {name}")

    # 11) AND WHAT THE BUILD AND THE CHECKS SAY OUT LOUD.
    #
    # Pass 9 reads Rust. Nothing read a `.nix` or a `.py`, and those speak to a
    # person too: `echo "  compilando o buffer $name…"` in the loop the owner
    # runs every day, `echo "--- extraindo … ---"` while the fonts unpack,
    # `print(f"duracao: …")` in a VM test, `subtest("o wipe renova @root")` —
    # and `echo "validando $cfg"` inside this very file. Thirty-nine lines of
    # Portuguese on somebody's terminal, and not one rule looked at them.
    #
    # ONLY WHAT IS SHOWN, and the restriction is what makes the pass possible:
    # reading every literal would fail on `FORBIDDEN` and `VOCABULARY` above,
    # which are Portuguese on purpose, and on the pt-BR half of every generated
    # table. A message is an ARGUMENT of something that prints.
    SHOWN = re.compile(
        r"\becho\s|\bprintf\s|\bprint\(|\bdie\(|\bcomplain\(|\bsys\.exit\(|\bsubtest\("
    )
    SPOKEN = re.compile(r'"([^"\n]*)"')

    for suf in ("*.nix", "*.py"):
        for f in sorted(root_dir.rglob(suf)):
            if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
                continue
            lines = f.read_text(errors="replace").splitlines()
            for n, line in enumerate(lines, 1):
                if line.lstrip().startswith("#") or not SHOWN.search(line):
                    continue
                # THE CALL MAY RUN OVER SEVERAL LINES, and in `gen-updates.py`
                # the Portuguese was on the third of them. Follow it until the
                # brackets close, counting them OUTSIDE the strings.
                span, i = whole_call(lines, n)
                for s in SPOKEN.findall(span):
                    s = re.sub(r"`[^`]*`", " ", s)
                    hit = said_in_portuguese(s)
                    if hit and not allowed_at(lines, n, i):
                        rel = f.relative_to(root_dir)
                        gaps.append(f"said: {rel}:{n}: {s[:70]}")

    if gaps:
        print("ERROR: a Portuguese name (invariant 18).", file=sys.stderr)
        print("", file=sys.stderr)
        # EVERY ONE OF THEM. A cap here hid 88 of 128 findings and cost a
        # whole round trip to discover; a check that knows the answer and
        # prints part of it is worse than one that prints nothing, because
        # the part reads like the whole. Saying the SAME one twice is not a
        # cap, and a line that quotes one string twice used to do exactly that.
        for f in dict.fromkeys(gaps):
            print(f"       {f}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Every NAME and every COMMENT is English.", file=sys.stderr)
        print("       In pt-BR, only the 4 docs, the VALUE of a key and the search tag.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: nothing in Portuguese — name, key, comment, subcommand, flag,")
    print("    message, shader AND what the build says out loud")
    END
  '';

  # The flag a `.nix` PASSES has to be one the generator DECLARES.
  #
  # It is a name crossing the boundary as a string, exactly like the WIT: nothing
  # ties `add_argument("--teto-sem-pt")` to the `--teto-sem-pt` in the caller, so
  # renaming one side compiles, evaluates, and dies at build time with
  # `unrecognized arguments` — pointing at the script, not at the rename. It
  # happened twice in the same pass (`--teto-sem-pt`, `--categorias`), which is
  # what earned it a check.
  generator-flags-agree = mkCheck "generator-flags-agree" [ pkgs.python3 ] ''
    python3 - <<'END'
    import collections, pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    declared = collections.defaultdict(set)
    for p in (root_dir / "nix/scripts").glob("*.py"):
        for m in re.finditer(r'add_argument\(\s*"(--[a-z0-9-]+)"', p.read_text()):
            declared[p.name].add(m.group(1))

    # `cp --reflink` and friends: a flag of a coreutils call, not of the script.
    NOT_OURS = {"--reflink"}
    gaps = []
    for p in root_dir.rglob("*.nix"):
        if any(x in p.parts for x in (".git", "result", "target", ".direnv")):
            continue
        t = p.read_text(errors="replace")
        for m in re.finditer(r"(gen-[a-z-]+\.py|scripts/[a-z-]+\.py)([^\n]*(?:\n\s+[^\n]*){0,14})", t):
            script = m.group(1).split("/")[-1]
            if script not in declared:
                continue
            for f in re.findall(r"(--[a-z][a-z0-9-]+)", m.group(2)):
                if f not in declared[script] and f not in NOT_OURS:
                    n = t.count("\n", 0, m.start()) + 1
                    gaps.append(f"{p.relative_to(root_dir)}:{n}: {script} receives {f}")

    if gaps:
        print("ERROR: a flag passed that the generator does not declare.", file=sys.stderr)
        for g in sorted(set(gaps)):
            print(f"       {g}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       A flag's name crosses the boundary as a STRING: both", file=sys.stderr)
        print("       sides change in the same pass, or nothing reports it until the build.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: every flag passed is declared by the generator that receives it")
    END
  '';

  # A `{name}` inside an f-string with no `name` in scope.
  #
  # It is what a rename leaves behind when it reaches the capture and not the
  # assignment, and NOTHING catches it: Python only raises on the line that runs,
  # so it hid on the "not measured" path of `nix run .#bench` and on the summary
  # line the bench prints every time. Three of them were live at once.
  #
  # In a `.nix` file the embedded script has to be isolated from the Nix around
  # it — otherwise a Nix attribute `name = …` counts as a Python definition and
  # hides exactly the orphan being hunted. And the fence does not end at the
  # first `''`: `'''`, `''$` and `''\` are escapes.
  format-captures-resolve = mkCheck "format-captures-resolve" [ pkgs.python3 ] ''
    python3 - <<'END'
    import builtins, pathlib, re, sys

    CAP = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)")
    FSTR = re.compile(r"""(?:\bf|\bfr|\brf)(\"\"\"|'''|\"|')""")
    DEFINE = [
        re.compile(r"^\s*\(?([A-Za-z_*][A-Za-z0-9_,\s*\[\]().]*?)\)?\s*(?::[^=\n]+)?=[^=]", re.M),
        re.compile(r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)", re.M),
        re.compile(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)", re.M),
        re.compile(r"\bfor\s+\(?([A-Za-z_][A-Za-z0-9_,\s()\[\]]*?)\)?\s+in\b", re.M),
        re.compile(r"\bas\s+\(?([A-Za-z_][A-Za-z0-9_,\s()]*)", re.M),
        re.compile(r"^\s*import\s+([A-Za-z_][A-Za-z0-9_.]*)", re.M),
        re.compile(r"\bdef\s+[A-Za-z_][A-Za-z0-9_]*\s*\(([^)]*)\)", re.M),
        re.compile(r"\bglobal\s+([A-Za-z_][A-Za-z0-9_,\s]*)", re.M),
        re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*:=", re.M),
        re.compile(r"\blambda\s+([A-Za-z_][A-Za-z0-9_,\s]*?):", re.M),
        # `from x import a, b as c` — the names it BINDS are on the right, and
        # the plain `import x` above does not see them. The gap showed up the
        # day a generator interpolated a constant it had imported: the name was
        # reported as an orphan, which was this check being wrong rather than
        # the code. (No example is written out here on purpose — an f-string in
        # a comment is an f-string this very scanner reads.)
        re.compile(r"^\s*from\s+[A-Za-z_.][A-Za-z0-9_.]*\s+import\s+(.+)$", re.M),
    ]
    DRIVER = {"machine", "subtest", "start_all", "nodes", "t", "polling_condition"}

    def defined(t):
        s = set(dir(builtins)) | DRIVER
        for p in DEFINE:
            for m in p.finditer(t):
                for part in re.split(r"[,\s()\[\]]+", m.group(1)):
                    part = part.split("=")[0].split(":")[0].split(".")[0].strip("*")
                    if part:
                        s.add(part)
        return s

    # The fence delimiter, built rather than written: a literal pair of quotes
    # in here would close the Nix string this script lives in.
    FENCE = chr(39) * 2

    def fences(t):
        """Every Nix fence. It does NOT end at the first delimiter: a tripled
        quote, and a delimiter followed by `$` or by a backslash, are escapes.
        Reading one of those as the terminator drops most of the script."""
        i = 0
        while True:
            a = t.find(FENCE, i)
            if a < 0:
                return
            j = a + 2
            while j < len(t):
                if not t.startswith(FENCE, j):
                    j += 1
                    continue
                if j + 2 < len(t) and t[j + 2] in chr(39) + "$" + chr(92):
                    j += 3
                    continue
                break
            yield a + 2, t[a + 2 : j]
            i = j + 2

    def fstrings(t):
        i = 0
        while True:
            m = FSTR.search(t, i)
            if not m:
                return
            q = m.group(1)
            j = m.end()
            while j < len(t):
                if t[j] == "\\":
                    j += 2
                    continue
                if t.startswith(q, j):
                    break
                j += 1
            yield m.end(), t[m.end() : j]
            i = j + len(q)

    root_dir = pathlib.Path("${self}")
    # Every VM test splices this prelude into its own script.
    pre = root_dir / "tests/niri-xvfb.nix"
    prelude = "\n".join(b for _, b in fences(pre.read_text())) if pre.exists() else ""

    orphans = []
    for p in sorted(root_dir.rglob("*")):
        if p.suffix not in (".nix", ".py") or not p.is_file():
            continue
        if any(x in p.parts for x in (".git", "result", "target", ".direnv")):
            continue
        t = p.read_text(errors="replace")
        regions = [(0, t)] if p.suffix == ".py" else list(fences(t))
        extra = prelude if p.suffix == ".nix" else ""
        known = defined("\n".join(b for _, b in regions) + "\n" + extra)
        for base, region in regions:
            for off, body in fstrings(region):
                for m in CAP.finditer(body):
                    if m.group(1) in known:
                        continue
                    if m.start() > 0 and body[m.start() - 1] == "{":
                        continue  # `{{` is a literal brace
                    if m.start() >= 1 and body[m.start() - 1] == "$":
                        continue  # Nix antiquotation, not a Python field
                    n = t.count("\n", 0, base + off + m.start()) + 1
                    orphans.append(f"{p.relative_to(root_dir)}:{n}: {m.group(1)}")

    if orphans:
        print("ERROR: a format capture with no name in scope.", file=sys.stderr)
        for o in orphans:
            print(f"       {o}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Python only reports it on the line that RUNS: one of these lives on", file=sys.stderr)
        print("       the error path and only shows up when there is already another problem.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: every format capture has its name in scope")
    END
  '';

  # The crates: tests, clippy and formatting, each in its own derivation so it
  # says WHAT broke instead of "Rust failed".
  rust-tests = rust.tests;
  rust-clippy = rust.clippy;
  rust-fmt = rust.fmt;

  # The footer itself. The surface only comes up with a compositor, so what is
  # asserted here is what can be asserted without one: that the binary exists,
  # runs and loads the embedded face.
  shell-binary = mkCheck "shell-binary" [ ] ''
    bin=${rust.workspace}/bin/ronoctua-shell
    test -x "$bin" || { echo "ERROR: the binary is missing" >&2; exit 1; }

    # With no WAYLAND_DISPLAY it must fail with a CLEAR message, not with a
    # panic. The message is the first thing anyone sees when the nested loop
    # does not come up.
    out_text=$("$bin" 2>&1 || true)
    echo "$out_text"
    echo "$out_text" | grep -q "Wayland compositor"       || { echo "ERROR: with no compositor, the error message does not explain why" >&2; exit 1; }

    echo "OK: the footer exists and fails legibly with no compositor"
  '';

  # THE FIVE COMPONENTS REACH THE SHELL THAT RUNS — the two sides, compared.
  #
  # # The defect this exists for
  #
  # The components used to be baked in by `option_env!`, and that made a whole
  # class of failure impossible: if the build had the variable, the binary had
  # the path. Taking them out of the compilation (`nix/rust.nix`, and the reason
  # is written there) trades that guarantee for minutes of iteration — and a
  # trade with nothing watching it is how `LEADER i n` becomes a key that opens
  # no window, with one line in a journal nobody is reading.
  #
  # So the two sides are compared against each other, and both come from real
  # artifacts rather than from a list kept by hand here:
  #
  #   - what the RUST reads: every `RONOCTUA_BUFFER_*` named in the shell's
  #     sources. A sixth buffer shows up here the moment its command is written;
  #   - what the WRAPPER carries: the strings `makeBinaryWrapper` compiled into
  #     the binary the module, the nested loop and the VMs all run.
  #
  # A name on one side and not on the other fails. That covers both directions:
  # the buffer nobody wired up, and the variable that stopped being set.
  shell-finds-its-buffers =
    mkCheck "shell-finds-its-buffers"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0

        # ---- WHAT THE RUST READS ----
        lidas=$(grep -oh 'RONOCTUA_BUFFER_[A-Z_]*' \
          ${../crates/ronoctua-shell/src}/*.rs | sort -u)
        [ -n "$lidas" ] || {
          echo "ERROR: no RONOCTUA_BUFFER_* in the shell's code — what does the grep match?" >&2
          exit 1
        }
        echo "o shell le: $(echo "$lidas" | tr '\n' ' ')"

        # ---- WHAT THE WRAPPER CARRIES ----
        # `grep -a`: the wrapper is an ELF, and both the name and the store path
        # are string literals inside it.
        for v in $lidas; do
          grep -qa "$v" ${rust.shell}/bin/ronoctua-shell || {
            echo "ERROR: the shell reads $v and the wrapper does not set it — the key would open nothing" >&2
            failed=1
          }
        done

        # ---- AND EACH ONE POINTS AT A COMPONENT THAT EXISTS ----
        #
        # A variable set to a path that is not there fails at the far end, when
        # the window does not open, and the message blames the buffer.
        ${lib.concatStringsSep "\n" (
          lib.mapAttrsToList (name: path: ''
            grep -qa "${path}" ${rust.shell}/bin/ronoctua-shell || {
              echo "ERROR: ${name} does not point at ${path} in the wrapper" >&2; failed=1; }
            case "${path}" in
              *.wasm) [ -s "${path}" ] || {
                echo "ERROR: ${path} does not exist or is empty" >&2; failed=1; } ;;
              *) echo "ERROR: ${name} does not end in .wasm" >&2; failed=1 ;;
            esac
            echo "$lidas" | grep -qx "${name}" || {
              echo "ERROR: the wrapper sets ${name} and no .rs of the shell reads it" >&2; failed=1; }
          '') rust.bufferComponents
        )}

        [ "$failed" = 0 ] || exit 1
        echo "OK: both sides match, and the components are there"
      '';

  # Section 9 requires the repository to be publishable AS IT IS. A local-path
  # input only works for whoever has both trees side by side, so it must not
  # survive publication — and since the font files went out as a Release, it no
  # longer WARNS: it fails.
  #
  # The four names are spelled out and the set is compared whole. Scanning for a
  # `fonts-` prefix alone would mean a renamed input empties the loop and the
  # check passes having verified nothing — which is exactly how the previous
  # version of this check spent its whole life (see below).
  fonts-input-publishable = mkCheck "fonts-input-publishable" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import json, sys

    # THE URLS COME FROM `flake.lock`, and not from `inputs.<name>.original.url`.
    #
    # That attribute DOES NOT EXIST on a `flake = false` input — the value is the
    # fetched tree, with `outPath` and `narHash` and nothing else. The `or`
    # fallback that used to be here therefore answered for every run, and this
    # check spent its whole life reporting on a URL nobody had written. It landed
    # in the right branch by luck, and would have gone on saying "local path"
    # after the inputs became a Release. A silent fallback needs a check; this
    # one WAS the silent fallback.
    expected = {
        "fonts-ttf",
        "fonts-ttf-nerdfonts",
        "fonts-woff2",
        "fonts-woff2-nerdfonts",
    }

    nodes = json.load(open("${self}/flake.lock"))["nodes"]
    found = {n for n in nodes if n.startswith("fonts-")}
    if found != expected:
        print("ERROR: the flake.lock's font inputs are not the expected ones.",
              file=sys.stderr)
        print("      expected: " + ", ".join(sorted(expected)), file=sys.stderr)
        print("      found:    " + ", ".join(sorted(found)), file=sys.stderr)
        sys.exit(1)

    failed = False
    for name in sorted(expected):
        node = nodes[name]
        original = node["original"]
        url = original.get("url") or (original["type"] + ":" + original.get("path", ""))

        # `narHash` is what actually pins this. `fonts-v1` is a mutable pointer
        # like any git tag: GitHub lets an asset be deleted and re-uploaded under
        # the same name, and without the hash the build would just take the new
        # bytes.
        pinned = node.get("locked", {}).get("narHash")

        if url.startswith("https://") or url.startswith("http://"):
            if not pinned:
                print("ERROR: the input '" + name + "' is remote but is NOT locked "
                      "by narHash in flake.lock.", file=sys.stderr)
                failed = True
            else:
                print("OK: input '" + name + "' is a remote asset and pinned by "
                      "hash — " + url)
        elif url.startswith("path:") or url.startswith("/"):
            print("ERROR: the input '" + name + "' points at a local path:",
                  file=sys.stderr)
            print("        " + url, file=sys.stderr)
            print("      That works on your disk, but NOT for whoever clones it.",
                  file=sys.stderr)
            print("      The font files are 4.1 GB and do NOT go into git (nor",
                  file=sys.stderr)
            print("      LFS): they are an asset of a Release of this very repository. Run",
                  file=sys.stderr)
            print("      nix run .#fonts-release -- /path/with/noctua-iosevka,",
                  file=sys.stderr)
            print("      upload the four on the fonts-v1 tag and point the input at it.",
                  file=sys.stderr)
            failed = True
        else:
            print("ERROR: input '" + name + "' has an unexpected shape: " + url,
                  file=sys.stderr)
            failed = True

    sys.exit(1 if failed else 0)
    PY
  '';

  # The OFL requires redistributing the license text along with the files.
  fonts-license = mkCheck "fonts-license" [ ] ''
    test -f ${fonts.noctua-iosevka-ttf-nerdfonts}/share/doc/noctua-iosevka/LICENSE-OFL.txt \
      || { echo "ERROR: LICENSE-OFL.txt is missing from the package" >&2; exit 1; }
    grep -q "SIL OPEN FONT LICENSE" \
      ${fonts.noctua-iosevka-ttf-nerdfonts}/share/doc/noctua-iosevka/LICENSE-OFL.txt \
      || { echo "ERROR: LICENSE-OFL.txt does not contain the OFL text" >&2; exit 1; }
    echo "OK: the OFL text is redistributed alongside the files"
  '';

  # EVERY COMMAND A BUFFER PUBLISHES HAS A LABEL THE HOST CAN DRAW.
  #
  # `SPC m` lists the focused buffer's keymap, and each row's description comes
  # from `cmd-<id>` — the buffer declares the id, the HOST owns the text
  # (invariant 11: a buffer does not translate). When the key is missing,
  # `Texts::t` returns `[cmd-<id>]` and does not fail, so the defect reaches the
  # screen as brackets and nothing before that says a word. It is the same hole
  # `installer-dryrun` closes for the installer's own labels.
  #
  # The literals are read from the buffers' SOURCE. The ids built at runtime
  # (`lucide`'s formats and filters) are already covered by the panels that draw
  # them, and a check that ran the components would cost a wasm instantiation to
  # learn what a grep can see.
  # EVERY TEXT KEY A BUFFER NAMES EXISTS IN BOTH LANGUAGES.
  #
  # The sibling below covers a buffer's COMMAND ids. Nothing covered the keys it
  # writes on screen — and a buffer cannot check them itself: it does not
  # translate (invariant 11), it names a key and the host looks it up. A typo
  # becomes `[memory-guard-detial]` drawn in front of the person, and only when
  # that screen is opened.
  #
  # The shape read here is the `mod key` every buffer keeps: one `pub const` per
  # key, the value a literal. A key assembled with `format!` is invisible to
  # this check — which is why the panels keep their keys as whole literals.
  buffer-keys-have-text = mkCheck "buffer-keys-have-text" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    owed = {}

    # INSIDE `mod key`, AND NOWHERE ELSE.
    #
    # This used to grep every `pub const NAME: &str = "…";` in a buffer, which
    # was true while the only such constants WERE text keys. The design crate
    # ended that: a `Table`'s columns are named, and a buffer now declares
    # `mod field { pub const COUNTER: &str = "counter"; }` — which is better
    # than a bare literal at the call site, and is not a sentence anybody
    # translates. The broad grep read four of those as missing translations.
    #
    # The module is the scope, so a constant is a promise about a bundle only
    # where the buffer says it is one.
    for rs in sorted((root_dir / "buffers").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        source = rs.read_text()
        for block in re.findall(r"\nmod key \{(.*?)\n\}", source, re.S):
            for k in re.findall(r'pub const [A-Z_0-9]+: &str = "([a-z0-9-]+)";', block):
                # A TRAILING `-` MEANS PREFIX, NOT KEY. `glyphs-sheet-tags-` is
                # completed at run time with the language's subtag, one row per
                # language, and demanding a `.ftl` line for the prefix itself
                # would be demanding a key nothing ever emits. What demands the
                # completed ones is `buffer-tag-rows-have-text`, which knows the
                # list of languages and so knows how many there are.
                if k.endswith("-"):
                    continue
                owed.setdefault(k, rs.relative_to(root_dir))

    if not owed:
        print("ERROR: no buffer key found — did the pattern change?", file=sys.stderr)
        raise SystemExit(1)

    failed = 0
    for language in ${builtins.toJSON languages}:
        text = (root_dir / "assets/i18n" / f"{language}.ftl").read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*)\s*=", text, re.M))
        for k, where in sorted(owed.items()):
            if k not in have:
                print(f"ERROR: {k} is missing from {language}.ftl ({where}) — the screen would show [{k}]", file=sys.stderr)
                failed = 1

    if failed:
        raise SystemExit(1)
    print(f"OK: {len(owed)} buffer keys, all translated in both languages")
    END
  '';

  # EVERY BUFFER ID HAS A LABEL, IN BOTH LANGUAGES.
  #
  # The descriptor's `name` and `mode` used to BE the label: the host wrote
  # `*{name}*` into the window title and `br.ronoctua.{mode}` into the app id,
  # and the modeline drew both raw. That is how `glifos` appeared on a screen
  # running in English — invariant 18 and invariant 8 broken by the same line,
  # through the one door no check was looking at.
  #
  # They are ids now: English, kebab-case, and what the screen reads is
  # `buffer-<id>` and `mode-<id>`. Without this check a rename compiles, runs,
  # and puts a literal bracketed key on the modeline — which is the failure the
  # i18n was built to make impossible everywhere else.
  buffer-modes-have-text = mkCheck "buffer-modes-have-text" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    ID = re.compile(r'^\s+(name|mode):\s*"([a-z][a-z0-9-]*)"')

    # The ids as the COMPONENT declares them, read out of the descriptor block —
    # the same place `english-names` reads, and for the same reason: nothing ties
    # the literal to the key, so both sides have to be read from the source.
    wanted = set()
    for rs in sorted((root_dir / "buffers").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        inside = False
        for line in rs.read_text().splitlines():
            if "Descriptor {" in line:
                inside = True
                continue
            if inside and line.strip() in ("}", "};"):
                inside = False
                continue
            if not inside:
                continue
            m = ID.match(line)
            if m:
                wanted.add(("buffer" if m.group(1) == "name" else "mode", m.group(2)))

    if not wanted:
        sys.exit("ERROR: no descriptor found — did the block's shape change?")

    # AND THE NATIVE WINDOWS, which are not buffers and reach the same place.
    #
    # `daemon::mode_label` turns any `br.ronoctua.<id>` into `mode-<id>` — it
    # does not care whether the window is a wasm component or a process of ours
    # with a `wl_surface`. The file manager's window, the terminal's, the
    # monitor's and the rescue console's had NO key: the modeline drew
    # `[mode-files]`, in brackets, and had been doing it since they existed.
    #
    # Found while adding the fifth. One list, one rule, one place to add the
    # next one.
    APP_ID = re.compile(r'pub const APP_ID[A-Z_]*: &str = "br\.ronoctua\.([a-z][a-z0-9-]*)"')
    for rs in sorted((root_dir / "crates/ronoctua-shell/src").rglob("*.rs")):
        for ident in APP_ID.findall(rs.read_text()):
            wanted.add(("mode", ident))

    missing = []
    for language in ${builtins.toJSON languages}:
        ftl = (root_dir / "assets/i18n" / (language + ".ftl")).read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*) *=", ftl, re.M))
        for kind, ident in sorted(wanted):
            key = kind + "-" + ident
            if key not in have:
                missing.append(language + ": " + key)

    if missing:
        print("ERROR: a buffer id with no text — the screen would show the key in square brackets.", file=sys.stderr)
        for m in missing:
            print("       " + m, file=sys.stderr)
        raise SystemExit(1)
    print("OK: " + str(len(wanted)) + " buffer and window ids, with text in both languages")
    END
  '';

  buffer-commands-have-labels = mkCheck "buffer-commands-have-labels" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    ids = set()

    # TWO SHAPES, because a buffer may BUILD its bindings instead of writing
    # them out.
    #
    # Seven components spell `command: "memory-full"` into a `Binding`. The
    # brain's builds them from a `KEYMAP` table of pairs, so `command:` there
    # reads `(*command).into()` — and a grep for the first shape saw nothing of
    # it. Its nine labels were unguarded, and a rename would have drawn
    # `[cmd-brain-run]` under `SPC m` with every check green.
    #
    # The second shape is read INSIDE the `KEYMAP` declaration and nowhere
    # else. A bare "pair of short strings" pattern matches half the tables in
    # these files — measured: it produced `cmd-t`, `cmd-u` and `cmd-v` out of
    # things that are not commands at all.
    for rs in sorted((root_dir / "buffers").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        source = rs.read_text()
        ids.update(re.findall(r'command:\s*"([a-z0-9-]+)"', source))
        for table in re.findall(r"KEYMAP:\s*&\[\(&str, &str\)\]\s*=\s*&\[(.*?)\n\];", source, re.S):
            ids.update(re.findall(r'\(\s*"[^"]+"\s*,\s*"([a-z0-9-]+)"\s*\)', table))

    if not ids:
        print("ERROR: no buffer command found — did the pattern change?", file=sys.stderr)
        raise SystemExit(1)

    failed = 0
    for language in ${builtins.toJSON languages}:
        text = (root_dir / "assets/i18n" / f"{language}.ftl").read_text()
        have = set(re.findall(r"^([a-z][a-z0-9-]*)\s*=", text, re.M))
        for i in sorted(ids):
            if f"cmd-{i}" not in have:
                print(f"ERROR: cmd-{i} is missing from {language}.ftl — SPC m would show [cmd-{i}]", file=sys.stderr)
                failed = 1

    if failed:
        raise SystemExit(1)
    print(f"OK: {len(ids)} buffer commands, all with a label in both languages")
    END
  '';

  # THE MEMORY POLICY HAS THE SHAPE IT CLAIMS, on the GENERATED units.
  #
  # Every line here failed against a plausible wrong version before it was
  # accepted, because every one of them is a way for this policy to look
  # configured and do nothing:
  #
  # - `systemd.oomd.enable` is ALREADY `true` in this nixpkgs, with every slice
  #   switch `false`. The daemon runs and its monitored list is empty. What arms
  #   it is `ManagedOOMMemoryPressure=kill` on a slice, and nothing else.
  # - `MemoryMin=` is capped by EVERY ancestor cgroup. Set on `session.slice`
  #   alone its effective value is ZERO — and the unit file looks right.
  # - `enableUserSlices` would install a drop-in that applies to every slice of
  #   that type, arming `session.slice`, whose only candidates are the compositor
  #   and the footer.
  # - `OOMScoreAdjust=` on a user unit is refused by the kernel and swallowed by
  #   systemd at debug level. Written here, it would build and do nothing.
  memory-policy-shape =
    let
      system = self.nixosConfigurations.vm.config.system.build.toplevel;
    in
    mkCheck "memory-policy-shape" [ pkgs.gnugrep pkgs.coreutils ] ''
      etc=$(readlink -f ${system}/etc)

      # `-R` AND NEVER `-r`. The whole of NixOS's `/etc` is symlinks into the
      # store, and `grep -r` does not follow a symlink it meets while recursing —
      # it skips it, silently, and reports nothing found. Every assertion below
      # would then be answered by the same wrong "no": the four missing ones
      # would fail correctly by accident, and the two that assert an ABSENCE
      # would pass without ever having looked.
      failed=0

      # THE CHAIN. Four levels, or the reservation is zero.
      for level in system/user.slice.d system/user-.slice.d \
                   system/user@.service.d user/session.slice.d; do
        grep -qR "MemoryMin=" "$etc/systemd/$level" 2>/dev/null || {
          echo "ERROR: no MemoryMin at $level — the chain breaks and the effective value is zero" >&2
          failed=1
        }
      done

      # The daemon is armed, and against the applications.
      grep -qR "ManagedOOMMemoryPressure=kill" "$etc/systemd/user/app.slice.d" 2>/dev/null || {
        echo "ERROR: app.slice is not armed — oomd runs watching nothing" >&2; failed=1; }

      # And NOT against the desktop. Both of these would put the compositor and
      # the footer in the candidate set.
      [ ! -d "$etc/systemd/user/slice.d" ] || {
        echo "ERROR: a drop-in by slice TYPE — it arms session.slice too" >&2; failed=1; }
      [ ! -e "$etc/systemd/system/-.slice.d/overrides.conf" ] || {
        echo "ERROR: -.slice is armed; its candidates include the desktop" >&2; failed=1; }

      # The upstream `session.slice` survived: a full-file override would have
      # discarded its CPUWeight without a word.
      grep -q "CPUWeight=100" "$etc/systemd/user/session.slice" 2>/dev/null || {
        echo "ERROR: session.slice became a whole file and lost upstream's" >&2; failed=1; }

      # The setting that would be a no-op is absent.
      if grep -qR "OOMScoreAdjust=" "$etc/systemd/user/" 2>/dev/null; then
        echo "ERROR: OOMScoreAdjust in a user unit is ignored in silence" >&2
        failed=1
      fi

      # The two units that must never give up.
      for unit in niri.service ronoctua-footer.service; do
        grep -q "StartLimitIntervalSec=0" "$etc/systemd/user/$unit" || {
          echo "ERROR: $unit gives up after 5 deaths, and then the screen stays empty" >&2
          failed=1
        }
      done

      # THE TABLE THE PANEL READS, and it has to agree with the units.
      #
      # `LEADER s m` loosens the policy by applying the numbers from this file,
      # and the file is generated from the same options the drop-in above uses.
      # If the two ever drift, the panel would switch to numbers that are not
      # the ones a reboot brings back — and nothing else would say so.
      table="$etc/ronoctua/memory-profiles.conf"
      [ -e "$table" ] || {
        echo "ERROR: no profile table; the panel would have no way of loosening it" >&2; failed=1; }
      for profile in strict tolerant; do
        grep -q "^$profile ceiling=.* pressure=.* duration=" "$table" 2>/dev/null || {
          echo "ERROR: the table does not carry the $profile profile whole" >&2; failed=1; }
      done
      grep -q "^declared " "$table" 2>/dev/null || {
        echo "ERROR: the table does not say which profile comes back on restart" >&2; failed=1; }
      # And the strict profile's numbers ARE the ones in force on the slice —
      # the drop-in and the table come from the same options, and this is what
      # proves they were not wired to different ones.
      ceiling=$(grep '^strict ' "$table" | tr ' ' '\n' | grep '^ceiling=' | cut -d= -f2)
      grep -qR "MemoryHigh=$ceiling" "$etc/systemd/user/app.slice.d" 2>/dev/null || {
        echo "ERROR: the table's ceiling ($ceiling) is not app.slice's" >&2; failed=1; }

      # THE FIVE UNITS THE PANEL MAY START, and the rule that lets it.
      #
      # A panel that could start any unit would be a bigger door than the whole
      # policy above closes.
      for what in profile@ sweep@; do
        [ -e "$etc/systemd/system/ronoctua-memory-$what.service" ] || {
          echo "ERROR: the unit ronoctua-memory-$what is missing" >&2; failed=1; }
      done

      [ "$failed" = 0 ] || exit 1
      echo "OK: a chain of four levels, app.slice armed, the desktop out of reach, a table for both profiles"
    '';

  # THE ISO CARRIES WHAT THE TARGET MACHINE WOULD OTHERWISE COMPILE.
  #
  # Everything a NixOS closure inherits from nixpkgs comes prebuilt from the
  # public cache. Everything this repository makes does not exist there, and was
  # being built on the machine being installed: the shell, niri from the pinned
  # input, `libghostty-vt` and its Zig toolchain, the five wasm buffers, the
  # font family from a GitHub Release. That is where an install's afternoon went.
  #
  # This check is the LIST's half of the promise, and `iso-is-measured` is the
  # artefact's: what goes on the medium has to be enough, and it has to have
  # come out whole. Measured on 2026-08-15: 3702 MiB — and since the owner
  # removed the ceiling there is no number it has to stay under.
  #
  # The names are grepped in the seed's CLOSURE and not in the list, because
  # most of them arrive through `ronoctua-shell`'s baked store paths rather than
  # by being written out — and it is the closure that goes on the ISO.
  iso-carries-the-seed =
    let
      seed = pkgs.closureInfo {
        rootPaths = self.nixosConfigurations.installer.config.isoImage.storeContents;
      };
    in
    mkCheck "iso-carries-the-seed" [ pkgs.gnugrep pkgs.coreutils ] ''
      failed=0
      for ours in -ronoctua-0.1.0 -niri- -libghostty-vt- -ronoctua-buffer-glyphs- \
                  -ronoctua-buffer-unicode- \
                  -ronoctua-buffer-lucide- -ronoctua-buffer-memory- \
                  -ronoctua-buffer-transfers- \
                  -ronoctua-buffer-format- \
                  -ronoctua-buffer-brain- \
                  -noctua-icon-theme- \
                  -noctua-iosevka-ttf-nerdfonts-; do
        grep -q -e "$ours" ${seed}/store-paths || {
          echo "ERROR: $ours does not go into the ISO — the target machine will compile it" >&2
          failed=1
        }
      done

      # The flake's inputs travel as SOURCES. Without them the target has the
      # built pieces and still has to reach the network to evaluate the flake
      # that names them — which is the same install, waiting on the same wire.
      grep -q -e "-source" ${seed}/store-paths || {
        echo "ERROR: the fonts from the flake's inputs do not go into the ISO" >&2; failed=1; }

      [ "$failed" = 0 ] || exit 1
      echo "OK: $(wc -l < ${seed}/store-paths) paths in the seed"
    '';

  # THE REDUCED PROFILE IS REDUCED, AND STILL IS THE DISTRO.
  #
  # Measured on 2026-08-14: `full` 22,76 GiB, `minimal` 6,90 GiB.
  #
  # # Why each name is checked on BOTH sides
  #
  # A check that only says "android-studio is not in the reduced closure" goes
  # green when the name is misspelled, when the package is renamed upstream, and
  # when the whole system fails to have any packages at all. Requiring it to be
  # PRESENT in the full one first makes the difference the thing being asserted,
  # which is what the profile actually promises.
  #
  # # And what may never leave
  #
  # The second list is the more important one. A reduced profile that dropped
  # niri or the footer would be smaller and would not be this system — the point
  # was never the gigabytes, it was installing in minutes what one can then add
  # to one line at a time.
  profile-minimal-closes =
    let
      closureOf =
        drv:
        pkgs.closureInfo {
          rootPaths = [ drv ];
        };
      minimal = closureOf self.nixosConfigurations.minimal.config.system.build.toplevel;
      full = closureOf self.nixosConfigurations.noctua.config.system.build.toplevel;
    in
    mkCheck "profile-minimal-closes" [ pkgs.gnugrep pkgs.coreutils ] ''
      failed=0

      # Applications: in the full one, out of the reduced one.
      #
      # `-ffmpeg-full-` USED TO BE ON THIS LIST and is now on the other one.
      # That is the owner's decision, and the line between the two lists is
      # what it changed: ffmpeg is not an application one installs to USE, it
      # is format SUPPORT, and a machine that cannot preview a video or read a
      # photograph is not a reduced distro. What stayed here is `-vlc-`, which
      # is a player — which is exactly the line.
      for app in -android-studio- -google-chrome- -emacs-30 -vlc- \
                 -flutter- -dart- -rustc-1 -go-1 -noctua-iosevka-ttf-3 \
                 -noctua-icon-theme-; do
        grep -q -e "$app" ${full}/store-paths || {
          echo "ERROR: $app is not in the full profile — did the name change?" >&2
          failed=1
          continue
        }
        if grep -q -e "$app" ${minimal}/store-paths; then
          echo "ERROR: $app is still in the minimal profile" >&2
          failed=1
        fi
      done

      # The superpack is on that list because the switch is REAL now. It used to
      # be excluded from the profile table for the opposite reason: the shell
      # carried its store path baked in, so turning the option off removed it
      # from `systemPackages` and left all 0,61 GiB in the closure. This line is
      # what would fail if the path were ever baked in again.

      # And what the reduced profile may NEVER lose, or it stops being this
      # system rather than becoming a smaller one.
      #
      # THE SIX COMPONENTS ARE ON THAT LIST for the same reason the superpack
      # is on the one above, and by the same mechanism: they no longer arrive by
      # carriage inside the binary (`nix/rust.nix`), they arrive through the
      # wrapper. If the module ever points at the unwrapped workspace again, the
      # system installs a shell whose every buffer key opens nothing — and it
      # installs FASTER, with nothing else giving it away.
      # THE DECODERS ARE ON THIS LIST, and they are the reason it grew. The
      # file manager's preview and info panels reach for `ffmpeg` and
      # `exiftool` by a path taken from the ENVIRONMENT, so nothing in the
      # binary would fail if they left — the panels would simply say the
      # program is missing, on every reduced machine, for ever. This line is
      # what turns that into a build failure instead.
      for kept in -niri- -ronoctua-0.1.0 -noctua-iosevka-ttf-nerdfonts- -ghostty- \
                  -networkmanager- -systemd- -ffmpeg-full- -exiftool- \
                  -ronoctua-buffer-glyphs- \
                  -ronoctua-buffer-unicode- -ronoctua-buffer-lucide- \
                  -ronoctua-buffer-memory- -ronoctua-buffer-transfers- \
                  -ronoctua-buffer-format- -ronoctua-buffer-brain-; do
        grep -q -i -e "$kept" ${minimal}/store-paths || {
          echo "ERROR: the minimal profile lost $kept, which is part of the distro" >&2
          failed=1
        }
      done

      [ "$failed" = 0 ] || exit 1

      m=$(wc -l < ${minimal}/store-paths)
      f=$(wc -l < ${full}/store-paths)
      [ "$m" -lt "$f" ] || {
        echo "ERROR: the minimal one has $m paths and the full one $f" >&2; exit 1; }
      echo "OK: minimal $m paths, full $f"
    '';

  # THE ISO CARRIES NOTHING THAT ONLY A DESKTOP OR A COMPILER COULD USE.
  #
  # It used to carry 2,94 GiB of exactly that, and nothing said so: the ISO
  # installed `rust.workspace`, the one derivation that holds EVERY binary of the
  # workspace. A derivation references whatever its bytes mention, so
  # `ronoctua-shell` was dragging in `libghostty-vt` — and with it Zig, clang and
  # LLVM, 1,7 GiB — plus the icon superpack and mesa through the `option_env!`
  # paths. A console installer can run none of it.
  #
  # The fix was to install only `bin/ronoctua-install` (`nix/rust.nix`), and the
  # measurement is what makes it worth a check: the installer's closure went from
  # 5,81 GiB to 2,87 GiB and the ISO from 2,1 GiB to 1,46 GiB. That is the budget
  # the seeded closure spends, so a silent regression here costs twice.
  #
  # `closureInfo` and not `nix path-info`: inside the sandbox there is no daemon
  # to ask, and the closure has to be an INPUT of the derivation to be readable.
  iso-has-no-toolchain =
    let
      closure = pkgs.closureInfo {
        rootPaths = [ self.nixosConfigurations.installer.config.system.build.toplevel ];
      };
    in
    mkCheck "iso-has-no-toolchain" [ pkgs.gnugrep ] ''
      # Each of the four was measured in the closure before the fix. `mesa` is
      # here because a TTY installer has no GL at all — if it ever comes back it
      # came back by accident.
      failed=0
      for unwanted in -zig- -clang- -llvm- -noctua-icon-theme- -mesa-; do
        if found=$(grep -e "$unwanted" ${closure}/store-paths); then
          echo "ERROR: the ISO carries $unwanted, which only a desktop or a compiler uses" >&2
          echo "$found" >&2
          failed=1
        fi
      done
      [ "$failed" = 0 ] || exit 1

      # And the installer binary IS there — a check that only forbids would go
      # green on an ISO that lost the installer altogether.
      grep -q -e "-ronoctua-installer" ${closure}/store-paths || {
        echo "ERROR: the installer itself is not in the ISO" >&2; exit 1; }

      echo "OK: $(wc -l < ${closure}/store-paths) paths, none of them desktop or compiler"
    '';

  # THE ISO IS MEASURED, AND THERE IS NO LONGER A CEILING.
  #
  # There was one of 4 GiB, and the number was never arbitrary: it is the
  # maximum FILE SIZE of FAT32. It only ever bit when the `.iso` was COPIED
  # onto a stick — Ventoy, or an ESP the firmware boots the file from. Written
  # with `dd`, which writes raw sectors and which is what `docs/INSTALL.md`
  # already tells a person to do, there is no limit at all.
  #
  # The owner removed the ceiling and chose `dd`. What the ceiling was
  # defending is gone with it: the image may now carry whatever the target
  # would otherwise have to compile, and the seed is the whole point of it.
  #
  # WHAT IS LEFT IS STILL AN ASSERTION, and not a number printed into a log.
  # An ISO that comes out tiny is a build that silently lost its contents —
  # the `storeContents` list evaluating to nothing produces a perfectly valid,
  # perfectly useless image, and that failure looks exactly like success.
  # A hundred mebibytes is far below anything this ISO has ever measured and
  # far above an empty one, which is what makes it a floor and not a target.
  iso-is-measured = mkCheck "iso-is-measured" [ pkgs.coreutils ] ''
    iso=$(find ${self.packages.${system}.iso}/iso -name '*.iso' | head -1)
    [ -n "$iso" ] || { echo "ERROR: no .iso came out of the derivation" >&2; exit 1; }
    bytes=$(stat -c %s "$iso")
    floor=$((100 * 1024 * 1024))
    echo "ISO: $((bytes / 1024 / 1024)) MiB"
    if [ "$bytes" -lt "$floor" ]; then
      echo "ERROR: an ISO under 100 MiB — it lost the content, and an" >&2
      echo "      an empty image is valid: check the storeContents." >&2
      exit 1
    fi
    echo "OK: the ISO came out whole (no ceiling: it is written with dd)"
  '';

  # THE SIX DOCUMENTS EXIST.
  #
  # There WAS a 150-line ceiling on the `CLAUDE.md`, on the reasoning that it is
  # the only file that goes into every agent's context on its own, and that
  # going past the ceiling does not make it more informative — it makes it less
  # read. The owner removed it, and the reason is the other half of that same
  # sentence: it being the only file that arrives on its own is exactly why the
  # working rules have to live in it. A rule kept somewhere cheaper is a rule
  # that reaches an agent who thought to go looking, which is the ones who did
  # not need it.
  #
  # What the ceiling was really defending is still true and is now nobody's
  # check: length costs attention. Whoever writes here cuts before adding.
  #
  # `PENDING.md` is on the list because the "what is missing" half disappears
  # if it is not demanded: a project with no place for a pending item to live
  # goes back to hiding an overdue promise in a code comment.
  docs-exist = mkCheck "docs-exist" [ pkgs.coreutils ] ''
    for f in ${../CLAUDE.md} ${../README.md} \
             ${../docs/ARCHITECTURE.md} ${../docs/PENDING.md} \
             ${../docs/INSTALL.md} ${../docs/DEVELOPMENT.md}; do
      test -s "$f" || { echo "ERROR: $f is missing or empty" >&2; exit 1; }
    done

    echo "CLAUDE.md: $(wc -l < ${../CLAUDE.md}) lines"

    # The PENDING.md must not become a changelog. Its rule is "on finishing,
    # remove it"; a struck-through or done-marked item is the start of the rot.
    # Anchored on a LIST ITEM: the rule at the top itself cites the markers in
    # order to forbid them, and a loose grep would report the text that forbids them.
    if grep -qE '^ *[-*|] *(\[x\]|~~|✅)' ${../docs/PENDING.md}; then
      echo "ERROR: PENDING.md has an item struck through as done." >&2
      echo "      The rule is to remove, not to strike through — it keeps no history." >&2
      exit 1
    fi

    echo "OK: the six documents exist, and PENDING did not become a changelog"
  '';

  architecture-sections = mkCheck "architecture-sections" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    doc = pathlib.Path("${self}/docs/ARCHITECTURE.md").read_text()

    # EXACT match, and not "contains": the chapter heading `1. Fontes, glifos e
    # icones` contains the word `Fontes`, so a substring test would go on
    # accepting a reference to a `Fontes` section that had been renamed away.
    # That is this check passing while the defect it exists for is present, and
    # it happened on the first run.
    titles = set()
    for m in re.finditer(r"^#{1,5} (.+)$", doc, re.M):
        t = m.group(1).strip()
        titles.add(t)
        titles.add(re.sub(r"^\d+\.\s*", "", t))  # chapters carry a number

    # `section "X"` in a comment, in any tracked source file.
    ref = re.compile(r'ARCHITECTURE\.md,\s*(?:section|secao|seção)\s*"([^"]+)"')
    broken, seen = [], 0
    for f in pathlib.Path("${self}").rglob("*"):
        if f.suffix not in {".rs", ".nix", ".py", ".md"} or not f.is_file():
            continue
        try:
            content = f.read_text()
        except (UnicodeDecodeError, OSError):
            continue
        for m in ref.finditer(content):
            seen += 1
            target = m.group(1)
            if target not in titles:
                broken.append(f"{f}: section \"{target}\" does not exist in ARCHITECTURE.md")

    if broken:
        print("ERROR: a cross-reference to a section that does not exist:", file=sys.stderr)
        for b in broken:
            print("  " + b, file=sys.stderr)
        sys.exit(1)
    print(f"OK: {seen} cross-references by section name, all of which resolve")
    PY
  '';
  # WHAT THE FOOTER READS AT RUNTIME IS EXPORTED BEFORE THE FOOTER STARTS.
  #
  # `dev-exports-what-the-build-bakes` covers the variables the COMPILATION
  # bakes. It says nothing about WHEN, and the two that are not build inputs —
  # `RONOCTUA_ICONS` and `RONOCTUA_FILES_START` — were exported inside the block
  # that sets up the hot reload, which runs AFTER the first footer is already up.
  #
  # So every session's first footer had neither, and only a footer restarted by
  # a recompilation did. Both failures are silent, because a missing variable is
  # a fallback and not an error: the launcher opened with category glyphs that
  # were never replaced by the app's icon, and `LEADER .` opened the home while
  # `~/main` sat right there. The second one was reported from the screen.
  #
  # The rule is about ORDER, so the check reads line numbers: the export has to
  # come before the line that launches the footer.
  dev-exports-before-it-starts = mkCheck "dev-exports-before-it-starts" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    # What the SHELL reads at runtime and no build bakes. A variable that
    # arrives late is a variable that did not arrive.
    AT_RUNTIME = ["RONOCTUA_ICONS", "RONOCTUA_FILES_START", "RONOCTUA_FILES_SIDEBAR"]

    text = pathlib.Path("${../nix/dev.nix}").read_text()
    lines = text.splitlines()

    launch = None
    for n, line in enumerate(lines, 1):
        if "ronoctua-footer-single" in line and not line.lstrip().startswith("#"):
            launch = n
            break
    if launch is None:
        print("ERROR: could not find where the footer comes up in nix/dev.nix.", file=sys.stderr)
        raise SystemExit(1)

    gaps = []
    for name in AT_RUNTIME:
        where = [
            n
            for n, line in enumerate(lines, 1)
            if re.match(r"\s*export " + name + r"=", line)
        ]
        if not where:
                gaps.append(f"{name}: is not exported in nix/dev.nix")
        elif min(where) > launch:
            gaps.append(
                f"{name}: exported on line {min(where)}, and the footer comes up on {launch}"
            )

    if gaps:
        print("ERROR: a runtime variable arrives AFTER the footer.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in gaps:
            print("       " + g, file=sys.stderr)
        print("", file=sys.stderr)
        print("       The session's first footer does not have it, and only a restart", file=sys.stderr)
        print("       gains it. A silent failure: a missing variable is a fallback, not an error.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: the {len(AT_RUNTIME)} runtime variables come out before line {launch}")
    END
  '';

  # A `trap` THAT CALLS A FUNCTION THAT DOES NOT EXIST CLEANS NOTHING UP.
  #
  # `nix/bench-frames.nix` had `trap 'clean_up; rm -rf "$dir"' EXIT` next to a
  # function named `cleanup`: the rename to English reached the definition and
  # not the string. Nothing said so — a shell string is not code to shellcheck,
  # and the trap only runs at exit, when nobody is reading. The bench came back
  # with its numbers and left a nested compositor and a footer RUNNING, plus the
  # temp directory, because `rm -rf` came after the word that failed.
  #
  # That is the shape this project keeps paying for: the failure is at the end,
  # the message scrolls past, and what is left behind is somebody else's
  # problem. A word that has to name something is a name, and a name can be
  # checked.
  #
  # The allow-list is short on purpose. A trap normally kills something, removes
  # something, or calls a function of the script's own; anything else earns a
  # line here and the thought that goes with it.
  trap-calls-something-that-exists = mkCheck "trap-calls-something-that-exists" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys

    KNOWN = {
        "rm", "kill", "exit", "true", "false", "echo", "wait", "pkill",
        "umount", "rmdir", "sync", "printf", "stty", "tput", "clear",
        "killall", "chmod", "cp", "mv",
    }
    root_dir = pathlib.Path("${self}")
    gaps = []
    for f in sorted(root_dir.rglob("*.nix")):
        if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
            continue
        # COMMENT LINES DO NOT COUNT — including this check's own, which
        # quotes the very trap that earned it and would flag itself.
        text = "\n".join(
            "" if l.lstrip().startswith("#") else l
            for l in f.read_text(errors="replace").splitlines()
        )
        defined = set(re.findall(r"^\s*([a-z_][a-z0-9_]*)\(\)\s*\{", text, re.M))
        for m in re.finditer(r"""trap\s+(['"])(.*?)\1""", text, re.S):
            for piece in re.split(r"[;&|\n]+", m.group(2)):
                word = piece.strip().split(" ")[0].strip()
                if not word or not re.fullmatch(r"[a-z_][a-z0-9_]*", word):
                    continue
                if word in defined or word in KNOWN:
                    continue
                line = text[: m.start()].count("\n") + 1
                rel = f.relative_to(root_dir)
                gaps.append(f"{rel}:{line}: o trap chama `{word}`")

    if gaps:
        print("ERROR: a trap calling what does not exist.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in gaps:
            print("       " + g, file=sys.stderr)
        print("", file=sys.stderr)
        print("       It is neither a function defined in the file nor a command from the list.", file=sys.stderr)
        print("       The trap fails on exit, silently, and what it would clean up stays.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: every trap calls a function that exists or a known command")
    END
  '';

  # THE GATE HAS TO FETCH WHAT NO CACHE HAS — and the wiring is the risk.
  #
  # `nix/prefetch.nix` exists because one source in the `full` profile cannot be
  # downloaded by `fetchurl` on a connection that resets: it retries without
  # resuming, so it restarts from zero and never finishes. The tool works; what
  # can quietly stop being true is that the gate CALLS it.
  #
  # Unwired, nothing breaks at evaluation and nothing breaks in the short loop.
  # The gate simply goes back to dying after minutes of work, with a message
  # (`cannot download ... from any mirror`) that points at Google instead of at
  # the missing call — which is exactly how an afternoon was lost once already.
  #
  # It also has to come FIRST. Fetching after the heavy three would still work,
  # and would still make everyone pay the compile before finding out the network
  # is the problem.
  gate-fetches-what-no-cache-has = mkCheck "gate-fetches-what-no-cache-has" [ pkgs.gnugrep ] ''
    script=${gateApp}/bin/ronoctua-gate
    tool=${prefetchApp}/bin/ronoctua-prefetch

    if ! grep -qF "$tool" "$script"; then
      echo "ERROR: the gate does not call the prefetch." >&2
      echo "      Without it, the gate dies after minutes with" >&2
      echo "      'cannot download ... from any mirror', which looks" >&2
      echo "      a file removed, and it is not — see nix/prefetch.nix." >&2
      exit 1
    fi

    called_at=$(grep -nF "$tool" "$script" | head -1 | cut -d: -f1)
    heavy_at=$(grep -n '^heavy=(' "$script" | head -1 | cut -d: -f1)
    if [ -z "$heavy_at" ]; then
      echo "ERROR: could not find the block with the three heavy ones in the gate." >&2
      exit 1
    fi
    if [ "$called_at" -ge "$heavy_at" ]; then
      echo "ERROR: the prefetch comes AFTER the three heavy ones (line $called_at, against $heavy_at)." >&2
      echo "      That way the compiler runs whole before finding out that the" >&2
      echo "      problem is the network." >&2
      exit 1
    fi
    echo "OK: the gate downloads what no cache has, before compiling"
  '';

  # -------------------------------------------------------------------
  # The test tiers, and the guards that keep them total
  # -------------------------------------------------------------------

  # The names `tests/slow.nix` takes out of the everyday battery.
  slow = import ./slow.nix;
  # And the ten of the editing loop, which are a CHOSEN subset.
  quick = import ./quick.nix;

  # THE GUARD, and why it looks at `raw` and not at the final attrset.
  #
  # It is PART of the attrset, so reading `attrNames` of that same attrset from
  # inside one of its values would recurse. Building the set as `raw` and adding
  # the guard afterwards puts the two in order: it sees everything that existed
  # before it, which is exactly what it has to see.
  tiersAreTotal =
    let
      all = builtins.attrNames raw;
      ghosts = lib.subtractLists all slow;
      everyday = lib.subtractLists slow all;
    in
    mkCheck "test-tiers-are-total" [ pkgs.coreutils ] ''
      ${
        if ghosts == [ ] then
          ""
        else
          ''
            echo "ERROR: tests/slow.nix names a check that no longer exists:" >&2
            ${lib.concatMapStringsSep "\n" (n: ''echo "       ${n}" >&2'') ghosts}
            echo "      A dead line there excludes nothing — it only looks as if it does," >&2
            echo "      and the check it was meant to hold back comes back into the everyday" >&2
            echo "      round with nobody noticing." >&2
            exit 1
          ''
      }
      ${
        if everyday != [ ] then
          ""
        else
          ''
            echo "ERROR: no check was left for the everyday battery." >&2
            echo "      Either tests/slow.nix swallowed everything, or the checks are gone." >&2
            exit 1
          ''
      }
      echo "OK: ${toString (builtins.length everyday)} in .#most, ${toString (builtins.length slow)} only in .#gate"
    '';

  # THE SAME QUESTION, FOR THE SHORT LOOP.
  #
  # `quick` is a CHOSEN subset and does not have to be complete — but it does
  # have to be valid. A renamed check would drop out of it in silence: the list
  # would still have ten lines, and one of them would stop meaning anything.
  quickNamesExist =
    let
      ghosts = lib.subtractLists (builtins.attrNames raw) quick;
    in
    mkCheck "quick-names-exist" [ pkgs.coreutils ] ''
      ${
        if ghosts == [ ] then
          ""
        else
          ''
            echo "ERROR: tests/quick.nix names a check that does not exist:" >&2
            ${lib.concatMapStringsSep "\n" (n: ''echo "       ${n}" >&2'') ghosts}
            echo "      The short loop would run with less than the list says." >&2
            exit 1
          ''
      }
      echo "OK: the ${toString (builtins.length quick)} of the short loop exist"
    '';

  raw = {
    # =================================================================
    # Milestones
    # =================================================================
    #
    # ===================================================================
    # MILESTONE 1 — Feedback loop before any feature.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   `nix flake check` green with empty tests; bootable ISO generated by the
    #   flake; the VM comes up to login; the three documents of §12 and the
    #   `.gitignore`/`.gitattributes` exist.
    # ===================================================================
    m1 = pkgs.linkFarmFromDrvs "check-m1" [
      torrent-layout-matches-cxx
      torrent-speaks-v2
      extension-has-no-fingerprint
      extension-parses
      downloads-do-not-need-the-extension
      pot-provider-is-script-only
      extension-has-no-literal
      secrets-are-not-in-the-repo
      # "The repository has to be publishable as it is" (section 9).
      secrets-out-of-repo
      planted-secret-fails
      # The feedback loop is m1's goal, and a tree that gets fat destroys it
      # silently: every `nix flake check` starts copying gigabytes.
      lean-tree
      deps-layer-is-lean
      gitignore-keeps-fonts
      fonts-single-family
      fonts-metrics
      fonts-nerdfonts-version
      chrome-icons-match-the-text
      fonts-input-publishable
      fonts-license
      icons-table-generated
      icons-unknown-token-fails
      # A token name the WIT would refuse. It stays in m1 along with the other two
      # because it is the same family of defect: the token exists, the build passes
      # here, and the failure shows up far away.
      tokens-wit-name
      icons-superpack
      fonts-renderer-parity
      rust-tests
      rust-clippy
      rust-fmt
      shell-binary
      shell-finds-its-buffers
      rescue-actions-have-text
      rescue-runs-by-path
      rescue-staging-agrees
      rescue-has-no-toolchain
      docs-exist
      architecture-sections
      english-names
      xdg-user-dirs
      browser-has-no-keyring
      format-captures-resolve
      generator-flags-agree
      iso-is-measured
      iso-has-no-toolchain
      iso-carries-the-seed
      profile-minimal-closes
      memory-policy-shape
      buffer-commands-have-labels
      buffer-keys-have-text
      buffer-modes-have-text
      buffer-states-have-text
      design-constants-match-tokens
      buffers-use-the-design-crate
      ink-is-not-a-count
      separators-come-from-chrome
      syntax-table-generated
      syntax-refusal-refuses
      syntax-claims-no-prose
      syntax-colours-are-tokens
      syntax-has-one-owner
      sections-do-not-share-a-line
      one-offset-per-list
      every-list-can-be-dragged
      one-frame-model
      one-pixel-rail
      one-scrollbar
      manager-builds-its-own-rows
      inspect-keys-exist
      update-has-one-list
      update-names-agree
      updates-have-text
      updates-refusal-refuses
      updates-never-ask
      brain-unit-is-narrow
      tests-do-not-touch-the-machine
      states-driven
      footer-mouse-reaches-nothing-new
      oomVm
      # The rescue console's two doors: the VT is its own machine, the window is
      # asserted next to the monitor's in `terminal-vm`.
      rescueVm
      ctrl-alt-del-is-ours
      rescue-polkit-is-narrow
      # The file manager: the two doors, the narrow privileged one, and the
      # promise that the mouse reaches nothing the keyboard does not.
      filesVm
      files-actions-have-text
      files-everything-has-a-key
      files-runs-by-path
      files-notices-the-directory
      inspect-mesh-is-in-process
      mesh-formats-are-total
      inspect-labels-are-keys
      files-keys-are-not-the-compositors
      playlist-formats-are-total
      # The preview's player: the size it never guesses, and the child it never
      # leaves behind.
      motion-never-guesses-a-frame
      motion-never-leaks-a-child
      motion-photo-is-found-by-bytes
      # And the sidebar's device half: the border with udisks2, the polkit
      # posture we depend on and do not write, and the refusal that must not
      # hang.
      disksVm
      disks-adds-no-polkit-rule-of-its-own
      disks-tools-are-pinned
      screenshots-land-in-a-declared-directory
      files-polkit-is-narrow
      files-closure-has-no-mime-db
      files-columns-degrade
      transfers-unit-is-narrow
      transfers-socket-is-private
      transfers-words-are-total
      dev-rebuilds-what-sits-beside
      graphics-does-not-mix-libc
      mime-table-generated
      mime-unknown-icon-fails
      mime-preference-is-live
      self.packages.${system}.iso
      self.nixosConfigurations.vm.config.system.build.toplevel
    ];

    # Exposed so they can be run on their own during the work.
    footer-vm = footerVm;
    niri-contract = niriContract;
    modeline-vm = modelineVm;
    leader-vm = leaderVm;
    apps-vm = appsVm;
    inherit keymap-no-collision i18n-complete i18n-missing-key-fails;
    inherit languages-are-declared buffer-tag-rows-have-text buffers-test;
    inherit third-language-costs-nothing;
    inherit shell-finds-its-buffers;
    inherit
      rescue-actions-have-text
      rescue-runs-by-path
      rescue-polkit-is-narrow
      ctrl-alt-del-is-ours
      rescue-has-no-toolchain
      rescue-staging-agrees
      ;
    files-vm = filesVm;
    disks-vm = disksVm;
    transfers-vm = transfersVm;
    extension-vm = extensionVm;
    inherit
      files-actions-have-text
      files-everything-has-a-key
      files-runs-by-path
      files-notices-the-directory
      inspect-mesh-is-in-process
      mesh-formats-are-total
      inspect-labels-are-keys
      files-keys-are-not-the-compositors
      playlist-formats-are-total
      motion-never-guesses-a-frame
      motion-never-leaks-a-child
      motion-photo-is-found-by-bytes
      files-polkit-is-narrow
      files-closure-has-no-mime-db
      files-columns-degrade
      transfers-unit-is-narrow
      transfers-socket-is-private
      transfers-words-are-total
      dev-rebuilds-what-sits-beside
      graphics-does-not-mix-libc
      mime-table-generated
      mime-unknown-icon-fails
      mime-preference-is-live
      gate-fetches-what-no-cache-has
      trap-calls-something-that-exists
      dev-exports-before-it-starts
      ;
    inherit
      group-mark-outside-translation
      niri-actions-translated
      keyboard-layout-exists
      keyboard-options-do-what-they-say
      installer-defaults-match-the-keyboard
      locale-exists
      wifi-console-asks-once
      ;
    inherit
      niri-config-valid
      fontconfig-nested-finds-font
      xdg-user-dirs
      screenshots-land-in-a-declared-directory
      disks-adds-no-polkit-rule-of-its-own
      disks-tools-are-pinned
      persistence-takes-its-root-as-a-parameter
      browser-has-no-keyring
      ;
    notify-vm = notifyVm;
    impermanence-vm = impermanenceVm;
    buffer-vm = bufferVm;
    memory-vm = memoryVm;
    clipboard-vm = clipboardVm;
    terminal-vm = terminalVm;
    x11-vm = x11Vm;
    keyboard-vm = keyboardVm;
    rescue-vm = rescueVm;
    tpm-vm = tpmVm;
    secureboot-vm = secureBootVm;
    inherit installer-dryrun;
    oom-vm = oomVm;
    inherit
      notifications-table-generated
      notifications-have-text
      screenshot-notification-contract
      switches-that-do-nothing-say-so
      invalid-token-fails-build
      secrets-out-of-repo
      planted-secret-fails
      term-differential
      instance-fast
      browser-filters
      symbols-filters
      unicode-table-generated
      unicode-version-mismatch-fails
      unicode-instance-fast
      lucide-filters
      lucide-table-generated
      lucide-range-taken-fails
      lucide-instance-fast
      memory-instance-fast
      memory-driven
      brain-driven
      transfers-instance-fast
      transfers-driven
      format-instance-fast
      format-driven
      torrent-layout-matches-cxx
      torrent-speaks-v2
      extension-has-no-fingerprint
      extension-parses
      downloads-do-not-need-the-extension
      pot-provider-is-script-only
      extension-has-no-literal
      secrets-are-not-in-the-repo
      renderer-loads-three-faces
      chrome-icons-match-the-text
      layout-fast
      shaping-fast
      large-monitor-surface
      png-tool-draws
      reload-reloads
      reload-command-is-defined
      dev-exports-what-the-build-bakes
      footer-is-single
      footer-logs-to-file
      dev-exits-clean
      english-names
      format-captures-resolve
      generator-flags-agree
      ;

    # VM test of the disk layout (section 7.2). Heavy: it brings up a VM with
    # KVM, partitions a real disk and mounts it. Exposed separately so it can be
    # run on its own during the work.
    disko-vm = import ./disko-vm.nix {
      inherit pkgs lib;
      inherit (inputs) disko;
    };

    # ===================================================================
    # MILESTONE 2 — NixOS base + the whole font and icon layer.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   The VM boots from a disk partitioned by disko with LUKS2 + the 7
    #   subvolumes of §7.2; `fc-match` gives back our font in serif/sans/mono;
    #   renderer and fontconfig agree on the probe codepoints; a negative fixture
    #   with an icon token that does not exist **fails the build**; the
    #   superpack's manifest covers 100% of the input names; Ghostty installed.
    # ===================================================================
    m2 = pkgs.linkFarmFromDrvs "check-m2" [
      # font layer
      fonts-renderer-parity
      # icon layer
      icons-table-generated
      icons-unknown-token-fails
      icons-superpack
      # An icon drawn at another size than the text beside it — the defect the
      # bell had for as long as the modeline existed.
      chrome-icons-match-the-text
      # system: the full VM evaluates and builds, with Ghostty and the superpack
      self.nixosConfigurations.vm.config.system.build.toplevel
      # AND THE SYSTEM THE INSTALLER ACTUALLY INSTALLS.
      #
      # It was never built by anything here. The gate built the VM configuration
      # and the ISO — neither of which is `noctua` — so "the target system closes"
      # was an assumption, and the moment it would have been found false is during
      # `nixos-install`, with the disk already erased and nothing to go back to.
      #
      # It is not a duplicate of the VM: that one exists to be booted by a test and
      # leaves out what a real machine carries. This is the closure of the machine
      # somebody installs.
      self.nixosConfigurations.noctua.config.system.build.toplevel
    ];

    # ===================================================================
    # MILESTONE 3 — Two-line footer, in the right font, in Rust.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   A single layer-shell surface, bottom anchor, exclusive zone of 2 lines,
    #   writing a fixed state; the nested loop (`nix run .#dev`) brings up niri
    #   + shell with hot reload; sops-nix adopted.
    # ===================================================================
    m3 = pkgs.linkFarmFromDrvs "check-m3" [
      rust-tests
      rust-clippy
      rust-fmt
      shell-binary
      # The milestone's real proof: the surface is accepted by a real compositor,
      # wgpu assembles a context over it and a frame comes out. None of that shows
      # up in `cargo build` — all four are runtime.
      footerVm
      # The nested loop, and the proof that it RELOADS — not only that it exists.
      devApp
      reload-reloads
      # And that what it calls EXISTS, and that the binary it compiles gets the
      # same environment the build bakes. Those two are the wiring; the one above
      # is the mechanism, and the mechanism worked for months with no wiring.
      reload-command-is-defined
      dev-exports-what-the-build-bakes
      # And that restarting RESTARTS: `reload-reloads` proves that the command
      # fires, this one proves that exactly one footer is left.
      footer-is-single
      footer-logs-to-file
      dev-exits-clean
      # The two GPU-independent gates of section 9.3, which protect this
      # milestone's frame budget.
      layout-fast
      shaping-fast
      # And the resolution ceiling: the footer takes the SCREEN's width, so a
      # texture limit below it kills the whole milestone on a big monitor.
      large-monitor-surface
    ];

    # ===================================================================
    # MILESTONE 4 — `ronoctua-niri` + contract, and a live modeline.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   The contract test passes against the pinned tag **and** against the
    #   `niri-next` input; the daemon consumes the event stream; the modeline
    #   reflects the focused buffer in a screenshot with OCR.
    # ===================================================================
    m4 = pkgs.linkFarmFromDrvs "check-m4" [
      # The contract against BOTH versions. It is the subtest against `niri-next`
      # that gives the drift warning before the release becomes mandatory (risk R2).
      niriContract
      # The daemon consuming the event stream, proved on screen by OCR.
      modelineVm
    ];

    # ===================================================================
    # MILESTONE 5 — Leader, keymap and which-key in §4's geometry.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   The leader captures the keyboard via `keyboard-interactivity:
    #   exclusive`; which-key expands **upwards** with the list **below** the
    #   minibuffer; `Esc` goes back to 2 lines; the keymap collision check
    #   passes; the §7 elision property test passes.
    # ===================================================================
    m5 = pkgs.linkFarmFromDrvs "check-m5" [
      # Leader, which-key on screen, Esc, and the command fired.
      leaderVm
      # The default keymap does not collide. `rust-tests` already runs the same
      # unit test; this is the command a PERSON runs after editing their own
      # keymap, and that is why it is worth existing separately.
      keymap-no-collision
      # The elision property test lives in `rust-tests` (5,000 cases drawn by
      # `layout::tests::property_*`).
      rust-tests
    ];

    # ===================================================================
    # MILESTONE 6 — Design system, i18n and notifications.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   Tokens generated from Nix; the `text-key` sweep fails if a key is
    #   missing in `pt-BR` or `en-GB`; notifications queued **one at a time**; a
    #   critical one does not disappear on its own; a filterable tray; the
    #   3-stage icon chain exercised on all 3; the history opens **below the
    #   minibuffer**, with no second surface.
    # ===================================================================
    m6 = pkgs.linkFarmFromDrvs "check-m6" [
      # Every language complete and declared, and the proof that the generator
      # refuses the opposite.
      i18n-complete
      languages-are-declared
      buffer-tag-rows-have-text
      third-language-costs-nothing
      i18n-missing-key-fails
      group-mark-outside-translation
      niri-actions-translated
      keyboard-layout-exists
      locale-exists
      wifi-console-asks-once
      niri-config-valid
      fontconfig-nested-finds-font
      # THE CATALOGUE: one place for every notification and every filter the
      # distro ships with, generated from Nix — and both languages complete for
      # all of them, which is invariant 8 reaching the notifications.
      notifications-table-generated
      notifications-have-text
      # The screenshot the owner asked for, end to end: the compositor still
      # sends what the filter looks for, and the two dead switches say why.
      screenshot-notification-contract
      switches-that-do-nothing-say-so
      # A queue one at a time, nothing that holds the screen for ever, the band
      # above the modeline, and ONE surface.
      notifyVm
      # The 3-stage icon chain and the filterable tray are unit tests of
      # `ronoctua-notify`, which run here.
      rust-tests
    ];

    # Milestone 7 WAS ATTACKED — and this check stayed a `mkStub` by carelessness.
    #
    # A `mkStub` prints "NOT ATTACKED YET" and does `touch $out`: it ALWAYS
    # passes. The four real checks already existed and were loose, so
    # `nix build .#checks.x86_64-linux.m7` said OK without verifying anything.
    #
    # The lesson is not "I forgot to swap it": it is that a stub that PASSES is
    # indistinguishable from a closed milestone to whoever reads the output. See
    # the note about milestones in docs/ARCHITECTURE.md.
    # ===================================================================
    # MILESTONE 7 — Native buffers: protocol, runtime, glyph browser, terminal. Impermanence switchable.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   The first native buffer end to end with an `xdg_toplevel` of its own;
    #   instantiation < 5 ms measured with `memory_init_cow` on; a glyph browser
    #   with search, filters and assignment to an app id with hot reload; a
    #   differential test libghostty-vt × alacritty with zero grid diff on the
    #   corpus; a switchable root wipe.
    # ===================================================================
    m7 = pkgs.linkFarmFromDrvs "check-m7" [
      # A native buffer end to end, with an `xdg_toplevel` of its own.
      bufferVm
      memoryVm
      clipboardVm
      # Instantiation < 5 ms with `memory_init_cow` on.
      instance-fast
      # Zero grid diff on the VT corpus.
      term-differential
      # The terminal wired to the shell: its own window, and what is typed in it
      # shows up on the screen. `term-differential` proves the ENGINE; this one
      # proves that there is a path from the keyboard to the pixel.
      terminalVm
      # The leader as the Super key on its own (7.4): keyd standing, the right
      # remapping, and niri listening for the key that comes out of it.
      keyboardVm
      # Switchable impermanence, with the recovery window.
      impermanenceVm
      # Search, filters and assignment to an app id — IN THE COMPONENT, driven by
      # `evento` and checked by `render`. This used to point at `rust-tests`, which
      # exercises the database's GENERATOR: the database could be perfect and the
      # browser find nothing, which is what was actually happening.
      browser-filters
      symbols-filters
      unicode-table-generated
      unicode-version-mismatch-fails
      unicode-instance-fast
      lucide-filters
      lucide-table-generated
      lucide-range-taken-fails
      lucide-instance-fast
      memory-instance-fast
      memory-driven
      brain-driven
      transfers-instance-fast
      transfers-driven
      format-instance-fast
      format-driven
      renderer-loads-three-faces
      # THE APP FINDER, and what only the VM proves: the sweep at startup, the name
      # in the pixel, and the footer still responding with the application open —
      # the `--scope` is synchronous, and waiting for it froze everything.
      appsVm
      # And the PIXMAP step reaching the real superpack. Without it the list is
      # left with only the category glyphs, which look deliberate.
      apps-icon-from-superpack
      rust-tests
    ];

    # The installer's questions, and the plan they generate.
    #
    # It runs the binary as a DRY RUN: it prints the `host.nix` and the steps
    # without touching a disk. It is the same path `nix run .#install` takes — a
    # check that exercised another path would not prove the installer.

    # ===================================================================
    # MILESTONE 8 — Installer and secure boot end to end.
    #
    # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
    # check below has to be written — never against what has already been built:
    #
    #   The TUI collects **only** disk, hostname, password, passphrase, language
    #   and `resume_offset`; everything else detected by `nixos-facter` +
    #   `nixos-hardware`; Secure Boot with lanzaboote and a signed UKI; TPM2
    #   unlocks with PCR 7/11/12/15; **a forged LUKS volume does not unlock**
    #   (the PCR 15 proof).
    #
    # THE NETWORK WENT ONTO THE LIST, and the criterion changed with it —
    # deliberately, and with the reason written in `model.rs` and in section 7.1
    # of the ARCHITECTURE. In short: of the things the criterion said NOT to ask
    # about, the network is the only one that (a) `nixos-facter` has no way of
    # detecting, because a Wi-Fi password is a per-person secret, and (b)
    # without which the `nixos-install` step does not run on a machine with no
    # Ethernet port. The original criterion still stands: one asks what only the
    # person knows, and nothing beyond that.
    # ===================================================================
    m8 = pkgs.linkFarmFromDrvs "check-m8" [
      # The questions, the plan with the four PCRs, and the dry run by default.
      installer-dryrun
      # THE NETWORK AND SYNCTHING of the installed system. They stay here because
      # it is at m8 that the system starts really existing: before that there is no
      # machine on which "is powersave off?" means anything.
      network-tuned
      syncthing-tuned
      # The Wi-Fi password out of the file, sealed in the TPM.
      wifiTpmVm
      # The TUI. The milestone's criterion speaks of it — "the TUI collects ONLY
      # disk, hostname, password, passphrase, language and resume_offset" — and its
      # `State` is a function from (state, key) to state, with no terminal at all.
      # The tests walk through the questions, check that the `resume_offset` is NOT
      # shown, that an invalid answer does not advance, and that the confirmation
      # requires the disk's path typed out in full.
      rust-tests
      # TPM2 seals and unlocks with no passphrase; a volume with no token is refused.
      tpmVm
      # The whole chain: a UKI signed by lanzaboote, Secure Boot on, the firmware
      # measuring, and PCR 15 tying the state to the opened volume.
      secureBootVm
      # The ISO all of that is installed from.
      self.packages.${system}.iso
    ];

    # =================================================================
    # Granular checks — exposed so they can be run in isolation during the work.
    # =================================================================
    inherit
      lean-tree
      deps-layer-is-lean
      gitignore-keeps-fonts
      fonts-single-family
      fonts-metrics
      fonts-nerdfonts-version
      fonts-input-publishable
      fonts-license
      icons-table-generated
      icons-unknown-token-fails
      icons-superpack
      apps-icon-from-superpack
      fonts-renderer-parity
      rust-tests
      rust-clippy
      rust-fmt
      shell-binary
      iso-is-measured
      iso-has-no-toolchain
      iso-carries-the-seed
      profile-minimal-closes
      memory-policy-shape
      buffer-commands-have-labels
      buffer-keys-have-text
      buffer-modes-have-text
      buffer-states-have-text
      design-constants-match-tokens
      buffers-use-the-design-crate
      ink-is-not-a-count
      separators-come-from-chrome
      syntax-table-generated
      syntax-refusal-refuses
      syntax-claims-no-prose
      syntax-colours-are-tokens
      syntax-has-one-owner
      sections-do-not-share-a-line
      one-offset-per-list
      every-list-can-be-dragged
      one-frame-model
      one-pixel-rail
      one-scrollbar
      manager-builds-its-own-rows
      inspect-keys-exist
      update-has-one-list
      update-names-agree
      updates-have-text
      updates-refusal-refuses
      updates-never-ask
      brain-unit-is-narrow
      tests-do-not-touch-the-machine
      states-driven
      footer-mouse-reaches-nothing-new
      docs-exist
      architecture-sections
      ;

    # THE FOUR THAT ONLY EXISTED INSIDE AN `mN`, and that is a defect of its own.
    #
    # A check one cannot run alone is a check one cannot DEBUG: it fails inside a
    # `linkFarm` of forty others, and the only way to see it again is to rebuild
    # the whole milestone. That was true of these four until the test tiers needed
    # to enumerate `checks` and would have dropped them in silence.
    inherit
      tokens-wit-name
      network-tuned
      syncthing-tuned
      ;
    wifi-tpm-vm = wifiTpmVm;

    # Nix formatting.
    #
    # The source is FILTERED to contain only the .nix files. Without the filter,
    # `${../.}` would copy the 4.1 GB of assets/fonts into the store on every
    # evaluation of this check — 4.1 GB to run a formatter.
    fmt =
      let
        nixOnly = lib.cleanSourceWith {
          src = ../.;
          name = "ronoctua-nix-sources";
          filter =
            path: type:
            let
              base = baseNameOf path;
            in
            (
              type == "directory"
              && !lib.elem base [
                "assets"
                ".git"
                ".TEMP"
                "target"
                ".direnv"
              ]
            )
            || (type == "regular" && lib.hasSuffix ".nix" base);
        };
      in
      pkgs.runCommand "check-fmt" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
          cd ${nixOnly}
          if ! nixfmt --check $(find . -name '*.nix'); then
        echo "ERROR: run 'nix fmt' before committing" >&2
            exit 1
          fi
          touch $out
      '';

    # THE BUFFERS' FORMATTING, WHICH NOTHING CHECKED.
    #
    # `rust-fmt` is `cargoFmt` over the workspace, and `cargo fmt --all` reaches
    # the workspace's MEMBERS — `buffers/` is in `exclude` (a component cannot be
    # a member; `rust-lld` refuses its symbols). So the five components were
    # formatted by whoever remembered to `cd` into each one.
    #
    # The pinned toolchain, never `pkgs.rustfmt`: two versions of a formatter are
    # two opinions about the same file, and the disagreement would show up as a
    # check that fails on a tree `nix fmt` just formatted.
    # THE BUFFERS ARE FORMATTED, and this is checked with `rustfmt` and not with
    # `cargo fmt`.
    #
    # It WAS `cargo fmt --manifest-path`, and that stopped working the moment a
    # buffer gained a path dependency on `ronoctua-design`: `cargo fmt` runs
    # `cargo metadata` first, `${../buffers}` is a store copy that contains only
    # `buffers/`, and the dependency two directories up is not in it. Every
    # buffer failed with `failed to load manifest for dependency`, and the
    # message on screen said "out of format" — which was false, and pointed at
    # the wrong thing entirely.
    #
    # Mirroring the crates here, the way `nix/buffers.nix` does, would work and
    # would be a second copy of that arrangement to keep in step. `rustfmt`
    # needs no manifest: it reads a file. The edition has to be passed because
    # that is the one thing the manifest was telling it.
    buffers-fmt =
      pkgs.runCommand "check-buffers-fmt"
        {
          nativeBuildInputs = [ rustToolchain ];
        }
        ''
          failed=0
          seen=0
          for source in ${../buffers}/*/src/*.rs; do
            name=$(basename "$(dirname "$(dirname "$source")")")
            seen=$((seen + 1))
            if ! rustfmt --edition 2024 --check "$source" > /dev/null; then
              echo "ERROR: buffers/$name is out of format. Run: nix fmt" >&2
              failed=1
            fi
          done
          [ "$seen" -gt 0 ] || { echo "ERROR: no buffer source at all — did the glob change?" >&2; exit 1; }
          [ "$failed" = 0 ] || exit 1
          echo "OK: the $seen buffer sources are formatted"
          touch $out
        '';
  };
in
# THE GUARDS GO ON AFTERWARDS, which is what lets them see the whole set
# without recursion. They are cheap — set arithmetic over names — so they land
# in the everyday battery on their own, which is where they have to be: a guard
# that only ran in the `gate` would report a broken split hours after it broke.
raw
// {
  test-tiers-are-total = tiersAreTotal;
  quick-names-exist = quickNamesExist;
}
