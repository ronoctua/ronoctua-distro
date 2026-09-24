# The player in a VM: every family of format, the pictures, two videos in
# step, MPRIS, and the column that narrows the windows and not the footer.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The crates prove their rules on their own: the decoder table, the WAV header
# a streamed decoder gets wrong, the ring, the sync rule, the layout. What none
# of them reaches is the chain on a machine that is not the owner's:
#
#   a file  ->  the decoder the table names (by PATH, from `player-tools.nix`)
#   ->  mpv  ->  the position moving  ->  the ring  ->  a reader that is not us
#
# EVERY ASSERTION IS FROM OUTSIDE. The player is asked over its socket by a
# client written here; the pictures are read from the memfd it hands over;
# MPRIS is asked by `busctl`; the column is measured in the compositor's tree.
#
# NO SOUND SERVER: the VM has no PipeWire, which is the case
# `audio-fallback-to-null` exists for — a file must still play.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  player,
  fonts,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  tools = import ../nix/player-tools.nix { inherit pkgs; };
  setenv = lib.concatStringsSep " " (lib.mapAttrsToList (n: v: "--setenv=${n}=${v}") tools);

  # THE ADPLUG SOURCE, for two tunes it carries as test data: an AdLib song
  # and a MIDI file. The same tarball `nix/adplug.nix` builds from.
  adplugSrc = pkgs.fetchurl {
    url = "https://github.com/adplug/adplug/releases/download/adplug-2.4/adplug-2.4.tar.bz2";
    hash = "sha256-3hhGO/fAy2OaMiitR+aet/eKWhl4AtMl86XtfhxW1X8=";
  };

  # ONE FILE PER FAMILY, made here: nobody's music, and every one of them
  # something the decoder it names has to actually decode.
  media =
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
        printf '1\n00:00:00,000 --> 00:00:30,000\nhello\n' > sub.srt
        ffmpeg -loglevel error -f lavfi -i testsrc2=size=320x180:rate=25:duration=30 \
          -f lavfi -i sine=frequency=550:duration=30 -i sub.srt \
          -map 0 -map 1 -map 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -c:s srt \
          -shortest $out/subbed.mkv
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
        # AN XSPF, which libmpv does not read: the daemon opens it, and its
        # two tracks — one relative, one written as a `file://` URL — are
        # what the queue must hold.
        open(out + "/list.xspf", "w").write(
            '<playlist version="1" xmlns="http://xspf.org/ns/0/"><trackList>'
            '<track><location>tone.flac</location></track>'
            f'<track><location>file://{out}/tune.rad</location></track>'
            "</trackList></playlist>"
        )
        PY
      '';

  client = pkgs.writeText "player-client.py" ''
    import glob, json, os, socket, sys, time

    def path():
        found = glob.glob("/run/user/0/*player.sock")
        if not found:
            sys.exit("no player socket")
        return found[0]

    def ask(what):
        s = socket.socket(socket.AF_UNIX)
        s.connect(path())
        s.sendall((json.dumps(what) + "\n").encode())
        line = s.makefile().readline()
        s.close()
        return json.loads(line)

    def frame(of, count=10):
        s = socket.socket(socket.AF_UNIX)
        # A PAUSED PICTURE SENDS ONE FRAME OR NONE: a deadline, so "none"
        # fails the test instead of hanging it.
        s.settimeout(10)
        s.connect(path())
        s.sendall((json.dumps({"ask": "preview", "width": 320, "height": 180, "of": of}) + "\n").encode())
        data, fds, _, _ = socket.recv_fds(s, 512, 1)
        head = json.loads(data.decode().strip())
        assert head["say"] == "preview", head
        w, h = head["width"], head["height"]
        fd = fds[0]
        slot = 0
        for _ in range(count):
            slot = s.recv(1)[0]
        size = 16 + w * h * 4
        px = os.pread(fd, w * h * 4, slot * size + 16)
        s.close()
        os.close(fd)
        n = len(px) // 4
        lit = sum(1 for i in range(0, n, 97) if px[4*i] + px[4*i+1] + px[4*i+2] > 60) / len(range(0, n, 97))
        opaque = sum(1 for i in range(0, n, 97) if px[4*i+3] == 255) / len(range(0, n, 97))
        return {"w": w, "h": h, "lit": lit, "opaque": opaque}

    def twice(of):
        # TWO VIEWS AT ONCE, the second opened while the first is held.
        views = []
        for _ in range(2):
            s = socket.socket(socket.AF_UNIX)
            s.settimeout(10)
            s.connect(path())
            s.sendall((json.dumps({"ask": "preview", "width": 320, "height": 180, "of": of}) + "\n").encode())
            data, fds, _, _ = socket.recv_fds(s, 512, 1)
            head = json.loads(data.decode().strip())
            views.append((s, fds[0], head["width"], head["height"]))
        frames = []
        for s, fd, w, h in views:
            slot = s.recv(1)[0]
            frames.append(os.pread(fd, w * h * 4, slot * (16 + w * h * 4) + 16))
        return "same" if frames[0] == frames[1] else "different"

    what = sys.argv[1]
    if what == "twice":
        print(twice(json.loads(sys.argv[2])))
    elif what == "frame":
        print(json.dumps(frame(json.loads(sys.argv[2]), *map(int, sys.argv[3:4]))))
    else:
        print(json.dumps(ask(json.loads(what))))
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-player";

  nodes.machine = {
    virtualisation.memorySize = 3072;
    virtualisation.cores = 4;
    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.python3
      pkgs.dbus
      pkgs.playerctl
    ]
    ++ xvfb.packages;
    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];
    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    import json
    import re
    ${xvfb.prelude}

    def player(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def plays(name):
        said = player({"ask": "open", "items": [f"${media}/{name}"], "replace": True})
        assert said["say"] == "done", f"{name}: {said}"
        machine.sleep(3)
        a = player({"ask": "look"})
        machine.sleep(2)
        b = player({"ask": "look"})
        assert not b["troubles"], f"{name}: {b['troubles']}"
        assert b["position"] > a["position"] + 0.5, (
            f"{name} does not move: {a['position']} -> {b['position']}"
        )
        return b

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the player starts, on a bus of its own"):
        machine.succeed("mkdir -p /run/user/0 && chmod 700 /run/user/0")
        machine.succeed(
            "dbus-daemon --session --address=unix:path=/run/user/0/bus --fork --nopidfile"
        )
        machine.succeed(
            "systemd-run --unit=player --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "--setenv=RUST_LOG=ronoctua_player_engine=debug ${setenv} "
            "${player}/bin/ronoctua-player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q player.sock", timeout=30)

    with subtest("every family plays, and each through the decoder its table names"):
        film = plays("film.mp4")
        assert film["video"] == [640, 360], film["video"]
        plays("tone.flac")
        for name in ["tune.sid", "song.mid", "stream.vag", "tune.sndh", "tune.rad", "tune.psg", "mdat.tune", "tune.mod", "tune.vgm"]:
            plays(name)

    with subtest("an XSPF is opened into its tracks, and the first one plays"):
        b = plays("list.xspf")
        names = [item["location"] for item in b["queue"]]
        assert names == ["${media}/tone.flac", "${media}/tune.rad"], names

    with subtest("a decoder's stream seeks, forward and back"):
        plays("tune.sid")
        player({"ask": "seek", "seconds": 40, "relative": False})
        machine.sleep(6)
        at = player({"ask": "look"})["position"]
        assert at > 39, f"a forward seek into a SID landed at {at}"
        player({"ask": "seek", "seconds": 5, "relative": False})
        machine.sleep(4)
        at = player({"ask": "look"})["position"]
        assert 4 < at < 20, f"a backward seek into a SID landed at {at}"

    with subtest("a picture arrives through the memfd, opaque"):
        plays("film.mp4")
        f = json.loads(machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 python3 ${client} frame null"
        ))
        print(f)
        assert f["lit"] > 0.3, f"the picture is dark: {f}"
        assert f["opaque"] > 0.99, f"the picture is see-through: {f}"

    with subtest("a Matroska's subtitle track is offered, and taken"):
        s = plays("subbed.mkv")
        subs = [t for t in s["tracks"] if t["kind"] == "sub"]
        assert len(subs) == 1, s["tracks"]
        player({"ask": "subtitle-track", "id": subs[0]["id"]})
        machine.sleep(1)
        now = [t for t in player({"ask": "look"})["tracks"] if t["kind"] == "sub"]
        assert now and now[0]["selected"], now

    with subtest("two views of one picture are one decode: the same bytes"):
        # PAUSED, so both views get the frame on screen and nothing moves in
        # between: a byte that differs is a second decode.
        plays("film.mp4")
        player({"ask": "pause"})
        machine.sleep(1)
        same = machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 python3 ${client} twice null"
        ).strip()
        assert same == "same", same
        player({"ask": "play"})

    with subtest("a view opened on a PAUSED picture gets the frame on screen"):
        # A paused video — and an mp3's cover — sends no new frame; a view
        # opened then stayed black until something changed. The owner found
        # it on a cover: black in the column until a click paused the song.
        player({"ask": "pause"})
        machine.sleep(1)
        f = json.loads(machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 python3 ${client} frame null 1"
        ))
        assert f["lit"] > 0.3, f"the paused picture never came: {f}"
        player({"ask": "play"})

    with subtest("a second video is held to the first one's sound"):
        said = player({"ask": "follow", "item": "${media}/other.mp4"})
        assert said["say"] == "done", said
        machine.sleep(6)
        s = player({"ask": "look"})
        drift = s["participants"][0]["drift_ms"]
        print(f"drift {drift} ms")
        assert abs(drift) < 60, f"the follower is {drift} ms away"
        f = json.loads(machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 python3 ${client} frame 0"
        ))
        assert f["lit"] > 0.3, f"the follower's picture is dark: {f}"
        player({"ask": "unfollow", "index": 0})

    with subtest("MPRIS answers, and a PlayPause over it is a pause"):
        bus = "--address=unix:path=/run/user/0/bus"
        name = "org.mpris.MediaPlayer2.ronoctua"
        status = machine.succeed(
            f"busctl {bus} get-property {name} /org/mpris/MediaPlayer2 "
            "org.mpris.MediaPlayer2.Player PlaybackStatus"
        )
        assert "Playing" in status, status
        machine.succeed(
            f"busctl {bus} call {name} /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player PlayPause"
        )
        machine.sleep(1)
        assert player({"ask": "look"})["playing"] is False
        player({"ask": "play"})

    with subtest("playerctl sees the player, and its order is obeyed"):
        bus = "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus"
        players = machine.succeed(f"{bus} playerctl -l")
        assert "ronoctua" in players, players
        machine.succeed(f"{bus} playerctl -p ronoctua pause")
        machine.sleep(1)
        assert player({"ask": "look"})["playing"] is False
        assert "Paused" in machine.succeed(f"{bus} playerctl -p ronoctua status")
        machine.succeed(f"{bus} playerctl -p ronoctua play")
        machine.sleep(1)
        assert player({"ask": "look"})["playing"] is True

    with subtest("MPRIS: the queue is the track list, and it can be edited"):
        bus = "--address=unix:path=/run/user/0/bus"
        name = "org.mpris.MediaPlayer2.ronoctua"
        where = f"busctl {bus} --json=short"
        def prop(iface, what):
            return json.loads(machine.succeed(
                f"{where} get-property {name} /org/mpris/MediaPlayer2 {iface} {what}"
            ))["data"]
        assert prop("org.mpris.MediaPlayer2", "HasTrackList") is True
        assert prop("org.mpris.MediaPlayer2", "CanQuit") is True
        # A WILDCARD IS NOT A TYPE: every entry is named.
        types = prop("org.mpris.MediaPlayer2", "SupportedMimeTypes")
        assert types and not any("*" in t for t in types), types
        player({"ask": "open", "items": ["${media}/tone.flac", "${media}/tune.rad"], "replace": True})
        machine.sleep(3)
        tracks = prop("org.mpris.MediaPlayer2.TrackList", "Tracks")
        assert len(tracks) == 2, tracks
        # ONE ITEM PLAYING AND ONE AFTER: there is a next, and no previous.
        assert prop("org.mpris.MediaPlayer2.Player", "CanGoNext") is True
        assert prop("org.mpris.MediaPlayer2.Player", "CanGoPrevious") is False
        # ADDED FIRST, by "no track".
        machine.succeed(
            f"busctl {bus} call {name} /org/mpris/MediaPlayer2 "
            "org.mpris.MediaPlayer2.TrackList AddTrack sob "
            "file://${media}/film.mp4 /org/mpris/MediaPlayer2/TrackList/NoTrack false"
        )
        machine.sleep(2)
        queue = [i["location"] for i in player({"ask": "look"})["queue"]]
        assert queue[0].endswith("film.mp4") and len(queue) == 3, queue
        # AND THE METADATA OF AN ITEM THAT IS NOT PLAYING names it.
        said = machine.succeed(
            f"{where} call {name} /org/mpris/MediaPlayer2 "
            "org.mpris.MediaPlayer2.TrackList GetTracksMetadata ao 1 "
            "/br/ronoctua/player/track/0"
        )
        assert "film.mp4" in said, said
        # REMOVED BY ITS ID, and the list is one shorter.
        machine.succeed(
            f"busctl {bus} call {name} /org/mpris/MediaPlayer2 "
            "org.mpris.MediaPlayer2.TrackList RemoveTrack o /br/ronoctua/player/track/0"
        )
        machine.sleep(2)
        assert len(prop("org.mpris.MediaPlayer2.TrackList", "Tracks")) == 2

    with subtest("the column narrows the windows, and only them"):
        start_xvfb()
        start_compositor("compositor", "${scroll}")
        sock = socket_of("compositor")
        machine.succeed(
            "systemd-run --unit=window ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            "${ronoctua-shell}/bin/ronoctua-shell player"
        )
        await_windows(sock, 1, timeout=60)
        machine.sleep(2)
        out_w, _, _, _ = compositor_geometry()
        before = window_box(sock, "br.ronoctua.player")
        def widths():
            said = machine.succeed(
                "journalctl -u window --no-pager -o cat | grep 'configured' || true"
            )
            # THE LOG IS COLOURED even into the journal: the escapes go first.
            said = re.sub(r"\x1b\[[0-9;]*m", "", said)
            return [int(w) for w in re.findall(r"width=(\d+) height=\d+ door=Window", said)]
        # PAUSED, and nothing touched: the owner's screen. A playing video
        # repaints the window every frame and hid the defect in this test —
        # it passed with a fix that did nothing.
        player({"ask": "pause"})
        machine.sleep(2)
        told_before = widths()
        # THE COMPOSITOR'S SOCKET TOO, as the footer hands it to the column it
        # raises in a session: the column tells the windows their new room
        # over IPC, and without it said "no compositor" and told nobody.
        machine.succeed(
            "systemd-run --unit=column ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "${ronoctua-shell}/bin/ronoctua-shell column"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q column.pid", timeout=30)
        machine.sleep(3)
        after = window_box(sock, "br.ronoctua.player")
        print(f"output {out_w}, window before {before}, after {after}")
        assert after[2] + after[0] < before[2] + before[0], (
            f"the window's right edge did not move left: {before} -> {after}"
        )
        # AND THE WINDOW WAS TOLD, with no input in between. The tree above is
        # what the compositor COMPUTED; the commit that mapped the column used
        # to re-tile without sending it, and the window kept its width under
        # the column until the next click anywhere — the owner's screenshot.
        # AGAINST THE WIDTH IT HAD WHEN THE COLUMN OPENED, and a configure
        # that came AFTER. Compared with the first width of all — the door's
        # own default, before the compositor tiled it — this passed on the
        # defect itself: `[760, 512, 512]` is a window that was never told.
        told = widths()
        after = told[len(told_before):]
        print(f"the window was configured at {told_before} then {after}")
        assert told_before and after and after[-1] < told_before[-1], (
            f"the window was never told its new width: {told_before} then {after}"
        )
        machine.succeed("systemctl stop column window")

    with subtest("the small window floats in the bottom-right corner of the room, over the rest"):
        sock = socket_of("compositor")
        env = (
            "${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock}"
        )
        # A WINDOW IN THE LAYOUT, for the small one to float over: floating
        # means something only beside a window that does not.
        machine.succeed(f"systemd-run --unit=window {env} ${ronoctua-shell}/bin/ronoctua-shell player")
        await_windows(sock, 1, timeout=60)
        machine.succeed(f"systemd-run --unit=pip {env} ${ronoctua-shell}/bin/ronoctua-shell player pip")
        await_windows(sock, 2, timeout=60)
        # THE SLOT, which is what lets the column's button close it again.
        machine.wait_until_succeeds("grep -q ' pip' /run/user/0/ronoctua-player-stage.pid", timeout=30)
        pid = int(machine.succeed("systemctl show -p MainPID --value pip").strip())
        msg = f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg"
        room = next(
            w["rect"] for w in json.loads(machine.succeed(f"{msg} -t get_workspaces"))
            if w["focused"]
        )
        def placed():
            tree = json.loads(machine.succeed(f"{msg} -t get_tree"))
            def look(node, floating):
                if node.get("pid") == pid:
                    return node, floating
                for kid in node.get("nodes", []):
                    found = look(kid, floating)
                    if found:
                        return found
                for kid in node.get("floating_nodes", []):
                    found = look(kid, True)
                    if found:
                        return found
                return None
            return look(tree, False)
        # TOKENS, read back from the numbers the build baked: 384 x 216, 16 of
        # air (`space.player.pip*`).
        w, h, air = 384, 216, 16
        box = want = None
        # THE CONTENT is what is asked (`window_rect`), and the CONTAINER is
        # what sits `air` from the corner: `rect` plus the title bar the tree
        # keeps in `deco_rect`. The test's compositor draws one, 19 pixels,
        # and the first form of this came out 19 short.
        for _ in range(30):
            node, floating = placed()
            r, c, d = node["rect"], node["window_rect"], node.get("deco_rect") or {}
            title = d.get("height", 0)
            box = (c["width"], c["height"], r["x"], r["y"] - title)
            want = (w, h, room["x"] + room["width"] - r["width"] - air,
                    room["y"] + room["height"] - r["height"] - title - air)
            if floating and box == want:
                break
            machine.sleep(1)
        print(f"room {room}, the small window {box}, floating {floating}, wanted {want}")
        assert floating, "the small window is in the layout, not floating"
        assert box == want, f"the small window is at {box}, not {want}"
        assert node.get("sticky"), "the small window does not follow the workspaces"
        machine.succeed("systemctl stop pip window compositor")

    with subtest("nothing panicked"):
        journal = machine.succeed("journalctl -u player --no-pager")
        assert "panicked" not in journal, journal
  '';
}
