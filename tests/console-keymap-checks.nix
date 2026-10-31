# THE CONSOLE TYPES THE SESSION'S KEYBOARD (2026-10-31): the checks of the
# console's maps (`nix/console-keymaps.nix`) and of the two consoles that load
# them — the installer's and the machine's, in its initrd too.
#
#   - `console-keymaps-cover-the-answers`: the installer's maps, read against
#     `base.lst` with a parser of its own: one per answer the installer offers
#     (each layout plain and with each variant), every one read back by
#     `loadkeys`; the answers left out are exactly those whose layout has no
#     symbols file, and nothing else is missing. And what must be refused is:
#     `us(bogus)` — which `ckbcomp` answers with the plain `us` and exit 0 —,
#     `custom`, and a `kbd` name (`uk`) given where an xkb answer goes. The
#     machine's map of the default answer is the installer's, byte for byte.
#
#   - `console-keymap-is-the-answers`: two machines as `lib.machine` builds
#     them — the reference, whose `host.nix` is the installer's golden one and
#     carries `ronoctua.keyboard.console = "us(euro)"`, and the BIOS probe
#     (`de(nodeadkeys)`), whose `host.nix` predates the line and takes the
#     derived default. Each one's `console.keyMap` is the installer's map of
#     its answer, byte for byte; the system's `/etc/vconsole.conf` and the
#     initrd's both name it; the initrd copies it in, and sets the console up
#     early. That the Rust and the Nix spell an answer the same way is proved
#     here: the line the installer writes is the name this builds.
#
#   - `console-keymap-vm`: the machine's console types what its answer types,
#     in the initrd too. A UK machine (`gb`) whose root is LUKS: the disk's
#     passphrase holds `" @ # ~`, set from a shell, and it is typed on the
#     KEYBOARD at the initrd's prompt — the four keys that give those on a UK
#     map give `@ " \ |` on a US one, so the disk opens only if the prompt
#     reads the UK map. Booted, a line typed on a console comes out the same.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  self,
  mkCheck,
  machineLib,
  # The console module every machine and the ISO import, and what `base.nix`
  # and it read of the distro (`fonts`, the themes, …).
  consoleModule,
  ronoctua,
}:
let
  keymaps = import ../nix/console-keymaps.nix { inherit pkgs; };
  keyboard = import ../nix/keyboard-xkb.nix;
  xkb = "${pkgs.xkeyboard_config}/share/X11/xkb";

  # What a machine's console is, as its evaluation says it.
  consoleOf =
    config:
    let
      keyMap = toString config.console.keyMap;
    in
    {
      answer = config.ronoctua.keyboard.console;
      inherit keyMap;
      early = config.console.earlySetup;
      system = "${config.environment.etc."vconsole.conf".source}";
      initrd = "${config.boot.initrd.systemd.contents."/etc/vconsole.conf".source}";
      copied = lib.any (p: p.enable && toString p.source == keyMap) config.boot.initrd.systemd.storePaths;
    };
  machines = {
    "us(euro)" = consoleOf self.nixosConfigurations.noctua.config;
    "de(nodeadkeys)" = consoleOf (machineLib.machine { dir = ./fixtures/machine-bios; }).config;
  };
