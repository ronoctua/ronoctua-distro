# The player's LOOKUPS against the services' own addresses — answered by a
# server of the test's, on the other node, over HTTPS with a certificate
# authority made for this test and trusted by the player's machine alone.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The lookup crate is tested against canned answers (`ronoctua_lookup::
# canned`): what it SENDS is asserted there, never what reaches a socket. And
# the engine is tested with a network that records. What neither reaches:
#
#   - MusicBrainz's one request a second, measured by the SERVER's clock
#     between the requests one suggestion makes — the search, the
#     recording's own page, its release (`ronoctua_lookup::pace`);
#   - the headers as the server receives them: the player's name and the
#     public repository in `User-Agent`, TMDB's token as a Bearer, the
#     OpenSubtitles key in `Api-Key` — and no key in any address;
#   - no key in any process's command line while the lookups run;
#   - a subtitle found, taken, fetched, saved beside the film and become a
#     lane of it — the whole chain, from the find to the lane.
#
# THE KEYS are written as a door writes them (`<config>/player-keys/<file>`,
# 0600): what a door does with what the person types is the shell's, and
# `lookup-keys-are-private` holds how the file is made.
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

  # EVERY HOST A LOOKUP OF THIS TEST ASKS, answered by the one server. A
  # service is let only when EVERY host of its `Service::hosts` is let
  # (`Preferences::pressed`), so a service's list here is the whole of it:
  # TMDB is its API and its images, and without `image.tmdb.org` the search
  # is refused before a request leaves the machine.
  hosts = [
    "musicbrainz.org"
    "coverartarchive.org"
    "api.themoviedb.org"
    "image.tmdb.org"
    "api.opensubtitles.com"
    "lrclib.net"
  ];

  # A CERTIFICATE AUTHORITY MADE FOR THIS TEST, and a certificate for every
  # host above signed by it. The player's machine trusts it through
  # `security.pki` — the bundle `ronoctua_http::tls` reads — and nothing else
  # does: the keys never leave the store path of a test.
  pki = pkgs.runCommand "ronoctua-lookups-pki" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    cd $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=ronoctua test authority" \
      -keyout ca.key -out ca.crt
    openssl req -newkey rsa:2048 -nodes -subj "/CN=musicbrainz.org" -keyout leaf.key -out leaf.csr
    printf 'subjectAltName=%s\n' "${lib.concatMapStringsSep "," (h: "DNS:${h}") hosts}" > san.ext
    openssl x509 -req -in leaf.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 3650 \
      -extfile san.ext -out leaf.crt
  '';

  # THE ANSWERS, in the shapes the crate parses (its own tests' fixtures):
  # one recording found, its own page, and its release — the requests to
  # MusicBrainz one suggestion makes, which is what the pace is measured on.
  recId = "11111111-1111-4111-8111-111111111111";
  relId = "22222222-2222-4222-8222-222222222222";
  answers = {
    search = ''{"count":1,"recordings":[{"id":"${recId}","score":100,"title":"Tone","length":12000,"artist-credit":[{"name":"Band","joinphrase":""}],"releases":[{"id":"${relId}","title":"Album","date":"2005"}]}]}'';
    recording = ''{"id":"${recId}","title":"Tone","length":12000,"artist-credit":[{"name":"Band","joinphrase":""}],"releases":[{"id":"${relId}","title":"Album","date":"2005"}]}'';
    release = ''{"id":"${relId}","title":"Album","date":"2005","artist-credit":[{"name":"Band","joinphrase":""}],"cover-art-archive":{"front":false,"back":false,"count":0},"media":[{"position":1,"track-count":1,"tracks":[{"position":1,"recording":{"id":"${recId}"}}]}]}'';
    tmdb = ''{"page":1,"results":[],"total_results":0}'';
    subtitles = ''{"total_count":1,"data":[{"id":"1","attributes":{"language":"en","download_count":10,"release":"film","files":[{"file_id":7,"file_name":"film.srt"}],"feature_details":{"title":"Film","year":2020}}}]}'';
    download = ''{"link":"https://api.opensubtitles.com/files/film.srt","file_name":"film.srt","remaining":19}'';
  };
  srt = "1\\n00:00:01,000 --> 00:00:04,000\\nWords from the network\\n";

  # THE KEYS, made up: what is looked for in every address and command line.
  keys = {
    opensubtitles = "osk-planted-key-0123456789";
    tmdb = "eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJyb25vY3R1YSJ9.c2lnbmF0dXJlLXBsYW50ZWQ";
  };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-lookups";

  nodes.hosts = {
    networking.firewall.allowedTCPPorts = [ 443 ];
    services.nginx = {
      enable = true;
      appendHttpConfig = ''
        log_format lookups '$msec $host $request_method "$request_uri" ua="$http_user_agent" auth="$http_authorization" apikey="$http_api_key"';
        server {
          listen 443 ssl;
          server_name _;
          ssl_certificate ${pki}/leaf.crt;
          ssl_certificate_key ${pki}/leaf.key;
          access_log /var/log/nginx/lookups.log lookups;
          default_type application/json;
          location = /ws/2/recording { return 200 '${answers.search}'; }
          location ^~ /ws/2/recording/ { return 200 '${answers.recording}'; }
          location ^~ /ws/2/release/ { return 200 '${answers.release}'; }
          location ^~ /3/search/ { return 200 '${answers.tmdb}'; }
          location = /api/v1/subtitles { return 200 '${answers.subtitles}'; }
          location = /api/v1/download { return 200 '${answers.download}'; }
          location = /files/film.srt { default_type text/plain; return 200 "${srt}"; }
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
      # THE TEST'S AUTHORITY, in the system's bundle — the one place
      # `ronoctua_http::tls` reads roots from.
      security.pki.certificateFiles = [ "${pki}/ca.crt" ];
      # EVERY SERVICE'S NAME, pointed at the test's server.
      networking.hosts.${nodes.hosts.networking.primaryIPAddress} = hosts;
    };

  testScript = ''
    import json, re

    def player(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def asked():
        """What the server was asked, a line a request: (time, host, method,
        address, user agent, authorization, api key)."""
        log = hosts.succeed("cat /var/log/nginx/lookups.log || true")
        out = []
        for line in log.splitlines():
            m = re.match(r'(\S+) (\S+) (\S+) "([^"]*)" ua="([^"]*)" auth="([^"]*)" apikey="([^"]*)"', line)
            if m:
                out.append((float(m[1]), m[2], m[3], m[4], m[5], m[6], m[7]))
        return out

    def desk(kind, generation_before):
        """The desk's page once its generation has moved past `before` and it
        is no longer busy."""
        for _ in range(60):
            d = [d for d in player({"ask": "look"}).get("desks", []) if d["kind"] == kind]
            if d and d[0]["generation"] > generation_before and not d[0].get("busy"):
                return player({"ask": "desk", "kind": kind})
            machine.sleep(1)
        raise AssertionError(f"the {kind} desk never settled: {player({'ask': 'look'}).get('desks')}")

    def generation(kind):
        d = [d for d in player({"ask": "look"}).get("desks", []) if d["kind"] == kind]
        return d[0]["generation"] if d else 0

    start_all()
    hosts.wait_for_unit("nginx.service")
    hosts.wait_for_open_port(443)
    machine.wait_for_unit("multi-user.target")

    with subtest("the player starts, its hosts let and its keys kept as a door keeps them"):
        machine.succeed("mkdir -p /run/user/0 /root/.config/ronoctua/player-keys /root/films")
        machine.succeed("chmod 700 /run/user/0 /root/.config/ronoctua/player-keys")
        consent = {"hosts": {h: True for h in ${builtins.toJSON hosts}}}
        machine.succeed(f"printf '%s' '{json.dumps(consent)}' > /root/.config/ronoctua/consent.json")
        for name, value in json.loads('${builtins.toJSON keys}').items():
            machine.succeed(
                f"printf '%s' '{value}' | install -m 600 /dev/stdin /root/.config/ronoctua/player-keys/{name}"
            )
        machine.succeed("cp ${media}/film.mp4 ${media}/tone.flac /root/films/")
        machine.succeed(
            "systemd-run --unit=player --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=RUST_LOG=ronoctua_player=debug ${setenv} "
            "${player}/bin/ronoctua-player"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q player.sock", timeout=30)

    with subtest("one suggestion is several requests to MusicBrainz, a second apart at least"):
        said = player({"ask": "put", "what": ["/root/films/tone.flac"], "replace": True})
        assert said["say"] == "done", said
        before = generation("suggestions")
        said = player({"ask": "find", "query": {"what": "suggest", "location": "/root/films/tone.flac",
                                                "source": "musicbrainz", "words": "Tone"}})
        assert said["say"] == "done", said
        page = desk("suggestions", before)
        print(json.dumps(page)[:400])
        brainz = [a for a in asked() if a[1] == "musicbrainz.org"]
        print("\n".join(f"{a[0]:.3f} {a[3]}" for a in brainz))
        assert len(brainz) >= 2, f"MusicBrainz was asked {len(brainz)} time(s), not the search and the recording's own page"
        gaps = [b[0] - a[0] for a, b in zip(brainz, brainz[1:])]
        assert min(gaps) >= 0.99, f"MusicBrainz was asked faster than once a second: {gaps}"

    with subtest("the headers: the player named, TMDB's token a Bearer, no key in an address"):
        before = generation("suggestions")
        said = player({"ask": "find", "query": {"what": "suggest", "location": "/root/films/film.mp4",
                                                "source": "tmdb", "words": "Film"}})
        assert said["say"] == "done", said
        desk("suggestions", before)
        for a in asked():
            assert a[4].startswith("ronoctua-player/") and "github.com/ronoctua/ronoctua-distro" in a[4], (
                f"{a[1]} was asked without the player's name: {a[4]!r}"
            )
        tmdb = [a for a in asked() if a[1] == "api.themoviedb.org"]
        assert tmdb, "TMDB was never asked"
        assert all(a[5] == "Bearer ${keys.tmdb}" for a in tmdb), f"TMDB's token is not a Bearer header: {tmdb}"

    with subtest("a subtitle is found, taken, fetched, saved beside the film, and plays"):
        said = player({"ask": "put", "what": ["/root/films/film.mp4"], "replace": True})
        assert said["say"] == "done", said
        before = generation("subtitles")
        said = player({"ask": "find", "query": {"what": "subtitles", "location": "/root/films/film.mp4",
                                                "tongues": ["en"]}})
        assert said["say"] == "done", said
        page = desk("subtitles", before)
        print(json.dumps(page)[:600])
        subs = [a for a in asked() if a[1] == "api.opensubtitles.com"]
        assert subs and all(a[6] == "${keys.opensubtitles}" for a in subs if a[3].startswith("/api/")), (
            f"OpenSubtitles was not asked with the key in `Api-Key`: {subs}"
        )
        # A CANDIDATE IS TAKEN BY ITS ID, which the desk hands out and the
        # test reads off the page — never a position: ids are not 0, 1, 2.
        found = [c for c in page["page"]["candidates"] if c["found"]["source"] == "opensubtitles"]
        assert found, f"OpenSubtitles' candidate is not on the page: {page['page']['candidates']}"
        before = generation("subtitles")
        said = player({"ask": "take", "desk": "subtitles", "candidate": found[0]["id"]})
        assert said["say"] == "done", said
        machine.wait_until_succeeds("ls /root/films | grep -q '^film\\.en\\.srt$'", timeout=60)
        assert "Words from the network" in machine.succeed("cat /root/films/film.en.srt")
        for _ in range(30):
            lanes = player({"ask": "look"})["lanes"]
            if any(l["kind"] == "subtitle" and l["source"].endswith("film.en.srt") for l in lanes):
                break
            machine.sleep(1)
        assert any(l["kind"] == "subtitle" and l["source"].endswith("film.en.srt") for l in lanes), (
            f"the saved subtitle did not become a lane of the film: {lanes}"
        )

    with subtest("no key in any address, nor in any process's command line"):
        for a in asked():
            for value in ${builtins.toJSON (builtins.attrValues keys)}:
                assert value not in a[3], f"a key went in an address: {a[1]}{a[3]}"
        lines = machine.succeed("for f in /proc/[0-9]*/cmdline; do tr '\\0' ' ' < $f; echo; done 2>/dev/null")
        for value in ${builtins.toJSON (builtins.attrValues keys)}:
            assert value not in lines, "a key is in a process's command line"

    with subtest("nothing panicked"):
        journal = machine.succeed("journalctl -u player --no-pager")
        assert "panicked" not in journal, journal
  '';
}
