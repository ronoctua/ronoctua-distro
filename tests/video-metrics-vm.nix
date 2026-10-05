# The video catalogue's scan on a BOOTED machine: the brain starts the unit,
# the unit asks a fixture instead of YouTube, and a reader reads while it
# writes.
#
# WHAT ONLY THE VM PROVES
# -----------------------
#   1. ADDING A CHANNEL BY HAND (`ronoctua-video-metrics add @handle`) asks
#      yt-dlp who it is — a stand-in here, by store path — makes the
#      catalogue under the person's data folder, and lets the two hosts a
#      scan asks (`consent.json`): adding a channel is the consent to scan it.
#   2. THE BRAIN STARTS THE UNIT the manifest names, as the person's systemd
#      unit, and hears it through the report the unit writes in
#      `%t/ronoctua-brain`: the scanning mark is on the mind while it runs,
#      gone after, and the run is recorded done.
#   3. THE UNIT RUNS AS THE MODULE SAYS: a oneshot, niced, idle on the disk,
#      with the feeds' origin and yt-dlp from its environment — evaluated as
#      the machine got it, not as the file reads.
#   4. A READER READS WHILE THE SCAN WRITES: `list` opens the catalogue
#      read-only (`Catalog::open_ro`) again and again during the scan, and
#      never meets a lock — the WAL's promise, on the real SQLite of the
#      system.
#   5. THE FIRST SCAN'S SAFEGUARD on a real feed: the videos of the last two
#      weeks, at most the newest ten; what is older lies under a tombstone.
#   6. HIDE AND DELETE FROM THE COMMAND LINE, through the readers' two
#      writes: a hidden video leaves the list and stays catalogued; a deleted
#      one leaves it for good — a second scan, of the same feed, does not
#      bring it back.
#   7. A SCAN KILLED BEFORE ITS END IS NO SCAN RUNNING: a report left
#      "begun" with nobody holding the catalogue is read by a brain that
#      starts after it, and no mark comes up; and the unit's `ExecStopPost`
#      (`scan-stopped`) writes the end such a scan never wrote, with
#      systemd's word for what happened — and leaves an ended report alone.
#
# WHAT IS NOT HERE: YouTube. The feeds come from a loopback fixture
# (`RONOCTUA_VIDEO_METRICS_FEEDS`, which takes nothing but a loopback `http`
# origin), and details from a stand-in yt-dlp that answers what the pinned one
# answers, slowly — so the scan lasts long enough to be read during. With a
# loopback origin a scan fetches no thumbnails (they are YouTube's pictures).
{
  pkgs,
  lib,
  ronoctua,
  videoMetrics,
}:

let
  port = 8123;
  # A channel id is `UC` and twenty-two more; a video id is eleven.
  channel = "UCfixturefixturefixture0";
  handle = "@fixture";

  # THE FEEDS, made when the server starts, so the dates are the VM's today:
  # fourteen videos a day apart, the newest an hour old, and one Short, a day
  # old — fifteen inside the first scan's two weeks, of which it takes the
  # newest ten (nine videos and the Short). Fourteen and not fifteen: a full
  # feed of new videos is a backlog, which asks a tab of yt-dlp.
  server = pkgs.writeText "feeds.py" ''
    import datetime, http.server, sys, urllib.parse

    CHANNEL = "${channel}"
    REST = CHANNEL[2:]
    NOW = datetime.datetime.now(datetime.timezone.utc)

    def video_id(n):
        return f"fixture{n:04d}"

    def entry(vid, title, when, short=False):
        link = f"https://www.youtube.com/shorts/{vid}" if short else f"https://www.youtube.com/watch?v={vid}"
        return f"""
      <entry>
        <yt:videoId>{vid}</yt:videoId>
        <yt:channelId>{CHANNEL}</yt:channelId>
        <title>{title}</title>
        <link rel="alternate" href="{link}"/>
        <published>{when.strftime('%Y-%m-%dT%H:%M:%S+00:00')}</published>
        <media:group>
          <media:title>{title}</media:title>
          <media:description>A fixture's video.</media:description>
        </media:group>
      </entry>"""

    def feed(entries):
        return ('<?xml version="1.0" encoding="UTF-8"?>'
                '<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" '
                'xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">'
                f"<yt:channelId>{CHANNEL}</yt:channelId><title>Fixture</title>"
                + "".join(entries) + "</feed>").encode()

    VIDEOS = feed(entry(video_id(n), f"Fixture video {n}", NOW - datetime.timedelta(hours=1 + 24 * n))
                  for n in range(14))
    SHORTS = feed([entry("fixtureshrt", "Fixture short", NOW - datetime.timedelta(days=1), short=True)])

    class Feeds(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            url = urllib.parse.urlparse(self.path)
            asked = urllib.parse.parse_qs(url.query)
            body = None
            if url.path == "/feeds/videos.xml":
                playlist = asked.get("playlist_id", [""])[0]
                if playlist == "UULF" + REST:
                    body = VIDEOS
                elif playlist == "UUSH" + REST:
                    body = SHORTS
            if body is None:
                self.send_response(404)
                self.end_headers()
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/atom+xml")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            sys.stderr.write("feeds: " + (args[0] % args[1:]) + "\n")

    http.server.ThreadingHTTPServer(("127.0.0.1", ${toString port}), Feeds).serve_forever()
  '';

  # A STAND-IN yt-dlp: who the channel is (`--dump-single-json`, what `add`
  # asks) and one video's fields (`--dump-json`, what a scan's details ask),
  # two seconds each — the pinned yt-dlp's answer shapes, and a scan slow
  # enough to be read during.
  ytDlp = pkgs.writeScript "yt-dlp" ''
    #!${pkgs.python3}/bin/python3
    import json, sys, time
    args = sys.argv[1:]
    url = args[-1]
    if "--dump-single-json" in args:
        print(json.dumps({"channel_id": "${channel}", "uploader_id": "${handle}",
                          "channel": "Fixture", "entries": []}))
    elif "--dump-json" in args:
        time.sleep(2)
        vid = url.rsplit("=", 1)[-1].rsplit("/", 1)[-1]
        print(json.dumps({"id": vid, "fulltitle": f"Fixture video {vid}", "duration": 600,
                          "timestamp": int(time.time()) - 3600, "channel_id": "${channel}",
                          "channel": "Fixture", "width": 1920, "height": 1080,
                          "description": "A fixture's video, in detail.", "tags": ["fixture"],
                          "categories": ["Education"], "age_limit": 0,
                          "webpage_url": f"https://www.youtube.com/watch?v={vid}"}))
    else:
        sys.exit("the stand-in answers who a channel is and a video's fields, nothing else")
  '';

  feeds = "http://127.0.0.1:${toString port}";

  # A QUIET BRAIN: nothing starts by itself, and what is asked still runs.
  brainPrefs = pkgs.writeText "brain.json" ''{"quiet":true}'';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-video-metrics";

  nodes.machine =
    { config, options, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/media.nix
        ../nix/modules/brain.nix
        (import ../nix/modules/video-metrics.nix { inherit videoMetrics; })
      ];
      _module.args.ronoctua = ronoctua;

      # NO MEDIA, and the scan's module on by hand: this machine proves the
      # scan, and yt-dlp is the stand-in below.
      ronoctua.media.enable = false;
      ronoctua.videoMetrics.enable = true;
      # THE ONE MODULE OF THE BRAIN this machine is about: every other one
      # off, by the list the brain declares — each has its own units and its
      # own tests, and none is named here.
      ronoctua.brain.modules = lib.mapAttrs (id: _: {
        enable = id == "video-metrics";
      }) options.ronoctua.brain.modules;

      systemd.user.services.ronoctua-video-metrics-scan.environment = {
        RONOCTUA_YT_DLP = lib.mkForce "${ytDlp}";
        RONOCTUA_VIDEO_METRICS_FEEDS = feeds;
      };

      systemd.services.fixture-feeds = {
        description = "the video catalogue's fixture feeds";
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${server}";
      };

      virtualisation.memorySize = 2048;
      networking.hostName = lib.mkForce "noctua-video-metrics";
      # No /persist in this VM (the file manager's reason).
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
        # A lingering manager: the person's units and runtime directory
        # without a login.
        linger = true;
        uid = 1000;
      };
      users.users.root.hashedPassword = lib.mkForce null;
      environment.systemPackages = [ pkgs.jq ];
    };

  testScript =
    { nodes, ... }:
    let
      user = nodes.machine.ronoctua.user;
      uid = toString nodes.machine.users.users.${user}.uid;
    in
    ''
      import json, time

      user = "${user}"
      uid = "${uid}"
      run_dir = f"/run/user/{uid}"
      catalogue = f"/home/{user}/.local/share/ronoctua/video-metrics/catalog.sqlite"

      brain = "${ronoctua.brain}/bin/ronoctua-brain"

      def as_person(command, check=True):
          line = (
              f"su - {user} -c 'export XDG_RUNTIME_DIR={run_dir} "
              f"DBUS_SESSION_BUS_ADDRESS=unix:path={run_dir}/bus "
              "RONOCTUA_YT_DLP=${ytDlp} RONOCTUA_VIDEO_METRICS_FEEDS=${feeds}; "
              f"{command} 2>&1'"
          )
          return machine.succeed(line) if check else machine.execute(line)

      def listed(hidden=False):
          said = as_person("ronoctua-video-metrics list" + (" --hidden" if hidden else ""))
          return [line.split("\t")[0] for line in said.splitlines() if line.strip()]

      def mind():
          return json.loads(as_person(f"{brain} look 2>/dev/null"))

      machine.start()
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit(f"user@{uid}.service")
      machine.wait_for_unit("fixture-feeds.service")
      machine.wait_for_open_port(${toString port})

      with subtest("the unit is what the module says"):
          unit = "ronoctua-video-metrics-scan.service"
          facts = dict(
              line.split("=", 1)
              for line in as_person(
                  f"systemctl --user show -p Type -p Nice -p IOSchedulingClass -p ExecStopPost {unit}"
              ).splitlines()
              if "=" in line
          )
          assert "scan-stopped" in facts.get("ExecStopPost", ""), facts
          assert facts.get("Type") == "oneshot", facts
          assert int(facts.get("Nice", "0")) > 0, facts
          # `idle` is class 3; systemd prints one or the other by version.
          assert facts.get("IOSchedulingClass") in ("idle", "3"), facts
          shown = as_person(f"systemctl --user show -p Environment {unit}")
          assert "RONOCTUA_VIDEO_METRICS_FEEDS=${feeds}" in shown, shown
          assert "RONOCTUA_YT_DLP=${ytDlp}" in shown, shown

      with subtest("adding a channel makes the catalogue and lets its hosts"):
          said = as_person("ronoctua-video-metrics add ${handle}")
          assert "added: ${channel}" in said, said
          machine.succeed(f"test -f {catalogue}")
          consent = json.loads(as_person("cat ~/.config/ronoctua/consent.json"))
          text = json.dumps(consent)
          for host in ("www.youtube.com", "i.ytimg.com"):
              assert host in text, f"{host} was not let: {text}"
          assert listed() == [], "a channel added listed videos before any scan"

      with subtest("the brain starts the unit, marks the modeline while it runs, and hears it end"):
          # WRITTEN BY ROOT, owned by the person: the folders on the way too,
          # or the consent the person keeps beside it could not be written.
          machine.succeed(
              f"install -d -o {user} -g users /home/{user}/.config /home/{user}/.config/ronoctua "
              f"&& install -o {user} -g users -m 644 ${brainPrefs} /home/{user}/.config/ronoctua/brain.json"
          )
          as_person("systemctl --user start ronoctua-brain.service")
          machine.wait_until_succeeds(
              f"su - {user} -c 'XDG_RUNTIME_DIR={run_dir} {brain} look' >/dev/null", timeout=60
          )
          assert "video-metrics" in [t["id"] for t in mind()["tasks"]]
          as_person(f"{brain} run video-metrics")
          # THE GATE MAY HOLD A MOMENT on a machine that just booted.
          # `is-active` exits 3 on `activating`, and the driver runs every
          # command under `pipefail`: without the `|| true` the pipe fails on
          # the very answer it waits for.
          machine.wait_until_succeeds(
              f"su - {user} -c 'XDG_RUNTIME_DIR={run_dir} systemctl --user is-active "
              "ronoctua-video-metrics-scan.service || true' | grep -qx activating",
              timeout=240,
          )
          marks = [m for m in mind()["marks"] if m["module"] == "video-metrics"]
          assert [m["what"]["id"]["name"] for m in marks] == ["scanning"], marks

      with subtest("a reader reads while the scan writes"):
          reads = 0
          while as_person(
              "systemctl --user is-active ronoctua-video-metrics-scan.service", check=False
          )[1].strip() == "activating":
              status, said = as_person("ronoctua-video-metrics list", check=False)
              assert status == 0, f"a reader was refused during the scan: {said}"
              reads += 1
              time.sleep(1)
          assert reads >= 3, f"the scan was over before it was read {reads} times"

      with subtest("the run is recorded done, and the mark is gone"):
          machine.wait_until_succeeds(
              f"su - {user} -c 'XDG_RUNTIME_DIR={run_dir} {brain} look' "
              "| jq -e '.tasks[] | select(.id == \"video-metrics\") | .last.kind == \"done\"'",
              timeout=120,
          )
          m = mind()
          assert not [x for x in m["marks"] if x["module"] == "video-metrics"], m["marks"]
          report = json.loads(as_person(f"cat {run_dir}/ronoctua-brain/video-metrics.json"))
          assert report["ended"] > 0 and report["trouble"] == "", report

      with subtest("the first scan took the newest ten of the last two weeks"):
          got = listed()
          assert len(got) == 10, f"the first scan catalogued {len(got)}: {got}"
          assert "fixtureshrt" in got, f"the Short, a day old, was left out: {got}"
          for older in ("fixture0009", "fixture0013"):
              assert older not in got, f"{older} is past the newest ten: {got}"

      with subtest("hide and delete from the command line"):
          as_person("ronoctua-video-metrics hide fixture0001")
          assert "fixture0001" not in listed(), "a hidden video is still listed"
          assert "fixture0001" in listed(hidden=True), "a hidden video left the catalogue"
          as_person("ronoctua-video-metrics delete fixture0002")
          assert "fixture0002" not in listed(hidden=True)
          status, said = as_person("ronoctua-video-metrics delete fixture0002", check=False)
          assert status == 1 and "not-found" in said, said
          # A SECOND SCAN OF THE SAME FEED, by hand: the deleted one stays away.
          as_person("ronoctua-video-metrics scan --feeds-only")
          assert "fixture0002" not in listed(hidden=True), "a deleted video came back"
          assert "fixture0003" in listed(), "a second scan lost what was there"

      with subtest("a scan killed before its end is no scan running, and is ended for the brain"):
          report_file = f"{run_dir}/ronoctua-brain/video-metrics.json"
          # A BEGUN REPORT NOBODY HOLDS THE CATALOGUE FOR — what the memory
          # killer leaves — read by a brain that starts after it.
          as_person("systemctl --user stop ronoctua-brain.service")
          machine.succeed(
              f"echo '{{\"began\": 100, \"ended\": 0, \"steps\": [], \"trouble\": \"\"}}' > {report_file} "
              f"&& chown {user} {report_file}"
          )
          as_person("systemctl --user start ronoctua-brain.service")
          machine.wait_until_succeeds(
              f"su - {user} -c 'XDG_RUNTIME_DIR={run_dir} {brain} look' >/dev/null", timeout=60
          )
          marks = [m for m in mind()["marks"] if m["module"] == "video-metrics"]
          assert marks == [], f"a killed scan's report marked the modeline: {marks}"
          # THE UNIT'S STOP writes the end the scan could not.
          as_person(f"SERVICE_RESULT=signal ronoctua-video-metrics scan-stopped --report {report_file}")
          report = json.loads(as_person(f"cat {report_file}"))
          assert report["ended"] > 0, report
          assert report["trouble"] == "stopped before it ended: signal", report
          # AND AN ENDED REPORT IS LEFT AS IT IS.
          as_person(f"SERVICE_RESULT=signal ronoctua-video-metrics scan-stopped --report {report_file}")
          assert json.loads(as_person(f"cat {report_file}")) == report, "an ended report was rewritten"
    '';
}
