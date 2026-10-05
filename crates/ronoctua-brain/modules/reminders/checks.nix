# The reminders' own checks, in the shape every module's `checks.nix` has: the
# arguments it asks for by name (`nix/brain-modules.nix`, `checkArguments`),
# and its checks, each named after its id.
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
  environment = vm.systemd.user.services.ronoctua-brain.environment;
in
{
  checks = {
    # THE CHIME IS PLAYED BY STORE PATH, AND IT IS A SOUND.
    #
    # The brain's unit is given `pw-play` and the chime as ENVIRONMENT
    # (`module.nix`), and `ronoctua_audio::chime` reads two variables when a
    # reminder rings. Four things have to agree, and when one of them moved the
    # alarm still rang on the line and was silent — the one failure nobody
    # reports, because nothing looks broken:
    #
    #   1. the unit names both variables, each a path in the store;
    #   2. `pw-play` is there and runs;
    #   3. the chime is a WAV of a second or two that is not silent and does
    #      not clip;
    #   4. the Rust reads exactly those two names, from the environment and
    #      not from the build.
    #
    # The unit is read as the machine gets it — the evaluated configuration.
    reminders-chime-is-played-by-path =
      let
        facts = pkgs.writeText "reminders-chime.json" (
          builtins.toJSON {
            names = lib.attrNames environment;
            player = environment.RONOCTUA_PW_PLAY or "";
            chime = environment.RONOCTUA_CHIME or "";
          }
        );
      in
      mkCheck "reminders-chime-is-played-by-path" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, os, pathlib, re, sys, wave
        exec(open("${pyPrelude}").read())

        # PARSED: the unit arrives as the evaluated configuration, JSON; the
        # chime as a WAV read by the standard library.
        # FILE-WIDE: in the audio crate's boundary, two constants and one
        # function's body (`body_of`), comments off and strings kept — the
        # needles are strings.
        facts = json.load(open("${facts}"))
        wrong = []

        pipewire = uncommented(pathlib.Path("${../../../ronoctua-audio/src/pipewire.rs}").read_text())
        floor(len(pipewire), 1000, "characters of pipewire.rs")
        names = {}
        for const in ("PW_PLAY_VARIABLE", "CHIME_VARIABLE"):
            m = re.search(r"pub const " + const + r': &str = "([A-Z_]+)";', pipewire)
            if not m:
                wrong.append(f"pipewire.rs no longer declares {const}")
            else:
                names[const] = m.group(1)
        chime = must(body_of(pipewire, "chime"), "the audio crate's `chime`")
        for const in ("PW_PLAY_VARIABLE", "CHIME_VARIABLE"):
            if f"std::env::var_os({const})" not in chime:
                wrong.append(f"`chime` does not read {const} from the environment when it rings")
        if "option_env!" in chime:
            wrong.append("`chime` reads a path compiled in: the brain's binary would carry it")

        for const, name in names.items():
            if name not in facts["names"]:
                wrong.append(f"the brain's unit is not given {name}, which `chime` reads")

        player = facts["player"]
        if not (player.startswith("/nix/store/") and player.endswith("/bin/pw-play")):
            wrong.append(f"RONOCTUA_PW_PLAY is not pw-play by store path: {player!r}")
        elif not os.access(player, os.X_OK):
            wrong.append(f"{player} is not a program")

        sound = facts["chime"]
        if not sound.startswith("/nix/store/"):
            wrong.append(f"RONOCTUA_CHIME is not a store path: {sound!r}")
        else:
            with wave.open(sound) as w:
                rate, frames, width = w.getframerate(), w.getnframes(), w.getsampwidth()
                data = w.readframes(frames)
            seconds = frames / rate
            if not 0.5 <= seconds <= 3.0:
                wrong.append(f"the chime lasts {seconds:.2f} s; a chime is a second or two")
            if width != 2:
                wrong.append(f"the chime has {width}-byte samples, not 16-bit")
            samples = [int.from_bytes(data[i:i + 2], "little", signed=True) for i in range(0, len(data), 2)]
            peak = max(abs(s) for s in samples) if samples else 0
            if peak < 3000:
                wrong.append(f"the chime is nearly silent (peak {peak})")
            if peak >= 32767:
                wrong.append("the chime clips")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the brain plays a {seconds:.2f} s chime with pw-play, both by store path, under the names `chime` reads")
        END
      '';
  };
  slow = [ ];
}
