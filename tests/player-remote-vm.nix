# Two machines playing in step: pairing by a code, the media over HTTP from
# the one being followed, and the follower within a frame's worth of it.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The rules are proven in the crates — the code, the key, the clock, the sync
# rule. What none of them reaches is two daemons on two machines, over a real
# network, with nothing shared but the wire:
#
#   invite -> pair by code -> join -> the master's lanes, each file over
#   HTTP with `Range` -> the clock measured -> every voice of the follower in
#   step -> the role choosing the lanes -> leave, and the follower's own
#   queue exactly as it was
#
# THE TAILNET IS THE ONLY THING STOOD IN FOR. The daemon listens only on the
# tailnet's addresses and accepts only its ranges (`remote::on_the_tailnet`),
# and what tells it which addresses and which peers it has is the Tailscale
# daemon's local API, on its socket. A real `tailscaled` needs a control
# server; so each machine gets the address a tailnet would give it
# (100.64.0.0/10) on the test network, and a local API of a few lines that
# answers the two requests the daemon makes — `status` and `prefs` — with it.
# Everything past that socket is the code that runs on a real tailnet.
#
# NO SOUND SERVER, as in `player-vm`: the null output plays, and in step.
{
  pkgs,
  lib,
  player,
}:

let
  media = pkgs.runCommand "player-remote-media" { nativeBuildInputs = [ pkgs.ffmpeg-full ]; } ''
    mkdir -p $out
    ffmpeg -loglevel error -f lavfi -i testsrc2=size=320x180:rate=25:duration=120 \
      -f lavfi -i sine=frequency=440:duration=120 -c:v libx264 -pix_fmt yuv420p -c:a aac \
      -shortest $out/film.mp4
  '';

  # THE LOCAL API, standing in for `tailscaled`: `status` names this machine
  # and its one peer, both online, with the addresses the test gives them.
  localApi = pkgs.writeText "fake-tailscaled.py" ''
    import http.server, json, os, socketserver, sys
    me, me_ip, peer, peer_ip = sys.argv[1:5]
    def node(name, ip):
        return {"ID": name, "HostName": name, "DNSName": name + ".tail.ts.net.",
                "OS": "linux", "Online": True, "TailscaleIPs": [ip]}
    STATUS = json.dumps({
        "BackendState": "Running",
        "MagicDNSSuffix": "tail.ts.net",
        "CurrentTailnet": {"Name": "test", "MagicDNSEnabled": True},
        "Self": node(me, me_ip),
        "Peer": {"peer": node(peer, peer_ip)},
    }).encode()
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            body = STATUS if self.path.startswith("/localapi/v0/status") else b"{}"
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        def log_message(self, *a):
            pass
        # A UNIX SOCKET HAS NO PEER ADDRESS, and the handler asks for one.
        def address_string(self):
            return "local"
    class Server(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
        daemon_threads = True
    path = "/run/tailscale/tailscaled.sock"
    os.makedirs("/run/tailscale", exist_ok=True)
    if os.path.exists(path):
        os.unlink(path)
    Server(path, Handler).serve_forever()
  '';

  client = pkgs.writeText "player-remote-client.py" ''
    import json, socket, sys
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(30)
    s.connect("/run/user/0/ronoctua-player.sock")
    s.sendall((sys.argv[1] + "\n").encode())
    print(s.makefile().readline().strip())
  '';

  machine = address: {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;
    # CURL asks the master for a lane's file the way a guest's mpv does.
    environment.systemPackages = [
      pkgs.python3
      pkgs.curl
    ];
    # THE PLAYER'S PORT, and only that: the test network is not the tailnet,
    # and the daemon itself refuses anything from outside its ranges.
    networking.firewall.allowedTCPPorts = [ 45517 ];
    # A TAILNET ADDRESS on the test network, as `tailscale0` would carry it.
    networking.interfaces.eth1.ipv4.addresses = lib.mkAfter [
      {
        inherit address;
        prefixLength = 10;
      }
    ];
  };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-player-remote";

  nodes.host = machine "100.64.0.1";
  nodes.guest = machine "100.64.0.2";

  testScript = ''
    import json

    def player(m, what):
        said = m.succeed(f"python3 ${client} '{json.dumps(what)}'")
        return json.loads(said)

    start_all()
    for m, me, me_ip, peer, peer_ip in [
        (host, "host", "100.64.0.1", "guest", "100.64.0.2"),
        (guest, "guest", "100.64.0.2", "host", "100.64.0.1"),
    ]:
        m.wait_for_unit("multi-user.target")
        m.succeed("mkdir -p /run/user/0 && chmod 700 /run/user/0")
        m.succeed(
            f"systemd-run --unit=tailscaled python3 ${localApi} {me} {me_ip} {peer} {peer_ip}"
        )
        m.wait_until_succeeds("test -S /run/tailscale/tailscaled.sock", timeout=30)
        m.succeed(
            "systemd-run --unit=player --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=RUST_LOG=ronoctua_player=debug "
            "${player}/bin/ronoctua-player"
        )
        m.wait_until_succeeds("test -S /run/user/0/ronoctua-player.sock", timeout=30)

    with subtest("each daemon listens on its tailnet address, and only there"):
        host.wait_until_succeeds("ss -ltn | grep -q '100.64.0.1:45517'", timeout=30)
        guest.wait_until_succeeds("ss -ltn | grep -q '100.64.0.2:45517'", timeout=30)
        listening = host.succeed("ss -ltn | grep ':45517' || true")
        assert "0.0.0.0:45517" not in listening and "*:45517" not in listening, listening

    with subtest("the host plays, and shows a code"):
        said = player(host, {"ask": "put", "what": ["${media}/film.mp4"], "replace": True})
        assert said["say"] == "done", said
        player(host, {"ask": "invite"})
        host.sleep(1)
        code = player(host, {"ask": "look"})["invite"]
        assert code and len(code) == 6 and code.isdigit(), code

    with subtest("the guest pairs with it by that code — and a wrong one is refused"):
        wrong = str((int(code) + 1) % 1000000).zfill(6)
        said = player(guest, {"ask": "pair-with", "to": "host", "code": wrong})
        assert said["say"] == "refused", f"a wrong code paired: {said}"
        player(host, {"ask": "invite"})
        host.sleep(1)
        code = player(host, {"ask": "look"})["invite"]
        said = player(guest, {"ask": "pair-with", "to": "host", "code": code})
        assert said["say"] == "done", said
        guest.wait_until_succeeds(
            "python3 ${client} '{\"ask\": \"look\"}' | grep -q '\"hosts\":\\[\"host\"'",
            timeout=20,
        )
        host.wait_until_succeeds(
            "python3 ${client} '{\"ask\": \"look\"}' | grep -q '\"guests\":\\[\"guest\"'",
            timeout=20,
        )

    with subtest("the guest joins, plays the host's lanes over HTTP, and keeps in step"):
        # THE GUEST'S OWN QUEUE, paused, which joining must never touch.
        said = player(guest, {"ask": "put", "what": ["${media}/film.mp4"], "replace": True})
        assert said["say"] == "done", said
        guest.sleep(1)
        player(guest, {"ask": "pause"})
        guest.sleep(1)
        own_queue = player(guest, {"ask": "queue"})
        own_current = player(guest, {"ask": "look"})["current"]
        said = player(guest, {"ask": "join", "host": "host", "role": "both"})
        assert said["say"] == "done", said
        # THE SETTLE WINDOW is four seconds after a load; past it the follower
        # is only nudged, and what it says of itself is the drift.
        guest.sleep(10)
        # A FILM WITH A SOUND follows as a sound, nudged at two percent
        # (sync::NUDGE_SOUND): what the settle window leaves, up to 120 ms
        # (sync::SETTLE_MS), takes up to four more seconds to come under a
        # frame. It has to come, within a bound, and then stay.
        caught = []
        for _ in range(10):
            j = player(guest, {"ask": "look"})["joined"]
            assert j and j["host"] == "host", j
            caught.append(j["drift_ms"])
            if abs(caught[-1]) < 80:
                break
            guest.sleep(1)
        print(f"the guest's drift while it catches up, ms: {caught}")
        assert abs(caught[-1]) < 80, f"never came in step: {caught}"
        drifts = []
        for _ in range(8):
            s = player(guest, {"ask": "look"})
            j = s["joined"]
            assert j and j["host"] == "host", s
            drifts.append(j["drift_ms"])
            guest.sleep(1)
        print(f"the guest's drift, ms: {drifts}")
        s = player(guest, {"ask": "look"})
        # WHAT PLAYS IS THE HOST'S LANES — its film's picture and its sound,
        # one file, served once at the path of its first lane — and nothing
        # of the guest's queue is current while it follows.
        host_s = player(host, {"ask": "look"})
        leaf = host_s["current"]
        assert [l["kind"] for l in s["lanes"]] == ["video", "audio"], s["lanes"]
        sources = {l["source"] for l in s["lanes"]}
        assert sources == {f"http://100.64.0.1:45517/media/{leaf}/1"}, sources
        assert s["location"].startswith("http://100.64.0.1:45517/media/"), s["location"]
        assert s["current"] is None, s["current"]
        assert s["playing"], s
        # STAYING IN STEP, under a battery that runs several VMs at once: a
        # reading is a whole number of frames (40 ms at 25 fps), and one
        # scheduler hiccup lands a single reading one frame late — measured,
        # [40, 40, 40, 40, 40, 80, 40, 40] beside two other VMs, all 40 alone.
        # So one reading may touch two frames; none may pass them, two may
        # not, and the last is back under one.
        late = [d for d in drifts if abs(d) >= 80]
        assert all(abs(d) <= 80 for d in drifts), f"out of step: {drifts}"
        assert len(late) <= 1, f"out of step more than once: {drifts}"
        assert abs(drifts[-1]) < 80, f"did not come back in step: {drifts}"
        # A LANE OF WHAT DOES NOT PLAY is not served, key or no key.
        key = json.loads(guest.succeed("cat /root/.local/state/ronoctua/player-peers.json"))["hosts"][0]["key"]
        code = guest.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' -H 'Authorization: Bearer {key}' "
            f"http://100.64.0.1:45517/media/{leaf + 999}/1"
        ).strip()
        assert code == "404", code
        code = guest.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' -r 0-99 -H 'Authorization: Bearer {key}' "
            f"http://100.64.0.1:45517/media/{leaf}/2"
        ).strip()
        assert code == "206", code

    with subtest("a screen plays the pictures only"):
        said = player(guest, {"ask": "join", "host": "host", "role": "screen"})
        assert said["say"] == "done", said
        guest.wait_until_succeeds(
            "python3 ${client} '{\"ask\": \"look\"}' | python3 -c "
            "'import json,sys; s=json.load(sys.stdin); "
            "sys.exit(0 if [l[\"kind\"] for l in s[\"lanes\"]] == [\"video\"] else 1)'",
            timeout=30,
        )
        s = player(guest, {"ask": "look"})
        assert s["joined"]["role"] == "screen", s["joined"]
        # A GUEST'S OWN ORDERS wait for it to leave: what plays is the host's.
        said = player(guest, {"ask": "pause"})
        assert said == {"say": "refused", "trouble": "following", "host": "host"}, said

    with subtest("leaving gives the guest its own player back"):
        player(guest, {"ask": "leave"})
        guest.sleep(2)
        s = player(guest, {"ask": "look"})
        assert s["joined"] is None, s
        # THE GUEST'S QUEUE IS AS IT WAS, and its own leaf is current again.
        assert player(guest, {"ask": "queue"}) == own_queue, "joining changed the guest's queue"
        assert s["current"] == own_current, (s["current"], own_current)

    with subtest("nothing panicked"):
        for m in (host, guest):
            journal = m.succeed("journalctl -u player --no-pager")
            assert "panicked" not in journal, journal
  '';
}