in
{
  console-keymaps-cover-the-answers = mkCheck "console-keymaps-cover-the-answers" [ pkgs.python3 ] ''
    python3 - ${keymaps.every} ${xkb} ${keymaps.one keyboard.console} <<'END'
    import os, re, subprocess, sys
    every, xkb, default_map = sys.argv[1:4]
    wrong = []

    # BASE.LST, read here and not by the build's parser: the layouts of
    # `! layout`, and each variant of `! variant` under the layout its
    # description names first (`abnt2  br: Portuguese (Brazil, ABNT2)`).
    answers, section = [], None
    for line in open(f"{xkb}/rules/base.lst", encoding="utf-8"):
        if line.startswith("!"):
            section = line.split()[1] if len(line.split()) > 1 else None
            continue
        found = re.match(r"\s+(\S+)\s+(.*)", line)
        if not found:
            continue
        if section == "layout":
            answers.append((found.group(1), ""))
        elif section == "variant":
            answers.append((found.group(2).split(":", 1)[0], found.group(1)))
    if len(answers) < 500:
        print(f"ERROR: base.lst gave {len(answers)} answers: this reading is blind", file=sys.stderr)
        sys.exit(1)

    def name(layout, variant):
        return f"{layout}({variant})" if variant else layout

    left_out = {l.split("\t")[0] for l in open(f"{every}/left-out.txt", encoding="utf-8") if l.strip()}
    maps = {f[:-4] for f in os.listdir(every) if f.endswith(".map")}
    for layout, variant in answers:
        n = name(layout, variant)
        symbols = os.path.exists(f"{xkb}/symbols/{layout}")
        if n in maps:
            if not symbols:
                wrong.append(f"{n} has a map and its layout has no symbols file")
        elif n in left_out:
            if symbols:
                wrong.append(f"{n} was left out, and {xkb}/symbols/{layout} exists")
        else:
            wrong.append(f"{n} is offered and has no map, and is not said to be left out")
    offered = {name(l, v) for l, v in answers}
    for n in sorted(maps - offered):
        wrong.append(f"{n}.map answers nothing base.lst offers")
    if len(maps) < 500:
        wrong.append(f"only {len(maps)} maps")
    if "custom" not in left_out:
        wrong.append("`custom`, which has no symbols file, is not among the answers left out")

    # EACH MAP IS ONE THE CONSOLE READS: its own parser, every file.
    for n in sorted(maps):
        read = subprocess.run(["${pkgs.kbd}/bin/loadkeys", "-b", "-u", f"{every}/{n}.map"],
                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        if read.returncode != 0:
            wrong.append(f"loadkeys cannot read {n}.map: {read.stderr.decode()[:200]}")

    # THE DEFAULT ANSWER: the machine's map of it is the installer's.
    default = "${keyboard.console}"
    if default not in maps:
        wrong.append(f"the default answer {default} has no map")
    elif open(default_map, "rb").read() != open(f"{every}/{default}.map", "rb").read():
        wrong.append(f"the machine's map of {default} is not the installer's")

    # WHAT MUST BE REFUSED, by the very program the builds run.
    for answer, because in (
        ("us(bogus)", "base.lst has no variant `bogus` of `us`"),
        ("custom", "cannot be made"),
        ("uk", "not a kbd keymap"),
        ("us(", "no keyboard answer"),
    ):
        out = os.path.join(os.environ.get("TMPDIR", "/tmp"), "refused.map")
        done = subprocess.run(["python3", "${../nix/scripts/console-keymaps.py}",
                               "--xkb", xkb, "--ckbcomp", "${pkgs.ckbcomp}/bin/ckbcomp",
                               "--loadkeys", "${pkgs.kbd}/bin/loadkeys", "--out", out,
                               "--one", answer], capture_output=True, text=True)
        if done.returncode == 0 or because not in done.stderr:
            wrong.append(f"`{answer}` was not refused saying {because!r}: {done.returncode} {done.stderr[:300]!r}")

    if wrong:
        print("ERROR: the console's maps are not the installer's answers:", file=sys.stderr)
        for w in wrong:
            print("  " + w, file=sys.stderr)
        sys.exit(1)
    print(f"OK: {len(maps)} maps for {len(answers)} answers, {len(left_out)} left out with no symbols file; the refusals refuse")
    END
  '';

  console-keymap-is-the-answers = mkCheck "console-keymap-is-the-answers" [ pkgs.python3 ] ''
    python3 - ${keymaps.every} ${pkgs.writeText "consoles.json" (builtins.toJSON machines)} <<'END'
    import json, sys
    every, facts = sys.argv[1], json.load(open(sys.argv[2]))
    wrong = []
    if len(facts) < 2:
        wrong.append("fewer than two machines: this proves nothing")
    for wanted, c in sorted(facts.items()):
        if c["answer"] != wanted:
            wrong.append(f"the machine answered {wanted} says its console is {c['answer']}")
            continue
        if open(c["keyMap"], "rb").read() != open(f"{every}/{wanted}.map", "rb").read():
            wrong.append(f"{wanted}: the console's map is not the installer's map of the answer")
        for where in ("system", "initrd"):
            said = [l for l in open(c[where]).read().splitlines() if l.startswith("KEYMAP=")]
            if said != [f"KEYMAP={c['keyMap']}"]:
                wrong.append(f"{wanted}: the {where}'s vconsole.conf says {said}, not its map")
        if not c["copied"]:
            wrong.append(f"{wanted}: the initrd does not carry the map (boot.initrd.systemd.storePaths)")
        if not c["early"]:
            wrong.append(f"{wanted}: the console is not set up in the initrd (console.earlySetup)")
    if wrong:
        print("ERROR: a machine's console does not type its answer:", file=sys.stderr)
        for w in wrong:
            print("  " + w, file=sys.stderr)
        sys.exit(1)
    print(f"OK: {', '.join(sorted(facts))}: each console, and each initrd, holds the installer's map of its answer")
    END
  '';

  console-keymap-vm = import ./console-keymap-vm.nix {
    inherit pkgs lib consoleModule;
    inherit ronoctua;
  };
}
