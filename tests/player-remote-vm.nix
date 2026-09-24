# Two machines playing in step: pairing by a code, the media over HTTP from
# the one being followed, and the follower within a frame's worth of it.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The rules are proven in the crates — the code, the key, the clock, the sync
# rule. What none of them reaches is two daemons on two machines, over a real
# network, with nothing shared but the wire:
#
#   invite -> pair by code -> join -> the media over HTTP with `Range`
#   -> the clock measured -> the follower in step
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
    environment.systemPackages = [ pkgs.python3 ];
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
            "--setenv=HOME=/root --setenv=RUST_LOG=ronoctua_player_engine=debug "
            "${player}/bin/ronoctua-player"
        )
        m.wait_until_succeeds("test -S /run/user/0/ronoctua-player.sock", timeout=30)

    with subtest("each daemon listens on its tailnet address, and only there"):
        host.wait_until_succeeds("ss -ltn | grep -q '100.64.0.1:45517'", timeout=30)
        guest.wait_until_succeeds("ss -ltn | grep -q '100.64.0.2:45517'", timeout=30)
        listening = host.succeed("ss -ltn | grep ':45517' || true")
        assert "0.0.0.0:45517" not in listening and "*:45517" not in listening, listening

    with subtest("the host plays, and shows a code"):
        said = player(host, {"ask": "open", "items": ["${media}/film.mp4"], "replace": True})
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

    with subtest("the guest joins, plays the host's film over HTTP, and keeps in step"):
        said = player(guest, {"ask": "join", "host": "host", "role": "both"})
        assert said["say"] == "done", said
        # THE SETTLE WINDOW is four seconds after a load; past it the follower
        # is only nudged, and what it says of itself is the drift.
        guest.sleep(10)
        drifts = []
        for _ in range(8):
            s = player(guest, {"ask": "look"})
            j = s["joined"]
            assert j and j["host"] == "host", s
            drifts.append(j["drift_ms"])
            guest.sleep(1)
        print(f"the guest's drift, ms: {drifts}")
        s = player(guest, {"ask": "look"})
        playing = s["queue"][s["current"]]["location"]
        assert playing.startswith("http://100.64.0.1:45517/"), playing
        assert s["playing"], s
        assert max(abs(d) for d in drifts) < 80, f"out of step: {drifts}"

    with subtest("leaving gives the guest its own player back"):
        player(guest, {"ask": "leave"})
        guest.sleep(2)
        s = player(guest, {"ask": "look"})
        assert s["joined"] is None, s

    with subtest("nothing panicked"):
        for m in (host, guest):
            journal = m.succeed("journalctl -u player --no-pager")
            assert "panicked" not in journal, journal
  '';
}
