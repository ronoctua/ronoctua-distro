# THE PLAYER'S MEDIA: one file per family of format, made here and shared.
#
# `tests/player-vm.nix` plays every one of them through the decoder its table
# names, and `nix/shot.nix` hands them to a scene (`MEDIA`), so a photograph
# of the player shows the same film the test proves plays. One derivation, so
# the two cannot drift apart: a file changed for one is changed for both.
#
# Nobody's music and nobody's film: every file is generated from a formula
# (`testsrc2`, `sine`, `mandelbrot`), written byte by byte below, or is test
# data the decoder's own source carries.
{ pkgs }:

let
  # THE ADPLUG SOURCE, for two tunes it carries as test data: an AdLib song
  # and a MIDI file. The same tarball `nix/adplug.nix` builds from.
  adplugSrc = pkgs.fetchurl {
    url = "https://github.com/adplug/adplug/releases/download/adplug-2.4/adplug-2.4.tar.bz2";
    hash = "sha256-3hhGO/fAy2OaMiitR+aet/eKWhl4AtMl86XtfhxW1X8=";
  };
in
# ONE FILE PER FAMILY, made here: nobody's music, and every one of them
# something the decoder it names has to actually decode.
pkgs.runCommand "player-media"
  {
    nativeBuildInputs = [
      pkgs.ffmpeg-full
      pkgs.python3
    ];
  }
  ''
    mkdir -p $out
    ffmpeg -loglevel error -f lavfi -i testsrc2=size=640x360:rate=30:duration=30 \
      -f lavfi -i sine=frequency=440:duration=30 -c:v libx264 -pix_fmt yuv420p -c:a aac \
      -shortest $out/film.mp4
    ffmpeg -loglevel error -f lavfi -i "mandelbrot=size=320x180:rate=30" -t 30 \
      -c:v libx264 -pix_fmt yuv420p $out/other.mp4
    ffmpeg -loglevel error -f lavfi -i sine=frequency=330:duration=30 \
      -metadata artist=Nobody -metadata title=Tone $out/tone.flac
    # A MATROSKA WITH A SUBTITLE TRACK, which is what the subtitle ring is
    # for: a line of text over thirty seconds.
    #
    # NO `-shortest` HERE, although every input is thirty seconds anyway: with
    # it ffmpeg holds the subtitle's one packet back and writes it two thirds
    # into the file (byte 776 462 of 1 147 591, measured), beside the video of
    # about twenty seconds in, so a player that reads ahead a second at a time
    # meets a line timed at zero twenty seconds late. The track was offered
    # and taken, and "hello" was on no frame at three, six or thirteen
    # seconds — the photos of the daemon's own frame showed it. Without the
    # flag the packet is at byte 8 967 and libass draws it from the start
    # (measured with the pinned mpv, `--vo=image`).
    printf '1\n00:00:00,000 --> 00:00:30,000\nhello\n' > sub.srt
    ffmpeg -loglevel error -f lavfi -i testsrc2=size=320x180:rate=25:duration=30 \
      -f lavfi -i sine=frequency=550:duration=30 -i sub.srt \
      -map 0 -map 1 -map 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -c:s srt \
      $out/subbed.mkv
    # A PLAIN DARK FILM WITH ITS OWN SUBTITLE TRACK, and a subtitle file
    # beside it named after it: the two lanes the merge stacks, each in its
    # colour. Dark and plain so a subtitle's colour found in a frame can only
    # be the subtitle's — `testsrc2` paints every colour there is. No sound: a
    # picture's position moves by itself.
    printf '1\n00:00:00,000 --> 00:00:30,000\nfrom inside\n' > inside.srt
    ffmpeg -loglevel error -f lavfi -i color=c=0x303030:size=640x360:rate=25:duration=30 \
      -i inside.srt -map 0 -map 1 -c:v libx264 -pix_fmt yuv420p -c:s srt $out/words.mkv
    printf '1\n00:00:00,000 --> 00:00:30,000\nbeside it\n' > $out/words.en.srt
    # THE AMBIENT SOUNDS' CATALOGUE IN MINIATURE, for the tests alone
    # (`RONOCTUA_AMBIENCE` is set to it by a test, never by a module): two
    # tones of three seconds under two of the real ids — so a photograph of
    # the page has names to show — going round in seconds, one at a time, so
    # the ceiling is seen too. Tones from a formula, which the real catalogue
    # refuses (`nix/ambience.nix`); the index is the one that build writes.
    mkdir $out/ambience
    ffmpeg -loglevel error -f lavfi -i sine=frequency=330:duration=3:sample_rate=48000 \
      -ac 2 $out/ambience/small-waterfall.flac
    ffmpeg -loglevel error -f lavfi -i sine=frequency=440:duration=3:sample_rate=48000 \
      -ac 2 $out/ambience/thunderstorm.flac
    cat > $out/ambience/index.json <<'JSON'
    {"format": "ronoctua-ambience", "version": 1, "at_once": 1, "route": "loop-file",
     "sounds": [
      {"id": "small-waterfall", "group": "water", "file": "small-waterfall.flac",
       "author": "nobody", "page": "https://example.invalid/tone-330", "licence": "CC0-1.0"},
      {"id": "thunderstorm", "group": "storm", "file": "thunderstorm.flac",
       "author": "nobody", "page": "https://example.invalid/tone-440", "licence": "public-domain"}]}
    JSON
    tar xjf ${adplugSrc} --wildcards '*/test/testmus/ALLOYRUN.RAD' '*/test/testmus/GRABBAG.MID'
    cp */test/testmus/ALLOYRUN.RAD $out/tune.rad
    cp */test/testmus/GRABBAG.MID $out/song.mid
    python3 - <<'PY'
    import math, os, struct
    out = os.environ["out"]
    # A PSID whose init and play are RTS: silence, decoded for real.
    h = b"PSID" + struct.pack(">HHHHHHHI", 2, 0x7C, 0x1000, 0x1000, 0x1003, 1, 1, 0)
    h += b"tone".ljust(32, b"\0") + b"nobody".ljust(32, b"\0") + b"2026".ljust(32, b"\0")
    h += struct.pack(">HBBBB", 0, 0, 0, 0, 0)
    open(out + "/tune.sid", "wb").write(h + bytes([0x60, 0, 0, 0x60]))
    # An SNDH whose three entries branch to one RTS.
    tags = b"SNDH" + b"TITLtone\0" + b"COMMnobody\0\0" + b"##01" + b"TC50" + b"HDNS"
    # THE CODE ON AN EVEN ADDRESS: a 68000 branch to an odd one is an
    # address error, and sc68 then plays nothing at all.
    if len(tags) % 2:
        tags += b"\0"
    end = 12 + len(tags)
    bra = lambda at: b"\x60\x00" + struct.pack(">h", end - (at + 2))
    open(out + "/tune.sndh", "wb").write(bra(0) + bra(4) + bra(8) + tags + b"\x4e\x75")
    # An Amiga module under the name the Amiga gave its music — the
    # format FIRST — which only uade claims, and which uade reads by content.
    mod = b"tune".ljust(20, b"\0")
    for _ in range(31):
        mod += bytes(22) + bytes([0, 0, 0, 64, 0, 0, 0, 1])
    mod += bytes([1, 127]) + bytes(128) + b"M.K." + bytes(1024)
    open(out + "/mdat.tune", "wb").write(mod)
    # AND THE SAME MODULE under the name everybody else gives it: that one
    # is libopenmpt's, through ffmpeg, and not uade's.
    open(out + "/tune.mod", "wb").write(mod)
    # A VGM made by hand: one PSG tone held for thirty seconds. libgme
    # reads it, and libmpv only when the demuxer is named — the score
    # ffmpeg gives it is one below what libmpv accepts.
    vgm = bytes([0x50, 0x8E, 0x50, 0x0F, 0x50, 0x90])
    for _ in range(30):
        vgm += bytes([0x61]) + struct.pack("<H", 44100)
    vgm += bytes([0x66])
    head = bytearray(0x40)
    head[0:4] = b"Vgm "
    struct.pack_into("<I", head, 0x04, 0x40 + len(vgm) - 4)
    struct.pack_into("<I", head, 0x08, 0x150)
    struct.pack_into("<I", head, 0x0C, 3579545)
    struct.pack_into("<I", head, 0x18, 44100 * 30)
    struct.pack_into("<I", head, 0x24, 60)
    struct.pack_into("<H", head, 0x28, 0x0009)
    head[0x2A] = 16
    struct.pack_into("<I", head, 0x34, 0x40 - 0x34)
    open(out + "/tune.vgm", "wb").write(bytes(head) + vgm)
    # A ZX Spectrum chip stream zxtune reads: a tone held for a minute.
    psg = b"PSG\x1a" + bytes(12) + bytes([7, 0x3E, 8, 15, 0, 100, 1, 0]) + bytes([0xFF]) * 3000
    open(out + "/tune.psg", "wb").write(psg)
    # A raw stream vgmstream reads through a TXTH beside it.
    pcm = b"".join(struct.pack("<h", int(8000 * math.sin(i / 20))) for i in range(44100 * 30))
    open(out + "/stream.vag", "wb").write(pcm)
    open(out + "/.vag.txth", "w").write(
        "codec = PCM16LE\nchannels = 1\nsample_rate = 44100\nnum_samples = data_size\n"
    )
    # AN XSPF, which libmpv does not read: the daemon opens it into ONE
    # playlist of the queue, named as the list names itself, and its two
    # tracks — one relative, one written as a `file://` URL — are that
    # playlist's children.
    open(out + "/list.xspf", "w").write(
        '<playlist version="1" xmlns="http://xspf.org/ns/0/"><title>list</title><trackList>'
        '<track><location>tone.flac</location></track>'
        f'<track><location>file://{out}/tune.rad</location></track>'
        "</trackList></playlist>"
    )
    PY
  ''
