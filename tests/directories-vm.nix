# The world's radio and TV against the directories' own addresses — answered
# by a server of the test's over HTTPS, with a certificate authority made for
# this test and trusted by the player's machine alone.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The broadcast crate is tested against canned answers and the engine with a
# network that records (`engine/directory/tests.rs`). What neither reaches is
# what a SERVER sees:
#
#   - nothing at all is asked of a directory before the person lets it — not
#     radio-browser, not iptv-org, not archive.org — measured by the server's
#     log being EMPTY, not by the engine's word;
#   - once let, radio-browser is found through its entry
#     (`all.api.radio-browser.info`, the servers it names) and its lists come
#     from the server it chose;
#   - a station's "click" — radio-browser's count of what is PLAYED — reaches
#     the directory only AFTER the station's own stream was asked
#     (`the-click-waits-for-the-sound`, measured on the two servers' clocks),
#     once per start, and never while the radio is not let;
#   - what played is kept among the recently played, and a star pressed is
#     kept among the favourites — files the DAEMON writes;
#   - a whole film of the archive is refused, naming its host, while the
#     archive is not let — before anything is asked of it.
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

  hosts = [
    "all.api.radio-browser.info"
    "de1.api.radio-browser.info"
    "iptv-org.github.io"
    "archive.org"
  ];
  # A STATION'S STREAM, in the clear, as most radio streams are: mpv opens
  # it, not a lookup — so no pass, and no certificate.
  streamHost = "stream.radio.test";

  pki = pkgs.runCommand "ronoctua-directories-pki" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    cd $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=ronoctua test authority" \
      -keyout ca.key -out ca.crt
    openssl req -newkey rsa:2048 -nodes -subj "/CN=all.api.radio-browser.info" -keyout leaf.key -out leaf.csr
    printf 'subjectAltName=%s\n' "${lib.concatMapStringsSep "," (h: "DNS:${h}") hosts}" > san.ext
    openssl x509 -req -in leaf.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 3650 \
      -extfile san.ext -out leaf.crt
  '';

  uuid = "96062a7b-0601-11e8-ae97-52543be04c81";
  answers = {
    servers = ''[{"ip":"192.0.2.1","name":"de1.api.radio-browser.info"}]'';
    countries = ''[{"name":"Brazil","iso_3166_1":"BR","stationcount":1}]'';
    stations = ''[{"stationuuid":"${uuid}","name":"Rádio do Teste","url":"http://${streamHost}/tone.flac","url_resolved":"http://${streamHost}/tone.flac","countrycode":"BR","codec":"FLAC","bitrate":0,"tags":"test","language":"portuguese","votes":1,"clickcount":1,"hls":0}]'';
    click = ''{"ok":true,"message":"retrieved station url","stationuuid":"${uuid}","name":"Rádio do Teste","url":"http://${streamHost}/tone.flac"}'';
  };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-directories";

  nodes.hosts = {
    networking.firewall.allowedTCPPorts = [
      80
      443
    ];
    services.nginx = {
      enable = true;
      appendHttpConfig = ''
        log_format directories '$msec $host $request_method "$request_uri"';
        server {
          listen 443 ssl;
          server_name _;
          ssl_certificate ${pki}/leaf.crt;
          ssl_certificate_key ${pki}/leaf.key;
          access_log /var/log/nginx/directories.log directories;
          default_type application/json;
          location = /json/servers { return 200 '${answers.servers}'; }
          location ^~ /json/countries { return 200 '${answers.countries}'; }
          location ^~ /json/stations/ { return 200 '${answers.stations}'; }
          location ^~ /json/url/ { return 200 '${answers.click}'; }
          location / { return 404 '{}'; }
        }
        server {
          listen 80;
          server_name ${streamHost};
          access_log /var/log/nginx/stream.log directories;
          location = /tone.flac { alias ${media}/tone.flac; default_type audio/flac; }
        }
      '';
    };
  };

  nodes.machine =
    { nodes, ... }:
    {
      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      environment.systemPackages = [ pkgs.python3 ];
      security.pki.certificateFiles = [ "${pki}/ca.crt" ];
      networking.hosts.${nodes.hosts.networking.primaryIPAddress} = hosts ++ [ streamHost ];
    };

  testScript = ''
    import json, re

    def player(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def asked(log="directories"):
        """What the server was asked: (time, host, method, address)."""
        text = hosts.succeed(f"cat /var/log/nginx/{log}.log || true")
        out = []
        for line in text.splitlines():
            m = re.match(r'(\S+) (\S+) (\S+) "([^"]*)"', line)
            if m:
                out.append((float(m[1]), m[2], m[3], m[4]))
        return out

    def let_hosts(names):
        consent = {"hosts": {h: True for h in names}}
        machine.succeed(f"printf '%s' '{json.dumps(consent)}' > /root/.config/ronoctua/consent.json")
        machine.sleep(2)

    def generation(kind):
        d = [d for d in player({"ask": "look"}).get("desks", []) if d["kind"] == kind]
        return d[0]["generation"] if d else 0

    def desk(kind, before):
        for _ in range(60):
            d = [d for d in player({"ask": "look"}).get("desks", []) if d["kind"] == kind]
            if d and d[0]["generation"] > before and not d[0].get("busy"):
                return player({"ask": "desk", "kind": kind})
            machine.sleep(1)
        raise AssertionError(f"the {kind} desk never settled: {player({'ask': 'look'}).get('desks')}")

    def station(live=True):
        return {"stream": {"url": "http://${streamHost}/tone.flac", "title": "Rádio do Teste",
                           "origin": {"radio-browser": {"uuid": "${uuid}"}}, "live": live}}

    def clicks():
        return [a for a in asked() if a[3] == "/json/url/${uuid}"]

    start_all()
    hosts.wait_for_unit("nginx.service")
    hosts.wait_for_open_port(443)
    machine.wait_for_unit("multi-user.target")

    with subtest("the player starts, with no directory let"):
        machine.succeed("mkdir -p /run/user/0 /root/.config/ronoctua && chmod 700 /run/user/0")
        let_hosts([])
        machine.succeed(
            "systemd-run --unit=player --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=RUST_LOG=ronoctua_player=debug ${setenv} "
            "${player}/bin/ronoctua-player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q player.sock", timeout=30)

    with subtest("nothing is asked of a directory the person did not let"):
        for query in ({"what": "radio", "list": "countries"}, {"what": "tv", "list": "countries"}):
            said = player({"ask": "find", "query": query})
            # `{"say":"refused","trouble":"off","host":…}`: the trouble's tag
            # sits beside the answer's.
            assert said["say"] == "refused" and said.get("trouble") == "off", (query, said)
        # A WHOLE FILM OF THE ARCHIVE, refused naming its host, at the put.
        film = {"stream": {"url": "https://archive.org/details/night_of_the_living_dead",
                           "origin": {"archive": {"identifier": "night_of_the_living_dead"}}}}
        said = player({"ask": "put", "what": [film], "replace": True})
        print(said)
        assert "archive.org" in json.dumps(said), f"the archive's film was not refused naming its host: {said}"
        # AND A STATION PLAYED while the radio is not let: no click.
        said = player({"ask": "put", "what": [station()], "replace": True})
        assert said["say"] == "done", said
        hosts.wait_until_succeeds("grep -q tone.flac /var/log/nginx/stream.log", timeout=60)
        machine.sleep(5)
        assert asked() == [], f"a directory was asked before it was let: {asked()}"
        player({"ask": "stop"})

    with subtest("let, radio-browser is found by its entry and lists from the server it chose"):
        let_hosts(["radio-browser.info"])
        before = generation("radio")
        said = player({"ask": "find", "query": {"what": "radio", "list": "countries"}})
        assert said["say"] == "done", said
        page = desk("radio", before)
        print(json.dumps(page)[:400])
        assert "BR" in json.dumps(page.get("page", {})), f"the countries did not arrive: {page}"
        seen = asked()
        assert seen and seen[0][1] == "all.api.radio-browser.info" and seen[0][3] == "/json/servers", (
            f"radio-browser was not found through its entry first: {seen}"
        )
        assert any(a[1] == "de1.api.radio-browser.info" and a[3].startswith("/json/countries") for a in seen), seen
        before = generation("radio")
        said = player({"ask": "find", "query": {"what": "radio", "list": "country", "country": "BR"}})
        assert said["say"] == "done", said
        page = desk("radio", before)
        assert "${uuid}" in json.dumps(page), f"the country's stations did not arrive: {page}"

    with subtest("the click waits for the sound, once per start"):
        assert clicks() == [], f"a click before anything played: {clicks()}"
        streamed_before = len(asked("stream"))
        said = player({"ask": "put", "what": [station()], "replace": True})
        assert said["say"] == "done", said
        hosts.wait_until_succeeds(
            f"test $(grep -c tone.flac /var/log/nginx/stream.log) -gt {streamed_before}", timeout=60
        )
        for _ in range(30):
            if clicks():
                break
            machine.sleep(1)
        assert len(clicks()) == 1, f"one start, {len(clicks())} clicks"
        sound = asked("stream")[streamed_before][0]
        assert clicks()[0][0] >= sound, f"the click ({clicks()[0][0]}) came before the sound ({sound})"
        machine.sleep(5)
        assert len(clicks()) == 1, f"one start, and the click came again: {clicks()}"

    with subtest("what played is kept among the recent, and a star is kept by the daemon"):
        machine.wait_until_succeeds("grep -q ${uuid} /root/.local/state/ronoctua/player-recent.json", timeout=30)
        said = player({"ask": "favourite", "kind": "radio", "id": "${uuid}", "on": True})
        assert said["say"] == "done", said
        kept = json.loads(machine.succeed("cat /root/.config/ronoctua/player-favourites.json"))
        assert any(p.get("id") == "${uuid}" for p in kept["picks"]), kept
        said = player({"ask": "favourite", "kind": "radio", "id": "${uuid}", "on": False})
        assert said["say"] == "done", said
        kept = json.loads(machine.succeed("cat /root/.config/ronoctua/player-favourites.json"))
        assert not any(p.get("id") == "${uuid}" for p in kept["picks"]), kept

    with subtest("the radio no longer let: a station plays, and is not counted"):
        let_hosts([])
        player({"ask": "stop"})
        before = len(clicks())
        streamed_before = len(asked("stream"))
        said = player({"ask": "put", "what": [station()], "replace": True})
        assert said["say"] == "done", said
        hosts.wait_until_succeeds(
            f"test $(grep -c tone.flac /var/log/nginx/stream.log) -gt {streamed_before}", timeout=60
        )
        machine.sleep(5)
        assert len(clicks()) == before, f"a click while the radio is not let: {clicks()}"

    with subtest("nothing panicked"):
        journal = machine.succeed("journalctl -u player --no-pager")
        assert "panicked" not in journal, journal
  '';
}
