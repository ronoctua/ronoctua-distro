# The player in a VM: every family of format, the pictures, a set of lanes on
# one clock, MPRIS, and the column that narrows the windows and not the footer
# — at each of its sizes, on the workspace it belongs to.
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

  # THE COLUMN'S WIDTHS COME FROM THE TOKENS, as the build bakes them: a cell
  # is `cellWidthEm` of the base size, a size is its cells and the margin on
  # each side (`chrome::FIELDS_LEAD`), and none passes `maxPct` of the screen
  # (`column::width_px`). Nailed-down pixels would measure a column that no
  # longer exists the day a token moves.
  space = import ../assets/tokens/space.nix;
  typography = import ../assets/tokens/typography.nix;
  columnTokens = {
    inherit (space.column)
      large
      medium
      small
      bar
      maxPct
      ;
    cell = typography.size.base * typography.cellWidthEm;
    margin = space.content.inset - space.files.gutter;
  };
  setenv = lib.concatStringsSep " " (lib.mapAttrsToList (n: v: "--setenv=${n}=${v}") tools);

  # THE MEDIA AND THE CLIENT, shared with the photo stage (`nix/shot.nix`):
  # one file per family of format, generated (`tests/player-media.nix`), and
  # the client that asks the socket and reads the memfd from outside
  # (`tests/player-client.py`).
  media = import ./player-media.nix { inherit pkgs; };
  client = ./player-client.py;

  # THREE PLAIN PICTURES, one colour each, for the two screens: what each
  # screen shows is told apart by its colours alone.
  colours =
    pkgs.runCommand "ronoctua-player-colours" { nativeBuildInputs = [ pkgs.ffmpeg-headless ]; }
      ''
        mkdir -p $out
        for c in red:0xff0000 green:0x00ff00 blue:0x0000ff; do
          ffmpeg -loglevel error -f lavfi \
            -i "color=c=''${c#*:}:size=320x180:rate=25:duration=120" \
            -c:v libx264 -pix_fmt yuv444p -qp 0 "$out/''${c%%:*}.mp4"
        done
      '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-player";

  nodes.machine = {
    # THE ADULT CATALOGUE'S LOCK, as the distro declares it
    # (`nix/modules/unlock.nix`): the one option it reads, as the player's
    # module declares it, and a person whose password the door asks —
    # `ronoctua-shell unlock-check` is that person's check.
    imports = [
      ../nix/modules/unlock.nix
      { options.ronoctua.player.enable = lib.mkEnableOption "the player"; }
    ];
    ronoctua.player.enable = true;
    users.users.alice = {
      isNormalUser = true;
      password = "right-horse";
    };
    virtualisation.memorySize = 3072;
    virtualisation.cores = 4;
    environment.systemPackages = [
      scroll
      ronoctua-shell
      pkgs.python3
      pkgs.dbus
      pkgs.playerctl
      # A PICTURE OF ONE OUTPUT at a time (`grim -o`), over the compositor's
      # own capture: the two outputs' X windows overlap on the Xvfb screen.
      pkgs.grim
    ]
    ++ xvfb.packages;
    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];
    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    import json
    import math
    import re
    import time
    ${xvfb.prelude}

    COLUMN = json.loads('${builtins.toJSON columnTokens}')

    def column_width(size, out_w):
        """`column::width_px`: the size's cells and both margins, rounded up,
        never past `maxPct` of the screen and never under the bar."""
        natural = lambda cells: math.ceil(round(COLUMN["cell"] * (cells + 2 * COLUMN["margin"]), 4))
        most = out_w * COLUMN["maxPct"] // 100
        return max(min(natural(COLUMN[size]), most), natural(COLUMN["bar"]))

    def player(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def plays(name):
        said = player({"ask": "put", "what": [f"${media}/{name}"], "replace": True})
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
            "--setenv=RUST_LOG=ronoctua_player=debug ${setenv} "
            # THE TEST'S OWN AMBIENT SOUNDS: two tones of three seconds
            # (`tests/player-media.nix`).
            "--setenv=RONOCTUA_AMBIENCE=${media}/ambience "
            "${player}/bin/ronoctua-player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q player.sock", timeout=30)

    with subtest("every family plays, and each through the decoder its table names"):
        film = plays("film.mp4")
        # A FILE ALONE IS ITS LANES: its picture (with its size) and its sound.
        assert [l["kind"] for l in film["lanes"]] == ["video", "audio"], film["lanes"]
        pictures = [l for l in film["lane_states"] if l["picture"]]
        assert pictures and pictures[0]["size"] == [640, 360], film["lane_states"]
        plays("tone.flac")
        for name in ["tune.sid", "song.mid", "stream.vag", "tune.sndh", "tune.rad", "tune.psg", "mdat.tune", "tune.mod", "tune.vgm"]:
            plays(name)

    with subtest("an XSPF is ONE playlist of the queue, named, and its first track plays"):
        b = plays("list.xspf")
        # THE OUTLINE, asked apart from the state: the queue's shape is a
        # message of its own, sent whole when it changes.
        q = player({"ask": "queue"})
        assert q["say"] == "queue" and q["revision"] == b["revision"], (q, b["revision"])
        assert len(q["entries"]) == 1, q
        node = q["entries"][0]
        assert node["kind"] == "playlist" and node["name"] == {"titled": "list"}, node
        names = [child["name"]["location"] for child in node["children"]]
        assert names == ["${media}/tone.flac", "${media}/tune.rad"], names
        assert b["current"] == node["children"][0]["id"], (b["current"], node)
        assert b["location"] == "${media}/tone.flac", b["location"]
        # WHAT PLAYS NEXT is the queue's answer, and the state says it.
        assert b["next"] == {"location": "${media}/tune.rad"}, b["next"]

    with subtest("an ask the player does not know is answered, never dropped"):
        said = player({"ask": "open", "items": ["${media}/tone.flac"], "replace": True})
        assert said["say"] == "refused" and said["trouble"] == "not-understood", said
        said = player({"ask": "jump", "id": 99999})
        assert said == {"say": "refused", "trouble": "no-such-entry", "id": 99999}, said

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
        subs = [t for t in s["sources"][0]["tracks"] if t["kind"] == "sub"]
        assert len(subs) == 1, s["sources"]
        # THE FILE'S SUBTITLES ARE A LANE of it, and choosing a track is
        # asked of that lane.
        lane = next(l for l in s["lanes"] if l["kind"] == "subtitle")
        said = player({"ask": "track", "lane": lane["id"], "track": subs[0]["id"]})
        assert said["say"] == "done", said
        machine.sleep(1)
        s = player({"ask": "look"})
        now = [t for t in s["sources"][0]["tracks"] if t["kind"] == "sub"]
        assert now and now[0]["selected"], now
        lane = next(l for l in s["lanes"] if l["kind"] == "subtitle")
        assert lane["enabled"] and lane["track"] == subs[0]["id"], lane

    with subtest("the film's own track and a file beside it are two lanes, each in its colour"):
        s = plays("words.mkv")
        own = next(l for l in s["lanes"] if l["kind"] == "subtitle")
        subs = [t for t in s["sources"][0]["tracks"] if t["kind"] == "sub"]
        # THE FILM'S OWN TRACK, taken: lane one, nearest the edge.
        said = player({"ask": "track", "lane": own["id"], "track": subs[0]["id"]})
        assert said["say"] == "done", said
        said = player({"ask": "add-lane", "source": "${media}/words.en.srt", "kind": "subtitle"})
        assert said["say"] == "done", said
        # READ AND MERGED OFF THE ENGINE'S LOCK: `preparing` until the words
        # of both are in one script, then both play.
        words = []
        for _ in range(30):
            s = player({"ask": "look"})
            words = [l for l in s["lane_states"] if l["lane"] in
                     [x["id"] for x in s["lanes"] if x["kind"] == "subtitle"]]
            if len(words) == 2 and all(w["status"]["status"] == "playing" for w in words):
                break
            machine.sleep(1)
        assert len(words) == 2 and all(w["status"]["status"] == "playing" for w in words), words
        # THE COLOURS THE SCRIPT WAS WRITTEN WITH, lane by lane — the tokens'
        # (`engine::subtitles`, whose own test says so) — as `&H00BBGGRR`.
        script = machine.succeed("cat /run/user/0/ronoctua-player-subs/*.ass")
        styles = dict(re.findall(r"^Style: (Lane\d),[^,]*,\d+,&H00([0-9A-F]{6}),", script, re.M))
        assert sorted(styles) == ["Lane1", "Lane2"], script
        rgb = {lane: bgr[4:6] + bgr[2:4] + bgr[0:2] for lane, bgr in styles.items()}
        assert rgb["Lane1"] != rgb["Lane2"], rgb
        # AND BOTH ARE IN THE FRAME the daemon hands over, each on its own
        # rows: lane one below lane two, both in the picture's lower half.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 python3 ${client} dump null /tmp/words.ppm 1280 720"
        )
        seen = json.loads(machine.succeed(
            f"python3 ${client} colours /tmp/words.ppm {rgb['Lane1']} {rgb['Lane2']}"
        ))
        print(seen)
        one, two = seen["found"][rgb["Lane1"]], seen["found"][rgb["Lane2"]]
        assert one["pixels"] > 100 and two["pixels"] > 100, seen
        assert seen["h"] // 2 < two["rows"][1] < one["rows"][0], (
            f"the lanes are not stacked in order at the foot: {seen}"
        )

    with subtest("six subtitle lanes over one picture, and a seventh is refused"):
        # FOUR MORE of the same file: six lanes over the film.
        for _ in range(4):
            said = player({"ask": "add-lane", "source": "${media}/words.en.srt", "kind": "subtitle"})
            assert said["say"] == "done", said
        said = player({"ask": "add-lane", "source": "${media}/words.en.srt", "kind": "subtitle"})
        assert said == {"say": "refused", "trouble": "too-many", "what": "subtitles", "most": 6}, said
        s = player({"ask": "look"})
        assert len([l for l in s["lanes"] if l["kind"] == "subtitle"]) == 6, s["lanes"]

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

    with subtest("a second picture is a lane of a set, held to the film's sound, and brings its own"):
        before = player({"ask": "queue"})["entries"][0]
        said = player({"ask": "add-lane", "source": "${media}/other.mp4", "kind": "video"})
        assert said["say"] == "done", said
        # THE SAME ENTRY, with its id: the file became a set of four lanes —
        # its picture, its sound, the other picture and THAT picture's own
        # sound, brought with it (no track was chosen) — and kept playing.
        after = player({"ask": "queue"})["entries"][0]
        assert after["id"] == before["id"] and after["kind"] == "set", (before, after)
        assert after["lanes"] == 4, after
        machine.sleep(6)
        # HELD, AND NOT ONE READING. A picture 40 ms or more out
        # (`sync::STILL_MS`) is nudged back at a few per cent of the speed,
        # and the position it reports moves a frame (33 ms) at a time, so a
        # single reading taken mid-nudge says 67 ms of a lane being brought
        # back: the same player binary read 33 on one VM and 67 on another
        # that differed only in what else was in its store (2026-10-20).
        # Held is: it comes back, and three readings in a row stay near.
        deadline = time.time() + 20
        near = 0
        while True:
            s = player({"ask": "look"})
            other = next(l for l in s["lane_states"] if l["lane"] == 3)
            print(f"lane 3 is {other['drift_ms']} ms from the film's sound")
            near = near + 1 if abs(other["drift_ms"]) < 60 else 0
            if near == 3 or time.time() > deadline:
                break
            machine.sleep(1)
        master = [l["lane"] for l in s["lane_states"] if l["master"]]
        assert other["status"]["status"] == "playing", other
        assert master == [2], f"the film's sound does not keep the time: {s['lane_states']}"
        assert near == 3, f"lane 3 is {other['drift_ms']} ms away, and did not stay near"
        # `other.mp4` HAS NO SOUND: its sound lane says so, once it is loaded,
        # in a sentence of its own — never a silence that looks like playing.
        sound = next(l for l in s["lane_states"] if l["lane"] == 4)
        assert sound["status"] == {
            "status": "refused", "trouble": "no-sound", "lane": 4
        }, sound
        # ITS OWN PICTURE, asked by its lane.
        f = json.loads(machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 python3 ${client} frame 3"
        ))
        assert f["lit"] > 0.3, f"lane 3's picture is dark: {f}"
        for lane in (4, 3):
            said = player({"ask": "lane", "lane": lane, "change": {"what": "remove"}})
            assert said["say"] == "done", said
        assert player({"ask": "queue"})["entries"][0]["lanes"] == 2

    with subtest("a queue opened from its file is the file's until it changes, and is saved again"):
        machine.succeed("mkdir -p /root/queues")
        machine.succeed(
            "cat > /root/queues/Sunday.rqueue <<'EOF'\n"
            '{"format":"ronoctua-queue","version":1,"name":"Sunday","entries":['
            '{"single":{"location":"${media}/tone.flac"}},'
            '{"single":{"location":"${media}/film.mp4"}}]}\n'
            "EOF"
        )
        said = player({"ask": "put", "what": ["/root/queues/Sunday.rqueue"], "replace": True})
        assert said["say"] == "done", said
        kept = player({"ask": "look"})["queue_file"]
        assert kept == {"kept": "same", "path": "/root/queues/Sunday.rqueue", "name": "Sunday"}, kept
        # AN ENTRY TAKEN OUT: not saved.
        leaf = player({"ask": "queue"})["entries"][1]["id"]
        assert player({"ask": "remove", "id": leaf})["say"] == "done"
        kept = player({"ask": "look"})["queue_file"]
        assert kept["kept"] == "changed" and kept["name"] == "Sunday", kept
        # SAVED AGAIN, into the same file: the file's once more.
        assert player({"ask": "save-queue-again"})["say"] == "done"
        assert player({"ask": "look"})["queue_file"]["kept"] == "same"
        text = machine.succeed("cat /root/queues/Sunday.rqueue")
        assert "film.mp4" not in text and "tone.flac" in text, text
        # KILLED AND STARTED AGAIN — no farewell, so what it wakes with is
        # what it kept on the disk as it went: still the file's, not "not
        # saved" (the queue read back fingerprints as it was written).
        machine.succeed("systemctl kill -s KILL player")
        machine.wait_until_succeeds(
            "systemctl show -p ActiveState --value player | grep -qvx active", timeout=15
        )
        machine.succeed("systemctl reset-failed player || true")
        machine.wait_until_fails("systemctl cat player", timeout=15)
        # THE DEAD DAEMON'S SOCKET, by its whole name: a KILL leaves it on the
        # disk, and the wait below would otherwise be answered by it — then
        # the first ask is refused by nobody listening.
        machine.succeed("rm -f /run/user/0/ronoctua-player.sock")
        machine.succeed(
            "systemd-run --unit=player --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/0/bus "
            "--setenv=RUST_LOG=ronoctua_player=debug ${setenv} "
            "--setenv=RONOCTUA_AMBIENCE=${media}/ambience "
            "${player}/bin/ronoctua-player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q player.sock", timeout=30)
        kept = player({"ask": "look"})["queue_file"]
        assert kept == {"kept": "same", "path": "/root/queues/Sunday.rqueue", "name": "Sunday"}, kept
        # THE FILE CHANGED ON THE DISK since: saving over it is asked, and
        # `force` is the person's answer.
        machine.succeed("echo >> /root/queues/Sunday.rqueue")
        said = player({"ask": "put", "what": ["${media}/tune.mod"]})
        assert said["say"] == "done", said
        said = player({"ask": "save-queue-again"})
        assert said == {
            "say": "refused", "trouble": "changed-on-disk", "path": "/root/queues/Sunday.rqueue"
        }, said
        assert player({"ask": "save-queue-again", "force": True})["say"] == "done"
        assert "tune.mod" in machine.succeed("cat /root/queues/Sunday.rqueue")
        # A QUEUE NEVER KEPT has nowhere to be saved again.
        assert player({"ask": "clear"})["say"] == "done"
        assert player({"ask": "look"})["queue_file"] == {"kept": "empty"}
        player({"ask": "put", "what": ["${media}/tone.flac"]})
        assert player({"ask": "look"})["queue_file"] == {"kept": "never"}
        said = player({"ask": "save-queue-again"})
        assert said == {"say": "refused", "trouble": "no-origin"}, said

    with subtest("put once: what is in the queue already is asked about, by the place it is"):
        machine.succeed("ln -sf ${media}/tone.flac /root/tone-link.flac")
        for spelled in ["${media}/tone.flac", "/root/tone-link.flac", "file://${media}/tone.flac"]:
            said = player({"ask": "put", "what": [spelled], "once": True})
            assert said["say"] == "refused" and said["trouble"] == "already-queued", (spelled, said)
        # AND WITHOUT `once`, it goes in again — the person said so.
        assert player({"ask": "put", "what": ["/root/tone-link.flac"]})["say"] == "done"
        q = player({"ask": "queue"})
        assert len(q["entries"]) == 2, q
        # PUT BY THE FILE IT NAMES, not by the link.
        assert q["entries"][1]["name"] == {"location": "${media}/tone.flac"}, q

    with subtest("a sound that starts two seconds in waits, and then keeps the time"):
        late = {"lanes": [
            {"id": 1, "kind": "video", "source": "${media}/film.mp4"},
            {"id": 2, "kind": "audio", "source": "${media}/tone.flac", "start_seconds": 2.0},
        ]}
        said = player({"ask": "put", "what": [{"set": late}], "replace": True})
        assert said["say"] == "done", said
        # THE SET'S CLOCK waits for its voices to load, so right after the put
        # it is still before the sound's start, however slow this machine is.
        s = player({"ask": "look"})
        sound = next(l for l in s["lane_states"] if l["lane"] == 2)
        assert s["position"] < 2 and sound["status"]["status"] == "pending", (s["position"], sound)
        assert not sound["master"], "a sound that has not begun keeps the time"
        for _ in range(30):
            machine.sleep(1)
            s = player({"ask": "look"})
            sound = next(l for l in s["lane_states"] if l["lane"] == 2)
            if sound["status"]["status"] == "playing":
                break
        print(f"at {s['position']} the late sound is {sound}")
        assert s["position"] >= 2 and sound["status"]["status"] == "playing", (s["position"], sound)
        assert sound["master"], s["lane_states"]

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
        player({"ask": "put", "what": ["${media}/tone.flac", "${media}/tune.rad"], "replace": True})
        machine.sleep(3)
        tracks = prop("org.mpris.MediaPlayer2.TrackList", "Tracks")
        assert len(tracks) == 2, tracks
        # A TRACK'S ID IS ITS ENTRY'S, the one the queue's outline names.
        ids = [e["id"] for e in player({"ask": "queue"})["entries"]]
        assert tracks == [f"/br/ronoctua/player/track/{i}" for i in ids], (tracks, ids)
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
        entries = player({"ask": "queue"})["entries"]
        assert len(entries) == 3 and entries[0]["name"]["location"].endswith("film.mp4"), entries
        # THE IDS FROM THE LIST ITSELF, never a place written here: the one
        # added first is named by its entry.
        tracks = prop("org.mpris.MediaPlayer2.TrackList", "Tracks")
        assert len(tracks) == 3 and tracks[0] == f"/br/ronoctua/player/track/{entries[0]['id']}", tracks
        # AND THE METADATA OF AN ITEM THAT IS NOT PLAYING names it.
        said = machine.succeed(
            f"{where} call {name} /org/mpris/MediaPlayer2 "
            f"org.mpris.MediaPlayer2.TrackList GetTracksMetadata ao 1 {tracks[0]}"
        )
        assert "film.mp4" in said, said
        # REMOVED BY ITS ID, and the list is one shorter — the others keep
        # the ids they had.
        machine.succeed(
            f"busctl {bus} call {name} /org/mpris/MediaPlayer2 "
            f"org.mpris.MediaPlayer2.TrackList RemoveTrack o {tracks[0]}"
        )
        machine.sleep(2)
        assert prop("org.mpris.MediaPlayer2.TrackList", "Tracks") == tracks[1:]

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
            "${ronoctua-shell}/bin/ronoctua-shell column player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q column-player.pid", timeout=30)
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

    # THE ROOM WINDOWS HAVE on a workspace is what a column carves: the
    # workspace's own rectangle, which the compositor computes from every
    # exclusive zone on its output. A window's width is a SHARE of it in this
    # compositor, so the room — and not a window — is what is compared with
    # the column's width.
    msg = f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg"

    def room(name):
        spaces = json.loads(machine.succeed(f"{msg} -t get_workspaces"))
        return next(w["rect"] for w in spaces if w["name"] == name)

    def record():
        """The slot's record: `<pid> <size> <workspace>`."""
        return machine.succeed("cat /run/user/0/ronoctua-column-player.pid").strip().split(" ", 2)

    def configured_of(unit):
        said = machine.succeed(
            f"journalctl -u {unit} --no-pager -o cat | grep 'configured' || true"
        )
        said = re.sub(r"\x1b\[[0-9;]*m", "", said)
        return [int(w) for w in re.findall(r"width=(\d+) height=\d+ door=Window", said)]

    def room_is(name, width, why):
        try:
            machine.wait_until_succeeds(
                f"{msg} -t get_workspaces | python3 -c "
                f"'import json,sys; w=[w for w in json.load(sys.stdin) if w[\"name\"]==\"{name}\"][0]; "
                f"sys.exit(0 if w[\"rect\"][\"width\"]=={width} else 1)'",
                timeout=30,
            )
        except Exception:
            raise AssertionError(f"{why}: workspace {name} has {room(name)}, not {width} wide")

    with subtest("the column stays on its workspace"):
        # ON WORKSPACE 1, medium: the room is the screen less the column.
        pid, size, where = record()
        assert (size, where) == ("medium", "1"), record()
        medium = column_width("medium", out_w)
        room_is("1", out_w - medium, "the column's zone")
        narrow = configured_of("window")[-1]
        # WORKSPACE 2 is shown, and no screen shows 1: the column is parked,
        # and a window opened there has the whole screen — and is TOLD so.
        machine.succeed(f"{msg} workspace 2")
        machine.wait_until_succeeds(
            "journalctl -u column -o cat | grep -q 'the column is parked'", timeout=30
        )
        room_is("2", out_w, "a parked column still carves its zone")
        machine.succeed(
            "systemd-run --unit=window2 ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            "${ronoctua-shell}/bin/ronoctua-shell player"
        )
        machine.wait_until_succeeds("journalctl -u window2 -o cat | grep -q configured", timeout=60)
        machine.sleep(2)
        wide = configured_of("window2")
        print(f"on workspace 2 the window was configured at {wide}, the one on 1 at {narrow}")
        assert wide and wide[-1] > narrow, (
            f"the window on workspace 2 was not given the column's room: {wide} vs {narrow}"
        )
        # BACK ON WORKSPACE 1: the SAME column is shown again, and the window
        # there is told the room it has.
        machine.succeed(f"{msg} workspace 1")
        # SHOWN AGAIN, said by the column — the room alone could be the one
        # workspace 1 kept from before, if nothing arranged it while hidden.
        machine.wait_until_succeeds(
            "journalctl -u column -o cat | grep -q 'the column is shown'", timeout=30
        )
        room_is("1", out_w - medium, "the column did not come back")
        machine.sleep(2)
        again = configured_of("window")
        print(f"back on workspace 1 the window was configured at {again}")
        assert again[-1] == narrow, f"the window was not told the column came back: {again}"
        assert record()[0] == pid, f"the column is another process: {pid} -> {record()}"
        machine.succeed(f"kill -0 {pid}")

    with subtest("a size is a width"):
        # EACH SIZE, from the command line (`--size`): the one before it
        # ends as the new one takes the workspace.
        for size in ["bar", "small", "large", "medium"]:
            machine.succeed(
                f"systemd-run --unit=column-{size} ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
                "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
                f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
                f"${ronoctua-shell}/bin/ronoctua-shell column player --size {size}"
            )
            want = column_width(size, out_w)
            print(f"{size}: {want} px on a screen {out_w} wide")
            room_is("1", out_w - want, f"the {size} column")
            assert record()[1] == size, record()
        # THE CLAMP IS SAID IN THE TOKENS, and the VM's screen is where it
        # bites: on a screen this narrow even medium reaches the cap, so large
        # may be no wider than medium — never wider than the cap, never
        # narrower than medium.
        medium, large = column_width("medium", out_w), column_width("large", out_w)
        cap = out_w * COLUMN["maxPct"] // 100
        assert medium <= large <= cap, (medium, large, cap)

    with subtest("opening the player elsewhere moves it"):
        old = record()[0]
        machine.succeed(f"{msg} workspace 2")
        machine.succeed(
            "systemd-run --unit=column-moved ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "${ronoctua-shell}/bin/ronoctua-shell column player --on 2"
        )
        machine.wait_until_succeeds(
            "test \"$(cut -d' ' -f3- /run/user/0/ronoctua-column-player.pid)\" = 2", timeout=30
        )
        # ONE PLAYER'S COLUMN: the one on workspace 1 is gone, not copied.
        machine.wait_until_fails(f"kill -0 {old}", timeout=30)
        room_is("2", out_w - column_width("medium", out_w), "the moved column")
        # FROM HERE ON, what the window on workspace 1 is told: the configures
        # before the switch belong to the sizes above.
        told = len(configured_of("window"))
        machine.succeed(f"{msg} workspace 1")
        room_is("1", out_w, "workspace 1 kept the column that moved")
        # AND THE WINDOW ALREADY OPEN THERE IS TOLD, once the column that took
        # its room parks. The room above is what the compositor COMPUTED; the
        # switch lays workspace 1 out while the column on 2 still carves its
        # zone, and only the park — a layer surface whose role is destroyed —
        # gives the width back. A window never told stays at `narrow` until
        # the next click anywhere, the defect `settle_the_windows` exists for.
        machine.wait_until_succeeds(
            "journalctl -u column-moved -o cat | grep -q 'the column is parked'", timeout=30
        )
        widened = []
        for _ in range(30):
            widened = configured_of("window")[told:]
            if widened and widened[-1] > narrow:
                break
            machine.sleep(1)
        print(f"after the park the window on workspace 1 was configured at {widened}, not {narrow}")
        assert widened and widened[-1] > narrow, (
            f"the window on workspace 1 was not told the room the parked column gave back: "
            f"{widened} after {narrow}"
        )
        machine.succeed(
            "systemctl stop column-moved window window2; "
            "systemctl stop column column-bar column-small column-large column-medium || true"
        )

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
        space = next(
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
            want = (w, h, space["x"] + space["width"] - r["width"] - air,
                    space["y"] + space["height"] - r["height"] - title - air)
            if floating and box == want:
                break
            machine.sleep(1)
        print(f"room {space}, the small window {box}, floating {floating}, wanted {want}")
        assert floating, "the small window is in the layout, not floating"
        assert box == want, f"the small window is at {box}, not {want}"
        assert node.get("sticky"), "the small window does not follow the workspaces"
        machine.succeed("systemctl stop pip window compositor")

    with subtest("an ambient sound goes round past its own length, and only its own asks reach it"):
        bus = "--address=unix:path=/run/user/0/bus"
        name = "org.mpris.MediaPlayer2.ronoctua"
        a = player({"ask": "look"})["ambience"]
        assert [x["id"] for x in a["sounds"]] == ["small-waterfall", "thunderstorm"], a
        assert not any(x["on"] for x in a["sounds"]), f"a sound is on by itself: {a}"
        said = player({"ask": "ambient", "id": "small-waterfall", "on": True})
        assert said["say"] == "done", said
        # ONE AT A TIME in this catalogue, and a sound nobody declared: both
        # refused, each with its reason.
        said = player({"ask": "ambient", "id": "thunderstorm", "on": True})
        assert (said.get("trouble"), said.get("what"), said.get("most")) == ("too-many", "ambience", 1), said
        said = player({"ask": "ambient", "id": "wind", "on": True})
        assert said.get("trouble") == "no-such-sound", said
        # MORE THAN TWICE ITS THREE SECONDS: it went round, and plays on.
        machine.sleep(7)
        tone = player({"ask": "look"})["ambience"]["sounds"][0]
        assert tone["on"] and tone["rounds"] >= 2, f"the loop did not go round: {tone}"
        # WHAT THE QUEUE IS TOLD never reaches it: a stop, and a PlayPause over
        # MPRIS — the media keys.
        player({"ask": "stop"})
        machine.succeed(
            f"busctl {bus} call {name} /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player PlayPause"
        )
        machine.sleep(4)
        a = player({"ask": "look"})["ambience"]
        assert a["sounds"][0]["on"] and not a["paused"], a
        assert a["sounds"][0]["rounds"] > tone["rounds"], f"it stopped going round: {a}"
        # ITS OWN TOGGLE holds it: the rounds stop where they were.
        assert player({"ask": "ambient-toggle"})["say"] == "done"
        machine.sleep(1)
        held = player({"ask": "look"})["ambience"]
        machine.sleep(4)
        still = player({"ask": "look"})["ambience"]
        assert held["paused"] and still["sounds"][0]["rounds"] == held["sounds"][0]["rounds"], (held, still)
        # SILENCE turns every sound off, and each keeps its volume.
        player({"ask": "ambient-volume", "id": "small-waterfall", "percent": 40})
        assert player({"ask": "ambient-silence"})["say"] == "done"
        a = player({"ask": "look"})["ambience"]
        assert not any(x["on"] for x in a["sounds"]), a
        assert a["sounds"][0]["volume_percent"] == 40 and not a["paused"], a
        # AND ONE ON AGAIN, which closing the player has to end.
        assert player({"ask": "ambient", "id": "thunderstorm", "on": True})["say"] == "done"

    with subtest("a title written into a FLAC: the stamp moves, it reads back, and no draft is left"):
        # A COPY THE TEST OWNS: the store's is read-only, and a write goes
        # into a draft beside the file that is renamed over it.
        machine.succeed(
            "rm -rf /root/tags && mkdir -p /root/tags "
            "&& cp ${media}/tone.flac /root/tags/ && chmod u+w /root/tags/tone.flac"
        )
        flac = "/root/tags/tone.flac"

        def field_of(sheet, name):
            return next(f for f in sheet["fields"] if f["field"] == name)

        def written():
            for _ in range(150):
                r = player({"ask": "look"})["retag"]
                if r and r["location"] == flac and r["status"]["status"] != "writing" and r["waiting"] == 0:
                    return r
                time.sleep(0.2)
            raise AssertionError(f"the write never ended: {r}")

        sheet = player({"ask": "sheet", "location": flac})
        assert sheet["say"] == "sheet", sheet
        # EVERY READER THE UNIT NAMES answered: lofty, ffprobe, exiftool.
        assert sheet["unread"] == [], sheet["unread"]
        title = field_of(sheet, "title")
        assert title["value"] == "Tone" and title["can"] == {"yes": "lofty"}, title
        said = player({
            "ask": "retag", "location": flac, "stamp": sheet["stamp"],
            "edits": [{"edit": "set", "field": "title", "value": "Written here"}],
        })
        assert said["say"] == "done", said
        r = written()
        assert r["status"] == {"status": "written"}, r
        again = player({"ask": "sheet", "location": flac})
        assert again["stamp"] != sheet["stamp"], (sheet["stamp"], again["stamp"])
        assert field_of(again, "title")["value"] == "Written here", again["fields"]
        assert field_of(again, "artist")["value"] == "Nobody", again["fields"]
        # NO DRAFT LEFT BESIDE IT (`.tone.flac.ronoctua-<pid>.part`), and the
        # sound decodes whole.
        left = machine.succeed("ls -A /root/tags").split()
        assert left == ["tone.flac"], left
        machine.succeed(f"${tools.RONOCTUA_FFMPEG} -v error -xerror -i {flac} -f null -")
        # THE STAMP IT WAS READ WITH IS GONE: a write that names it is refused,
        # and still nothing is left behind.
        player({
            "ask": "retag", "location": flac, "stamp": sheet["stamp"],
            "edits": [{"edit": "set", "field": "title", "value": "Stale"}],
        })
        r = written()
        assert r["status"].get("trouble") == "tags" and r["status"].get("why") == {"why": "changed-since"}, r
        assert machine.succeed("ls -A /root/tags").split() == ["tone.flac"]
        assert field_of(player({"ask": "sheet", "location": flac}), "title")["value"] == "Written here"

    with subtest("nothing is looked up until somebody says so, and a host out of reach is said"):
        song = "${media}/tone.flac"
        lyrics = {"ask": "find", "query": {"what": "lyrics", "location": song}}
        # NOTHING LET: refused at once, naming the host — a search and a key.
        said = player(lyrics)
        assert said == {"say": "refused", "trouble": "off", "host": "lrclib.net"}, said
        said = player({"ask": "key-check", "service": "tmdb"})
        assert said == {"say": "refused", "trouble": "off", "host": "api.themoviedb.org"}, said
        # THE PRESS ON A ROW THAT NAMES THE HOST is the consent: the door
        # lets its hosts in the distro's consent file (`ronoctua-consent`),
        # then asks — the daemon reads it at the moment of asking, and the
        # lookup's pass is minted from it. TMDB is its API AND its images.
        machine.succeed(
            "mkdir -p /root/.config/ronoctua && "
            "echo '{\"hosts\":{\"lrclib.net\":true,\"api.themoviedb.org\":true,"
            "\"image.tmdb.org\":true}}' > /root/.config/ronoctua/consent.json"
        )
        said = player({"ask": "key-check", "service": "tmdb"})
        assert said == {"say": "refused", "trouble": "lookup", "service": "tmdb", "why": "no-key"}, said
        assert player(lyrics)["say"] == "done"
        # THE VM HAS NO NETWORK: the search ends, and its desk says why.
        for _ in range(150):
            desk = next(d for d in player({"ask": "look"})["desks"] if d["kind"] == "lyrics")
            if not desk["busy"]:
                break
            time.sleep(0.2)
        assert desk["trouble"] == {"trouble": "lookup", "service": "lrclib", "why": "unreachable"}, desk
        page = player({"ask": "desk", "kind": "lyrics"})
        assert page["say"] == "desk" and page["generation"] == desk["generation"], page
        assert page["page"] == {"page": "nothing"} and page["query"]["location"] == song, page
        # A CANDIDATE NO PAGE HAS is refused, never carried out.
        said = player({"ask": "take", "desk": "lyrics", "candidate": 99})
        assert said == {"say": "refused", "trouble": "no-such-candidate", "candidate": 99}, said
        machine.succeed("rm /root/.config/ronoctua/consent.json")

    with subtest("nothing is asked of a directory until somebody says so, and one out of reach is said"):
        radio = {"ask": "find", "query": {"what": "radio", "list": "countries"}}
        # NOTHING LET: each directory refused at once, naming its host.
        for query, host in [
            (radio["query"], "radio-browser.info"),
            ({"what": "tv", "list": "countries"}, "iptv-org.github.io"),
        ]:
            said = player({"ask": "find", "query": query})
            assert said == {"say": "refused", "trouble": "off", "host": host}, said
        # THE THEMES' DESKS ARE RETIRED — a theme is the local catalogue's —
        # and their lists are no ask at all.
        for query in [
            {"what": "archive", "category": "films"},
            {"what": "lives", "category": "news"},
            {"what": "tv", "list": "category", "category": "news"},
        ]:
            said = player({"ask": "find", "query": query})
            assert said["say"] == "refused" and said["trouble"] == "not-understood", said
        # A STAR ON WHAT NO PAGE HAS is refused, and nothing is written.
        uuid = "7ada8a81-5ae1-418c-8f18-51d2f38d86a4"
        said = player({"ask": "favourite", "kind": "radio", "id": uuid, "on": True})
        assert said == {"say": "refused", "trouble": "no-such-pick", "kind": "radio", "id": uuid}, said
        machine.fail("test -e /root/.config/ronoctua/player-favourites.json")
        # THE PRESS ON "ASK radio-browser.info" is the consent: the door
        # lets the host in the consent file, then asks — and the VM has no
        # network.
        machine.succeed(
            "mkdir -p /root/.config/ronoctua && "
            "echo '{\"hosts\":{\"radio-browser.info\":true}}' > /root/.config/ronoctua/consent.json"
        )
        assert player(radio)["say"] == "done"
        for _ in range(150):
            desk = next(d for d in player({"ask": "look"})["desks"] if d["kind"] == "radio")
            if not desk["busy"]:
                break
            time.sleep(0.2)
        assert desk["trouble"] == {"trouble": "directory", "source": "radio", "why": "unreachable"}, desk
        page = player({"ask": "desk", "kind": "radio"})
        assert page["say"] == "desk" and page["generation"] == desk["generation"], page
        assert page["page"] == {"page": "nothing"} and page["query"] == radio["query"], page
        # EVERY DESK IS IN THE STATE, the directories' after the lookups'.
        kinds = [d["kind"] for d in player({"ask": "look"})["desks"]]
        assert kinds == ["suggestions", "subtitles", "lyrics", "radio", "television"], kinds
        machine.succeed("rm /root/.config/ronoctua/consent.json")

    with subtest("a pasted http list is read into the catalogue when asked, kept 0600, said plain, and its account never said"):
        CATALOG = "/root/.cache/ronoctua/player-catalog"
        BOOK = "/root/.config/ronoctua/player-sources/sources.json"
        # NOTHING IS READ ON A CLOCK: minutes of player and no catalogue yet.
        machine.fail(f"test -e {CATALOG}/catalog.sqlite")
        # A DEFAULT SOURCE WHOSE HOSTS ARE NOT LET is refused at once.
        said = player({"ask": "ingest", "source": "nasa"})
        assert said == {"say": "refused", "trouble": "source", "why": "not-let"}, said
        # A PERSON'S LIST, on a plain-http host of the VM's own — an IPTV
        # provider's shape: the account in the query, a group of each kind,
        # and one the adult words send to the other file.
        machine.succeed(
            "mkdir -p /srv/iptv && printf '%s\\n' '#EXTM3U' "
            "'#EXTINF:-1 group-title=\"News\",Channel One' 'http://127.0.0.1:8099/one.m3u8' "
            "'#EXTINF:-1 group-title=\"Films\",Film Two' 'http://127.0.0.1:8099/two.mp4' "
            "'#EXTINF:-1 group-title=\"Adult 18+\",Night' 'http://127.0.0.1:8099/three.mp4' "
            "> /srv/iptv/list.m3u"
        )
        machine.succeed(
            "systemd-run --unit=iptv ${pkgs.python3}/bin/python3 -m http.server 8099 "
            "--bind 127.0.0.1 --directory /srv/iptv"
        )
        machine.wait_for_open_port(8099)
        url = "http://127.0.0.1:8099/list.m3u?username=ana&password=hunter2"
        said = player({"ask": "add-source", "url": url})
        assert said == {"say": "done"}, said
        for _ in range(150):
            # A PERSON'S LIST IS `p<stamp>`; a default source's id is a word,
            # and `peertube` starts with a `p` too.
            mine = [s for s in player({"ask": "look"})["catalog"]["sources"] if re.fullmatch(r"p\d+", s["id"])]
            if mine and not mine[0]["busy"]:
                break
            time.sleep(0.2)
        assert len(mine) == 1, mine
        source = mine[0]
        assert source["trouble"] is None and source["items"] == 3, source
        # SAID PLAIN, and shown by its host alone.
        assert source["plain"] is True and source["shown"] == "127.0.0.1/…", source
        # THE BOOK is the daemon's, private from its first byte; the two
        # files hold the rows, the adult group apart.
        assert machine.succeed(f"stat -c %a {BOOK}").strip() == "600"
        assert machine.succeed(f"stat -c %a {CATALOG}/catalog-adult.sqlite").strip() == "600"
        # THE ACCOUNT never reaches the state nor the journal.
        assert "hunter2" not in json.dumps(player({"ask": "look"}))
        machine.fail("journalctl -u player --no-pager | grep -q hunter2")
        # READ AGAIN WHEN ASKED — what a door that showed it stale asks —
        # replaced whole: the same three.
        said = player({"ask": "ingest", "source": source["id"]})
        assert said == {"say": "done"}, said
        for _ in range(150):
            again = next(s for s in player({"ask": "look"})["catalog"]["sources"] if s["id"] == source["id"])
            if not again["busy"]:
                break
            time.sleep(0.2)
        assert again["items"] == 3 and again["trouble"] is None, again
        # FORGOTTEN: out of the book and out of the state.
        said = player({"ask": "remove-source", "id": source["id"]})
        assert said == {"say": "done"}, said
        assert "127.0.0.1" not in machine.succeed(f"cat {BOOK}")
        assert not [s for s in player({"ask": "look"})["catalog"]["sources"] if s["id"] == source["id"]]
        machine.succeed("systemctl stop iptv")

    with subtest("the adult catalogue's lock: the right password opens it; a wrong one, and no service, do not"):
        check = "su -l alice -c 'ronoctua-shell unlock-check'"
        machine.succeed("test -e /etc/pam.d/ronoctua-unlock")
        # THE RIGHT ONE, through pam_unix and its setuid helper, as the door
        # asks it: the session's own user.
        said = machine.succeed(f"printf '%s\\n' 'right-horse' | {check}").strip()
        assert said == "open", said
        # A WRONG ONE is wrong — after the service's delay (three seconds,
        # which Linux-PAM spreads around that), never at once.
        began = time.monotonic()
        code, said = machine.execute(f"printf '%s\\n' 'wrong-horse' | {check}")
        waited = time.monotonic() - began
        assert code != 0 and said.strip() == "wrong", (code, said)
        assert waited >= 1.5, f"a wrong password was answered in {waited:.1f} s"
        # AN EMPTY ONE is never let in.
        code, said = machine.execute(f"printf '\\n' | {check}")
        assert code != 0 and said.strip() == "wrong", (code, said)
        # NO SERVICE — a machine that never declared it — is said, and PAM
        # is never asked: every right password would read "wrong" under
        # `other`.
        machine.succeed("mv /etc/pam.d/ronoctua-unlock /etc/pam.d/ronoctua-unlock.away")
        code, said = machine.execute(f"printf '%s\\n' 'right-horse' | {check}")
        machine.succeed("mv /etc/pam.d/ronoctua-unlock.away /etc/pam.d/ronoctua-unlock")
        assert code != 0 and said.strip() == "no-service", (code, said)

    # `C-s` PRESSED IN A DOOR. What the socket says of the queue's file
    # (`queue_file`: never saved, the same, changed since, changed on the
    # disk) is the engine's subtests'; this one is the doors' part of it: the
    # key in a window saves the queue again — the mark the Queue's title draws
    # goes — and over a file changed on the disk the window asks first, with
    # the cursor born on "no", and writes over it only on a yes. Every
    # assertion is the player's answer over its socket, from outside.
    with subtest("C-s in a window saves the queue again, and asks before writing over a changed file"):

        def kept():
            return player({"ask": "look"})["queue_file"]["kept"]

        def settles(what, why, seconds=30):
            until = time.time() + seconds
            while time.time() < until:
                if kept() == what:
                    return
                time.sleep(1)
            print(machine.execute("journalctl -u queue-door --no-pager -o cat | tail -n 40")[1])
            raise AssertionError(f"{why}: the queue's file is {kept()!r}, not {what!r}")

        def stays(what, why, seconds=4):
            until = time.time() + seconds
            while time.time() < until:
                assert kept() == what, f"{why}: the queue's file is {kept()!r}, not {what!r}"
                time.sleep(1)

        def press(*keys):
            for k in keys:
                machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {k}")
                machine.sleep(1)

        said = player({"ask": "put", "what": ["${media}/tone.flac", "${media}/film.mp4"], "replace": True})
        assert said["say"] == "done", said
        said = player({"ask": "save-queue", "to": "/root/door.rqueue", "name": "door"})
        assert said["say"] == "done", said
        settles("same", "saved by name")

        # A COMPOSITOR OF ITS OWN and the window in it: the earlier one was
        # stopped with its sockets left on the disk, and a stale socket
        # answers "the compositor is up" at once.
        machine.succeed("rm -f /run/user/0/wayland-* /run/user/0/scroll-ipc.*.sock")
        start_compositor("compositor-queue", "${scroll}")
        sock = socket_of("compositor-queue")
        machine.succeed(
            "systemd-run --unit=queue-door ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "${ronoctua-shell}/bin/ronoctua-shell player"
        )
        await_windows(sock, 1, timeout=60)
        # With no WM on Xvfb nobody has input focus, and the key would be lost.
        focus_compositor()
        machine.sleep(2)

        # CHANGED SINCE ITS FILE — the mark appears — and `C-s` in the window
        # saves it again: the mark goes.
        first = player({"ask": "queue"})["entries"][0]["id"]
        assert player({"ask": "remove", "id": first})["say"] == "done"
        settles("changed", "an entry taken off")
        press("ctrl+s")
        settles("same", "C-s in the window")

        # THE FILE CHANGED ON THE DISK: `C-s` asks, "no" first — `RET` at once
        # answers no, and nothing is written over — then `C-s`, `↓`, `RET`
        # writes over it.
        said = player({"ask": "put", "what": ["${media}/tone.flac"], "once": False})
        assert said["say"] == "done", said
        settles("changed", "an entry added")
        machine.succeed("echo >> /root/door.rqueue")
        press("ctrl+s", "Return")
        stays("changed", "a file changed on the disk was written over on the first RET")
        press("ctrl+s", "Down", "Return")
        settles("same", "the yes of the window's question")

        machine.succeed("systemctl stop queue-door")
        stop_compositor("compositor-queue")

    # TWO SCREENS (`docs/PENDING.md`, the columns' "two screens of truth"):
    # "Show on every screen" (`M` in a door, `player-every-screen`) opens one
    # window per screen something is routed to — the door decides which
    # (`player_stage::everywhere`, from the screens it knows by name and the
    # routes of what plays) and starts `ronoctua-shell player maximized
    # --screen <connector>` for each — each whole-screen on ITS output and
    # showing only the pictures routed there. Three plain pictures: red to
    # the first screen, blue to the second, green to every one. The key is
    # pressed in a real door; the door then leaves (its windows are not its
    # to close), and each output is photographed by itself (`grim -o`).
    with subtest("every screen, a window: the door's M opens one whole on each output, with its own pictures"):
        start_compositor("compositor-two", "${scroll}", outputs=2)
        sock = socket_of("compositor-two")
        msg2 = f"SCROLLSOCK={sock} SWAYSOCK={sock} ${scroll}/bin/scrollmsg"
        outputs = json.loads(machine.succeed(f"{msg2} -t get_outputs"))
        names = sorted(o["name"] for o in outputs if o.get("active"))
        assert len(names) == 2, f"the compositor has {names}, not two screens"
        first, second = names
        said = player({"ask": "put", "what": ["${colours}/red.mp4"], "replace": True})
        assert said["say"] == "done", said
        # LOADED FIRST: a lane is added to what plays, and until mpv has the
        # file its answer is `nothing-playing` — the file's lanes are its
        # tracks, and there are none to read before it loads.
        for _ in range(30):
            if any(l["kind"] == "video" for l in player({"ask": "look"})["lanes"]):
                break
            machine.sleep(1)
        for name in ("green", "blue"):
            said = player({"ask": "add-lane", "source": f"${colours}/{name}.mp4", "kind": "video"})
            assert said["say"] == "done", said
        lanes = player({"ask": "look"})["lanes"]
        picture = {l["source"].rsplit("/", 1)[-1]: l["id"] for l in lanes if l["kind"] == "video"}
        assert set(picture) == {"red.mp4", "green.mp4", "blue.mp4"}, lanes
        for clip, screens in (("red.mp4", {"only": [first]}), ("blue.mp4", {"only": [second]}),
                              ("green.mp4", "every")):
            said = player({"ask": "lane", "lane": picture[clip],
                           "change": {"what": "screens", "screens": screens}})
            assert said["say"] == "done", (clip, said)

        # THE DOOR, and `M` pressed in it. `KillMode=process`: the windows it
        # starts are its children in its unit, and stopping the door must not
        # take them with it.
        machine.succeed(
            "systemd-run --unit=every-door -p KillMode=process ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 --setenv=XDG_RUNTIME_DIR=/run/user/0 --setenv=HOME=/root "
            f"--setenv=SCROLLSOCK={sock} --setenv=SWAYSOCK={sock} "
            "${ronoctua-shell}/bin/ronoctua-shell player"
        )
        await_windows(sock, 1, timeout=60)
        door = int(machine.succeed("systemctl show -p MainPID --value every-door").strip())
        # The door knows the screens by name once the compositor has told it.
        machine.sleep(3)
        window = focus_compositor(0)
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key --window {window} shift+m")
        await_windows(sock, 3, timeout=60)
        said = machine.succeed("journalctl -u every-door --no-pager -o cat | tail -n 20")
        print(said)
        machine.succeed("systemctl kill --kill-whom=main every-door")
        machine.wait_until_fails(f"kill -0 {door}", timeout=30)
        await_windows(sock, 2, timeout=30)
        machine.sleep(4)

        def placed():
            """Each window's output, box and pid, from the tree."""
            tree = json.loads(machine.succeed(f"{msg2} -t get_tree"))
            out = []
            def walk(node, output):
                if node.get("type") == "output":
                    output = node
                if node.get("pid"):
                    out.append((output["name"], node["rect"], output["rect"], node["pid"]))
                for kid in node.get("nodes", []) + node.get("floating_nodes", []):
                    walk(kid, output)
            walk(tree, None)
            return out

        where = placed()
        print(where)
        assert sorted(o for o, _, _, _ in where) == [first, second], f"not one window per screen: {where}"
        for output, box, whole, pid in where:
            assert (box["width"], box["height"]) == (whole["width"], whole["height"]), (
                f"the window on {output} is {box}, not its whole screen {whole}"
            )
            line = machine.succeed(f"tr '\\0' ' ' < /proc/{pid}/cmdline")
            assert f"player maximized --screen {output}" in line, (
                f"the window on {output} was not started for it by the door: {line!r}"
            )

        def seen(screen):
            machine.succeed(
                f"XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 grim -t ppm -o {screen} /tmp/{screen}.ppm"
            )
            found = json.loads(machine.succeed(
                f"python3 ${client} colours /tmp/{screen}.ppm ff0000 00ff00 0000ff"
            ))["found"]
            return {c: found[c]["pixels"] for c in found}

        on_first, on_second = seen(first), seen(second)
        print(f"{first}: {on_first}, {second}: {on_second}")
        # A PICTURE IS THERE when it covers a good part of a quarter of its
        # place, and not there when not one pixel of its colour is.
        enough = 2000
        assert on_first["ff0000"] > enough and on_first["00ff00"] > enough, f"{first} lacks red or green: {on_first}"
        assert on_first["0000ff"] == 0, f"blue, routed to {second}, shows on {first}: {on_first}"
        assert on_second["0000ff"] > enough and on_second["00ff00"] > enough, f"{second} lacks blue or green: {on_second}"
        assert on_second["ff0000"] == 0, f"red, routed to {first}, shows on {second}: {on_second}"

        for _, _, _, pid in where:
            machine.succeed(f"kill {pid}")
        machine.succeed("systemctl stop every-door || true")
        stop_compositor("compositor-two")

    with subtest("closed everywhere: the watcher hears farewell, and the player leaves with success"):
        # A DOOR'S WATCH, from outside, and a quit that does not wait for its
        # answer — as every door sends it (`client::tell_if_running`).
        heard = json.loads(machine.succeed("XDG_RUNTIME_DIR=/run/user/0 python3 ${client} quit"))
        print(heard)
        assert heard["last"] == "farewell", heard
        # THE SOCKET GOES FIRST, so nothing asks a player that is leaving.
        machine.wait_until_fails("ls /run/user/0 | grep -q player.sock", timeout=10)
        # AND THE PROCESS LEAVES WITH SUCCESS — by `_exit`, never through the C
        # library's exit handlers with libmpv's threads alive: a crash there
        # is a failed unit, and the installed one's `Restart=on-failure` would
        # bring the player back showing the queue it had just closed.
        machine.wait_until_succeeds(
            "systemctl show -p ActiveState --value player | grep -qx inactive", timeout=15
        )
        result = machine.succeed("systemctl show -p Result --value player").strip()
        assert result == "success", f"the player left with {result}"

    with subtest("nothing panicked"):
        journal = machine.succeed("journalctl -u player --no-pager")
        assert "panicked" not in journal, journal
  '';
}
