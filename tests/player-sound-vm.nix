# The player's SOUND, in a VM that has a sound server: what `player-vm`
# cannot see, because it has none (`audio-fallback-to-null` plays there).
#
# WHAT ONLY THIS PROVES
# ---------------------
# That two films played together are both HEARD: the second picture lane,
# added with no track chosen, brings its own sound (`engine/session.rs`,
# `lanes_added`), and its voice opens a stream of its own on the sound
# server. The unit tests prove that every voice of such a set is dressed with
# a sound (`aid` never `no`); only a sound server can say the streams exist.
#
# THE SERVER IS PIPEWIRE, SYSTEM-WIDE, with two null sinks: no card, no
# session manager's policy to wait for, and a socket at `/run/pipewire` that
# the daemon — started by hand, as root — is pointed at. `pw-dump` is asked
# from outside, as every assertion here is.
#
# AND TWO OUTPUTS, for what only a server can say of them: a lane sent to the
# second one (`LaneChange::Sink`) is a stream the server routes THERE — read
# back from the stream's own properties, not from what the engine meant —
# and the engine reads the graph (`pw-dump --monitor`, a child of its own)
# only while a door holds the outputs open or a lane has one chosen
# (`engine/sinks.rs`): a player at rest reads nothing.
#
# AND THE SOUND VIEW'S MOVE, which only a server can prove too: a playing
# program sent to the other sink by the `target.object` `ronoctua-audio`
# writes (`pw-metadata`, with the arguments and environment `pipewire.rs`
# gives it) plays there by the server's own links, and back; and a program
# that ended is refused in the log alone, with status 0.
{
  pkgs,
  lib,
  player,
}:

let
  tools = import ../nix/player-tools.nix { inherit pkgs; };
  setenv = lib.concatStringsSep " " (lib.mapAttrsToList (n: v: "--setenv=${n}=${v}") tools);
  media = import ./player-media.nix { inherit pkgs; };
  client = ./player-client.py;
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-player-sound";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;
    environment.systemPackages = [
      pkgs.python3
      pkgs.pipewire
    ];
    services.pipewire = {
      enable = true;
      systemWide = true;
      alsa.enable = false;
      pulse.enable = false;
      wireplumber.enable = true;
      # ONE SINK THAT IS NO CARD: the streams have somewhere to go, driven
      # by the server's own clock.
      extraConfig.pipewire."10-null-sink" = {
        "context.objects" = [
          {
            factory = "adapter";
            args = {
              "factory.name" = "support.null-audio-sink";
              "node.name" = "ronoctua-test-sink";
              "media.class" = "Audio/Sink";
              "object.linger" = true;
              "audio.position" = [
                "FL"
                "FR"
              ];
              "priority.session" = 1009;
              "priority.driver" = 1009;
            };
          }
          # THE SECOND OUTPUT, below the first in priority: the default stays
          # the first, and a lane is sent here only by being asked.
          {
            factory = "adapter";
            args = {
              "factory.name" = "support.null-audio-sink";
              "node.name" = "ronoctua-test-sink-two";
              "media.class" = "Audio/Sink";
              "object.linger" = true;
              "audio.position" = [
                "FL"
                "FR"
              ];
              "priority.session" = 900;
              "priority.driver" = 900;
            };
          }
        ];
      };
    };
  };

  testScript = ''
    import json
    from datetime import timedelta

    def player(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def graph():
        """`pw-dump`'s snapshot of the sound graph, TAKEN AGAIN when it does
        not parse: it is a snapshot of a live graph, and one taken while a
        stream comes up has come back malformed (`Expecting ',' delimiter`,
        2026-11-06) — a fault of the snapshot, not of what the test asks it,
        and a second look a moment later answers. Five that never parse are
        the failure they are."""
        for _ in range(5):
            try:
                return json.loads(machine.succeed("PIPEWIRE_RUNTIME_DIR=/run/pipewire pw-dump"))
            except json.JSONDecodeError as refused:
                last = refused
                machine.sleep(duration=timedelta(seconds=1))
        raise last

    def output_streams():
        """The sound streams the server holds for the player, read from
        outside: `pw-dump`'s nodes of class `Stream/Output/Audio`."""
        dump = graph()
        return [
            o for o in dump
            if o.get("type") == "PipeWire:Interface:Node"
            and o.get("info", {}).get("props", {}).get("media.class") == "Stream/Output/Audio"
        ]

    machine.start()
    # THE SOCKET, and not the service: system-wide, PipeWire is started by
    # its socket, and nothing has connected yet — `pw-dump` below is what
    # starts it (and WirePlumber with it).
    machine.wait_for_unit("pipewire.socket")
    machine.wait_until_succeeds(
        "PIPEWIRE_RUNTIME_DIR=/run/pipewire pw-dump | grep -q ronoctua-test-sink", timeout=timedelta(minutes=1)
    )

    with subtest("the player starts, on the sound server"):
        machine.succeed("mkdir -p /run/user/0 && chmod 700 /run/user/0")
        machine.succeed(
            "systemd-run --unit=player --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=PIPEWIRE_RUNTIME_DIR=/run/pipewire "
            "--setenv=RUST_LOG=ronoctua_player=debug ${setenv} "
            "${player}/bin/ronoctua-player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q player.sock", timeout=timedelta(seconds=30))

    with subtest("two films played together are two sound streams"):
        said = player({"ask": "put", "what": ["${media}/film.mp4"], "replace": True})
        assert said["say"] == "done", said
        for _ in range(30):
            if output_streams():
                break
            machine.sleep(duration=timedelta(seconds=1))
        assert len(output_streams()) == 1, output_streams()
        # THE SECOND FILM, with no track chosen: its picture AND its sound.
        said = player({"ask": "add-lane", "source": "${media}/subbed.mkv", "kind": "video"})
        assert said["say"] == "done", said
        lanes = player({"ask": "look"})["lanes"]
        kinds = [(l["kind"], l["source"]) for l in lanes]
        assert ("audio", "${media}/subbed.mkv") in kinds, kinds
        # EXACTLY TWO, waited for: a set cast again may close a stream and
        # open it anew on the way.
        for _ in range(30):
            if len(output_streams()) == 2:
                break
            machine.sleep(duration=timedelta(seconds=1))
        streams = output_streams()
        print(json.dumps([s["info"]["props"] for s in streams], indent=1))
        assert len(streams) == 2, f"the second film is not heard: {len(streams)} stream(s)"
        s = player({"ask": "look"})
        for state in s["lane_states"]:
            assert state["status"]["status"] == "playing", s["lane_states"]

    def player_processes():
        """The command lines in the player's own unit, from outside."""
        group = machine.succeed("systemctl show -p ControlGroup --value player").strip()
        pids = machine.succeed(f"cat /sys/fs/cgroup{group}/cgroup.procs").split()
        lines = []
        for pid in pids:
            status, line = machine.execute(f"tr '\\0' ' ' < /proc/{pid}/cmdline")
            if status == 0:
                lines.append(line)
        return lines

    def reading_the_graph():
        return any("pw-dump" in l for l in player_processes())

    def wait_for(what, want, timeout=30):
        for _ in range(timeout):
            if what() == want:
                return
            machine.sleep(duration=timedelta(seconds=1))
        assert what() == want, f"{what.__name__} stayed {not want}: {player_processes()}"

    def stream_targets():
        """Where each of the player's sound streams goes: the sink a LINK
        from it reaches, by name — the server's routing, not the engine's
        intention."""
        dump = graph()
        names = {o["id"]: o.get("info", {}).get("props", {}).get("node.name") for o in dump
                 if o.get("type") == "PipeWire:Interface:Node"}
        streams = {s["id"] for s in output_streams()}
        reached = set()
        for o in dump:
            if o.get("type") != "PipeWire:Interface:Link":
                continue
            info = o.get("info", {})
            if info.get("output-node-id") in streams:
                reached.add(names.get(info.get("input-node-id")))
        return reached

    with subtest("at rest the player does not read the sound graph"):
        wait_for(reading_the_graph, False)

    with subtest("a door holding the outputs: the graph read, both sinks listed"):
        machine.succeed(
            "systemd-run --unit=holder --setenv=XDG_RUNTIME_DIR=/run/user/0 python3 -c "
            "'import glob, socket, time; s = socket.socket(socket.AF_UNIX); "
            "s.connect(glob.glob(\"/run/user/0/*player.sock\")[0]); "
            "s.sendall(b\"{\\\"ask\\\":\\\"sinks\\\"}\\n\"); time.sleep(600)'"
        )
        wait_for(reading_the_graph, True)
        for _ in range(30):
            listed = [s["device"] for s in player({"ask": "look"}).get("sinks", [])]
            if "ronoctua-test-sink-two" in listed:
                break
            machine.sleep(duration=timedelta(seconds=1))
        assert {"ronoctua-test-sink", "ronoctua-test-sink-two"} <= set(listed), listed
        machine.succeed("systemctl stop holder")
        wait_for(reading_the_graph, False)

    with subtest("a lane sent to the second output is heard there, and read while it is"):
        lanes = player({"ask": "look"})["lanes"]
        sound = next(l["id"] for l in lanes if l["kind"] == "audio")
        said = player({"ask": "lane", "lane": sound,
                       "change": {"what": "sink", "device": "ronoctua-test-sink-two"}})
        assert said["say"] == "done", said
        for _ in range(30):
            if "ronoctua-test-sink-two" in stream_targets():
                break
            machine.sleep(duration=timedelta(seconds=1))
        assert "ronoctua-test-sink-two" in stream_targets(), stream_targets()
        # A LANE WITH ITS OWN OUTPUT keeps the graph read: it has to hear the
        # output go away.
        wait_for(reading_the_graph, True)
        said = player({"ask": "lane", "lane": sound, "change": {"what": "sink"}})
        assert said["say"] == "done", said
        wait_for(reading_the_graph, False)

    # THE SOUND VIEW'S MOVE, against the pinned PipeWire and WirePlumber: what
    # `ronoctua-audio` runs to send a playing program to another device
    # (`pipewire::move_argv`, in the environment `send_away` gives it), and
    # what it reads back — the LINKS, as `graph.rs` reads them.
    def moved_stream():
        """The id of the program this subtest plays, the outputs by name with
        their serials, and the names of the sinks its links reach."""
        dump = graph()
        props = {o["id"]: (o.get("info") or {}).get("props", {}) for o in dump
                 if o.get("type") == "PipeWire:Interface:Node"}
        stream = next((i for i, p in props.items()
                       if p.get("media.class") == "Stream/Output/Audio"
                       and p.get("node.name") == "pw-cat"), None)
        serials = {p.get("node.name"): p.get("object.serial") for p in props.values()
                   if p.get("media.class") == "Audio/Sink"}
        reached = {props.get(o["info"].get("input-node-id"), {}).get("node.name") for o in dump
                   if o.get("type") == "PipeWire:Interface:Link"
                   and (o.get("info") or {}).get("output-node-id") == stream}
        return stream, serials, reached

    def sent(stream, serial):
        return machine.succeed(
            "PIPEWIRE_RUNTIME_DIR=/run/pipewire PIPEWIRE_DEBUG=E LC_ALL=C "
            f"pw-metadata --name default {stream} target.object {serial} Spa:Id 2>&1"
        )

    def reaches(name):
        for _ in range(30):
            if moved_stream()[2] == {name}:
                return
            machine.sleep(duration=timedelta(seconds=1))
        assert moved_stream()[2] == {name}, moved_stream()

    with subtest("a program sent to the other output by its target.object plays there"):
        machine.succeed(
            "systemd-run --unit=moved --setenv=PIPEWIRE_RUNTIME_DIR=/run/pipewire "
            "pw-cat --playback --raw --format s16 --rate 48000 --channels 2 /dev/zero"
        )
        reaches("ronoctua-test-sink")
        stream, serials, _ = moved_stream()
        said = sent(stream, serials["ronoctua-test-sink-two"])
        assert "set property:" in said and "error" not in said, said
        reaches("ronoctua-test-sink-two")
        # AND BACK, the same way: a second move is not a first one's echo.
        said = sent(stream, serials["ronoctua-test-sink"])
        assert "set property:" in said and "error" not in said, said
        reaches("ronoctua-test-sink")

    with subtest("a program that ended is refused by the server, and only in its log"):
        machine.succeed("systemctl stop moved")
        for _ in range(30):
            if moved_stream()[0] is None:
                break
            machine.sleep(duration=timedelta(seconds=1))
        assert moved_stream()[0] is None, moved_stream()
        # THE STATUS IS 0 (`succeed`): the refusal is the log's `res:-2`,
        # which is what `pipewire::answer_of` reads as a program gone.
        said = sent(stream, serials["ronoctua-test-sink-two"])
        assert "set property error" in said and "res:-2 " in said, said

    with subtest("nothing panicked"):
        journal = machine.succeed("journalctl -u player --no-pager")
        assert "panicked" not in journal, journal
  '';
}
