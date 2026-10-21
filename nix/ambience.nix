# The player's ambient sounds, from the approved originals to the loops it
# plays — the twin of `nix/broadcast.nix` for `assets/ambience.nix`.
#
#   <id>.flac    each loop: 48 kHz, 16 bits, going round with no seam
#   index.json   what the player reads (`RONOCTUA_AMBIENCE`): the ids, the
#                credits, how many at once, the route, and every measure the
#                build took of every seam
#
# FROM EACH ORIGINAL: the clean window, its last `fadeSeconds` laid over its
# first (equal power: rain is noise, and two noises crossed that way keep
# their level), then gain to `loudnessLufs` — loudnorm's two passes, LINEAR
# or refused, because compression would flatten the thunder the recording is
# for — and 16 bits with dither.
#
# FLAC AND NOT OPUS: an Ogg Opus stream forgets its pre-skip on every seek to
# its start, and plays ~6.5 ms of the decoder warming up at every seam.
#
# THE SEAM IS MEASURED, not trusted: each loop is played three times round by
# the PINNED mpv (the one libmpv the player links: `mpv-unwrapped` over
# `ffmpeg-full`, as `nix/rust.nix` builds it) through each of the three ways
# it can go round, into a file, and compared sample for sample with the loop
# laid end to end. The route `assets/ambience.nix` names has to be exact; the
# others are measured and written down beside it.
#
# `originals` is the Release's archive of them (the input `ambience-v1`), or
# a directory holding them — a local copy, for a sound still being approved.
{
  lib,
  stdenvNoCC,
  writeText,
  python3,
  ffmpeg-full,
  mpv-unwrapped,
  gnutar,
  zstd,
  originals,
  table ? import ../assets/ambience.nix,
}:

let
  mpv = mpv-unwrapped.override { ffmpeg = ffmpeg-full; };
  python = python3.withPackages (ps: [ ps.numpy ]);

  # WHAT A SOUND MAY BE, refused while evaluating — before anything is
  # fetched or built.
  licences = [
    "CC0-1.0"
    "public-domain"
  ];
  routes = [
    "loop-file"
    "window"
    "amovie"
  ];
  # A GENERATOR'S NAME, as a word: a source that makes sound from a formula
  # is not a recording. `business` holds `sine` and is not one.
  generator = "(.*[^a-z0-9])?(anoisesrc|aevalsrc|anullsrc|sine|lavfi)([^a-z0-9].*)?";
  kebab = "[a-z][a-z0-9]*(-[a-z0-9]+)*";
  refuse = id: why: throw "assets/ambience.nix: ${id}: ${why}";
  check =
    sound:
    let
      id = sound.id or "?";
      text = lib.toLower (builtins.toJSON sound);
    in
    if builtins.match kebab id == null then
      refuse id "the id is not kebab-case (it becomes the key `player-ambience-<id>`)"
    else if builtins.match kebab (sound.group or "") == null then
      refuse id "the group is not kebab-case"
    else if !(builtins.elem (sound.licence or "") licences) then
      refuse id "the licence `${sound.licence or ""}` is not CC0 or the public domain"
    else if builtins.match generator text != null then
      refuse id "a generator is named; only recordings are ambient sounds"
    else if builtins.match "[A-Za-z0-9._-]+" (sound.original or "") == null then
      refuse id "the original is a file name among the originals, and nothing else"
    else if builtins.match "[0-9a-f]{64}" (sound.sha256 or "") == null then
      refuse id "the sha256 of the approved original is missing"
    else if builtins.match "https://.+" (sound.page or "") == null then
      refuse id "the page the recording came from is missing"
    else if (sound.author or "") == "" then
      refuse id "the author is missing"
    else if
      !(
        sound.fromSeconds >= 0
        && sound.fadeSeconds > 0
        && sound.toSeconds - sound.fromSeconds >= 4 * sound.fadeSeconds
      )
    then
      refuse id "the window is not a window: it must hold the loop and its crossfade three times over"
    else if !(sound.loudnessLufs >= -40 && sound.loudnessLufs <= -14) then
      refuse id "the loudness target is outside -40 to -14 LUFS"
    else
      sound;
  ids = map (s: s.id) table.sounds;
  checked =
    if lib.length (lib.unique ids) != lib.length ids then
      throw "assets/ambience.nix: an id repeats"
    else if !(builtins.isInt table.atOnce && table.atOnce >= 1 && table.atOnce <= 16) then
      throw "assets/ambience.nix: atOnce is a count from 1 to 16 (each sound is an mpv of its own)"
    else if !(builtins.elem table.route routes) then
      throw "assets/ambience.nix: the route `${table.route}` is none of ${lib.concatStringsSep ", " routes}"
    else
      # EVERY SOUND CHECKED when the table is read at all, and not only the
      # ones something happens to look at.
      builtins.deepSeq (map check table.sounds) table;

  declared = writeText "ambience.json" (builtins.toJSON checked);

  script = writeText "ambience.py" ''
    import argparse, hashlib, json, os, shutil, subprocess, sys

    import numpy as np

    # THE ONE RATE every loop is written at: PipeWire's, so no sound is
    # resampled on its way out.
    RATE = 48000
    # HOW MANY TIMES a route plays the loop: three plays, two seams.
    PLAYS = 3
    # THE WAYS mpv can go round, each measured.
    ROUTES = ("loop-file", "window", "amovie")
    # 10 ms, the grain a hole in the sound is looked for at.
    GRAIN = RATE // 100


    def refuse(why):
        sys.exit("ambience: " + why)


    def run(args):
        done = subprocess.run(args, capture_output=True)
        if done.returncode:
            refuse(" ".join(args[:3]) + " failed:\n" + done.stderr.decode(errors="replace")[-4000:])
        return done.stdout


    def sha256_of(path):
        h = hashlib.sha256()
        with open(path, "rb") as f:
            for block in iter(lambda: f.read(1 << 20), b""):
                h.update(block)
        return h.hexdigest()


    def seconds_of(ffprobe, path):
        out = run([ffprobe, "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path])
        return float(out.decode().strip())


    def decode(ffmpeg, path):
        raw = run([ffmpeg, "-v", "error", "-nostdin", "-i", path, "-f", "f32le", "-ac", "2", "-ar", str(RATE), "-"])
        return np.frombuffer(raw, dtype="<f4").reshape(-1, 2)


    def build(ffmpeg, work, source, sound, top):
        """The loop: the clean window, its last `fade` seconds laid over its
        first, brought to its loudness by gain alone — FLAC at RATE."""
        start = round(sound["fromSeconds"] * RATE)
        end = round(sound["toSeconds"] * RATE)
        fade = round(sound["fadeSeconds"] * RATE)
        joined = os.path.join(work, sound["id"] + ".joined.wav")
        # y = x[fade:end] with its last `fade` crossed into x[:fade], which
        # runs on into y's first sample: the end leads into the start.
        graph = (
            f"[0:a]aresample={RATE}:resampler=soxr:precision=28,"
            "aformat=sample_fmts=dbl:channel_layouts=stereo,"
            f"atrim=start_sample={start}:end_sample={end},asetpts=N/SR/TB,asplit=2[a][b];"
            f"[a]atrim=start_sample={fade},asetpts=N/SR/TB[body];"
            f"[b]atrim=end_sample={fade},asetpts=N/SR/TB[head];"
            f"[body][head]acrossfade=ns={fade}:c1=qsin:c2=qsin[out]"
        )
        run([ffmpeg, "-hide_banner", "-nostdin", "-y", "-i", source, "-filter_complex", graph,
             "-map", "[out]", "-c:a", "pcm_f64le", joined])
        target = sound["loudnessLufs"]
        peak = top["truePeakDb"]
        first = os.path.join(work, sound["id"] + ".measured.json")
        run([ffmpeg, "-hide_banner", "-nostdin", "-i", joined, "-af",
             f"loudnorm=I={target}:TP={peak}:LRA=50:print_format=json:stats_file={first}",
             "-f", "null", "-"])
        m = json.load(open(first))
        second = os.path.join(work, sound["id"] + ".applied.json")
        flac = os.path.join(work, sound["id"] + ".flac")
        chain = (
            f"loudnorm=I={target}:TP={peak}:LRA=50:measured_I={m['input_i']}:"
            f"measured_TP={m['input_tp']}:measured_LRA={m['input_lra']}:"
            f"measured_thresh={m['input_thresh']}:offset={m['target_offset']}:"
            f"linear=true:print_format=json:stats_file={second},"
            f"aresample={RATE}:resampler=soxr:precision=28:osf=s16:dither_method=triangular_hp"
        )
        # BIT-EXACT: no encoder's name or version in the file, so the same
        # original always makes the same bytes.
        run([ffmpeg, "-hide_banner", "-nostdin", "-y", "-i", joined, "-af", chain, "-c:a", "flac",
             "-compression_level", "12", "-fflags", "+bitexact", "-flags:a", "+bitexact",
             "-map_metadata", "-1", flac])
        applied = json.load(open(second))
        # GAIN AND NOTHING ELSE: loudnorm falls back to compressing, without
        # failing, when the true peak would pass the ceiling.
        if applied["normalization_type"] != "linear":
            refuse(f"{sound['id']}: reaching {target} LUFS would take compression "
                   f"(true peak {m['input_tp']} dBTP at {m['input_i']} LUFS): choose a lower target")
        os.remove(joined)
        return flac, end - start - fade, {
            "target_lufs": target,
            "measured_lufs": float(m["input_i"]),
            "gain_db": round(target - float(m["input_i"]), 2),
            "true_peak_db": float(applied["output_tp"]),
        }


    def power(x):
        return (x.astype(np.float64) ** 2).mean(axis=1)


    def levels(p, width, step):
        """The level of every `width` samples, `step` apart, in dB."""
        c = np.concatenate([[0.0], np.cumsum(p)])
        starts = np.arange(0, len(p) - width + 1, step)
        return 10 * np.log10(np.maximum((c[starts + width] - c[starts]) / width, 1e-30))


    def silence(x):
        """The longest run of samples silent in every channel."""
        silent = np.all(x == 0, axis=1).astype(np.int8)
        if not silent.any():
            return 0
        edges = np.diff(np.concatenate([[0], silent, [0]]))
        return int((np.nonzero(edges == -1)[0] - np.nonzero(edges == 1)[0]).max())


    def quietest(loop):
        """The loop's own quietest 10 ms as it usually is: the 5th percentile
        of the quietest 10 ms of every half second."""
        p = power(loop)
        half = RATE // 2
        lows = [levels(p[a:a + half], GRAIN, GRAIN // 4).min() for a in range(0, len(p) - half, half)]
        return float(np.percentile(lows, 5))


    def steps(loop):
        """How far one second's level is from the next's inside the loop, as
        it usually is: the 95th percentile."""
        return float(np.percentile(np.abs(np.diff(levels(power(loop), RATE, RATE))), 95))


    def seam(played, at, base):
        """The seam at sample `at`: the dip below the loop's quietest, the
        longest silence, the step from the second before to the one after."""
        near = played[max(0, at - RATE // 4):at + RATE // 4]
        before = levels(power(played[max(0, at - RATE):at]), RATE, RATE)
        after = levels(power(played[at:at + RATE]), RATE, RATE)
        return {
            "dip_db": round(base - float(levels(power(near), GRAIN, GRAIN // 4).min()), 2),
            "silent_samples": silence(played[max(0, at - RATE):at + RATE]),
            "step_db": round(abs(float(before[0]) - float(after[0])), 2)
            if len(before) and len(after) else None,
        }


    def event(loop, fade):
        """The loudest 400 ms around the crossfade — its `fade` seconds and
        one either side — above the loop's usual 400 ms: a thunder there is a
        thunder at every seam."""
        lv = levels(power(loop), 2 * RATE // 5, RATE // 10)
        at = np.arange(len(lv)) * (RATE // 10)
        near = (at >= len(loop) - fade - RATE) | (at <= RATE)
        return round(float(lv[near].max() - np.median(lv)), 2)


    def found(played, head, near, reach):
        """Where the loop's first samples are in `played`, exactly, within
        `reach` of `near`."""
        lo = max(0, near - reach)
        hi = min(len(played) - len(head), near + reach)
        if hi <= lo:
            return None
        for p in np.nonzero(np.all(played[lo:hi] == head[0], axis=1))[0] + lo:
            if np.array_equal(played[p:p + len(head)], head):
                return int(p)
        return None


    def play(mpv, route, flac, n, out):
        """The loop played by the pinned mpv PLAYS times round, into a file:
        what the player hands the sound server, sample for sample."""
        common = [mpv, "--no-config", "--idle=no", "--vid=no", "--audio-display=no", "--terminal=no",
                  "--ao=pcm", "--ao-pcm-file=" + out, "--ao-pcm-waveheader=no", "--audio-format=float",
                  f"--audio-samplerate={RATE}", "--audio-channels=stereo", "--gapless-audio=yes"]
        if route == "loop-file":
            args = common + [f"--loop-file={PLAYS - 1}", flac]
        elif route == "window":
            args = common + ["--prefetch-playlist=yes"] + [flac] * PLAYS
        else:
            args = common + [f"--end={PLAYS * n / RATE:.6f}",
                             f"av://lavfi:amovie={flac}:loop=0:discontinuity=1"]
        subprocess.run(args, capture_output=True, timeout=900, check=False)
        if not os.path.exists(out):
            return np.zeros((0, 2), dtype="<f4")
        played = np.fromfile(out, dtype="<f4").reshape(-1, 2)
        os.remove(out)
        return played


    def measure(loop, played, base):
        n = len(loop)
        ideal = np.tile(loop, (PLAYS, 1))
        m = min(len(played), len(ideal))
        differ = np.nonzero(np.any(played[:m] != ideal[:m], axis=1))[0]
        seams = []
        for k in range(1, PLAYS):
            s = seam(played, k * n, base)
            at = found(played, loop[:RATE // 10], k * n, RATE // 2)
            s["shift_samples"] = None if at is None else at - k * n
            seams.append(s)
        return {
            "samples": int(len(played)),
            "extra_samples": int(len(played) - len(ideal)),
            "identical": bool(len(played) == len(ideal) and len(differ) == 0),
            "identical_until": None if len(differ) == 0 else int(differ[0]),
            "seams": seams,
        }


    def main():
        parser = argparse.ArgumentParser()
        for name in ("table", "originals", "ffmpeg", "ffprobe", "mpv", "out", "work"):
            parser.add_argument("--" + name, required=True)
        a = parser.parse_args()
        top = json.load(open(a.table))
        os.makedirs(a.out, exist_ok=True)
        os.makedirs(a.work, exist_ok=True)
        index = {
            "format": "ronoctua-ambience",
            "version": 1,
            "at_once": top["atOnce"],
            "route": top["route"],
            "sounds": [],
        }
        for sound in top["sounds"]:
            name = sound["id"]
            source = os.path.join(a.originals, sound["original"])
            if not os.path.isfile(source):
                refuse(f"{name}: {sound['original']} is not among the originals")
            got = sha256_of(source)
            if got != sound["sha256"]:
                refuse(f"{name}: {sound['original']} is not the recording that was approved "
                       f"(sha256 {got}, approved {sound['sha256']})")
            length = seconds_of(a.ffprobe, source)
            if sound["toSeconds"] > length:
                refuse(f"{name}: the window ends at {sound['toSeconds']} s, the recording at {length:.3f} s")
            flac, n, loudness = build(a.ffmpeg, a.work, source, sound, top)
            loop = decode(a.ffmpeg, flac)
            if len(loop) != n:
                refuse(f"{name}: the loop is {len(loop)} samples and should be {n}")
            base = quietest(loop)
            usual = steps(loop)
            own = seam(np.tile(loop, (2, 1)), n, base)
            own["event_db"] = event(loop, round(sound["fadeSeconds"] * RATE))
            # THE LOOP ITSELF, round once: no hole, no silence, no step the
            # recording does not make on its own, no event in the crossfade.
            if (own["dip_db"] > 3.0 or own["silent_samples"] > RATE // 1000
                    or own["step_db"] > usual + 1.0 or own["event_db"] > 3.0):
                refuse(f"{name}: the seam would be heard ({own}; a usual step is {usual:.2f} dB): "
                       "choose another window")
            routes = {}
            for route in ROUTES:
                played = play(a.mpv, route, flac, n, os.path.join(a.work, f"{name}.{route}.f32"))
                routes[route] = measure(loop, played, base)
            kept = routes[top["route"]]
            # THE ROUTE THE PLAYER TAKES plays the loop sample for sample
            # across two seams — or the build stops, and says which does.
            if not kept["identical"]:
                exact = [r for r, m in routes.items() if m["identical"]]
                refuse(f"{name}: the route {top['route']} does not play the loop as it is ({kept}); "
                       f"{', '.join(exact) or 'no route'} does")
            shutil.move(flac, os.path.join(a.out, name + ".flac"))
            index["sounds"].append({
                "id": name,
                "group": sound["group"],
                "file": name + ".flac",
                "samples": n,
                "rate": RATE,
                "author": sound["author"],
                "page": sound["page"],
                "licence": sound["licence"],
                "loudness": loudness,
                "seam": {"own": own, "usual_step_db": round(usual, 2), "routes": routes},
            })
            print(f"{name}: {n / RATE:.1f} s, {loudness}, its own seam {own}")
            for route, m in routes.items():
                print(f"  {route}: {m}")
        with open(os.path.join(a.out, "index.json"), "w") as f:
            json.dump(index, f, indent=2)
            f.write("\n")


    main()
  '';
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-ambience";
  version = "1";

  nativeBuildInputs = [
    python
    gnutar
    zstd
  ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    # mpv writes nothing of its own here, and looks for no home.
    export HOME=$TMPDIR
    if [ -d ${originals} ]; then
      from=${originals}
    else
      mkdir originals
      tar --zstd -xf ${originals} -C originals
      from=originals
    fi
    python3 ${script} \
      --table ${declared} \
      --originals "$from" \
      --ffmpeg ${ffmpeg-full}/bin/ffmpeg \
      --ffprobe ${ffmpeg-full}/bin/ffprobe \
      --mpv ${mpv}/bin/mpv \
      --out $out \
      --work work
    runHook postBuild
  '';

  dontInstall = true;

  passthru.table = checked;

  meta = {
    description = "The player's ambient sounds: approved recordings, looped without a seam";
    platforms = lib.platforms.linux;
  };
}
