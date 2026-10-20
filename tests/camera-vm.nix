# The camera studio in a VM, with a camera that is not this laptop's.
#
# WHAT ONLY THIS PROVES
# ---------------------
# Every crate under the studio has its own tests over synthetic frames, and the
# host proofs used ONE camera — the owner's. What neither reaches is the whole
# chain on a machine that has never seen that camera:
#
#   a V4L2 node  ->  ronoctua-camera (ioctls, formats)  ->  ffmpeg  ->  the
#   studio's scene  ->  effects  ->  the preview memfd / the encoder / the
#   loopback  ->  another program reading it
#
# THE CAMERA IS A LOOPBACK FED BY `testsrc`. It announces capture, YUYV and one
# size, the way a cheap USB webcam does, and it is nobody's hardware: if the
# studio only worked for the node layout of one maker, it fails here.
#
# EVERY ASSERTION IS FROM OUTSIDE THE SCREEN. The preview is read from the
# memfd the studio hands over, by a client written here in Python rather than
# by our own window — our window reading our daemon would be the code agreeing
# with itself. The recording is read by `ffprobe`, the virtual camera by a
# second `ffmpeg`, and who holds the camera by `/proc`, which is what the
# modeline's mark reads.
#
# NO SOUND: the studio runs as root, outside the person's PipeWire session, and
# the recording is asked for with the microphone and the desktop off. Sound is
# proved on the host (a stereo AAC track, `ffprobe`); here a missing audio
# server must not stop the video.
#
# NO CAMERA OPENS BEFORE A PERSON ASKS FOR ONE. Opening a UVC node powers the
# camera up and its light blinks — and the footer and the studio, starting,
# each read every camera, so the light blinked twice at every start. The
# kernel's own answer is the witness: `inotifywait` watches `IN_OPEN` on both
# nodes while the studio and a footer start, and must see nothing until the
# camera's settings are opened, and then see the node opened. It runs as soon
# as the webcam is up, on this test's VM: no second boot, a footer more.
#
# NOR DOES THE PERSON'S LOGIN. The session's PipeWire is the distro's own
# (`base.nix`, with `studio.nix` for its cameras), and WirePlumber's V4L2 and
# libcamera monitors open every camera when it starts, to ask what each one
# is. The watch is up before any login is allowed, it is fanotify and not
# inotify because the question is WHO opened (the event carries the pid), and
# it reports by file handle so that watching never opens a camera itself. The
# person logs in — the one open allowed is udev's `v4l_id`, once per node for
# the seat's `change`, counted against udev's own events, and only those the
# kernel numbered after the watch was in place — their PipeWire and
# WirePlumber start, and the watch must see nothing more; then the same
# machine is switched to `pipewireCameras = true`, and
# within the same window WirePlumber opens the webcam and publishes it as a
# PipeWire node — so the switch switches, and the window is long enough to
# catch what it is there to catch.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  studio,
  fonts,
  ronoctua,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };

  # The client: one JSON line out, one back; the preview answer carries the
  # memfd as `SCM_RIGHTS`.
  client = pkgs.writeText "studio-client.py" ''
    import glob, json, os, socket, sys, time

    def path():
        found = glob.glob("/run/user/0/*studio*.sock")
        if not found:
            sys.exit("no studio socket")
        return found[0]

    def ask(what):
        s = socket.socket(socket.AF_UNIX)
        s.connect(path())
        s.sendall((json.dumps(what) + "\n").encode())
        line = s.makefile().readline()
        s.close()
        return json.loads(line)

    def black(pixels):
        # The program before any source has given a frame: RGB 0, opaque.
        n = len(pixels) // 4
        step = max(1, n // 20000)
        return not any(
            pixels[4 * i] or pixels[4 * i + 1] or pixels[4 * i + 2]
            for i in range(0, n, step)
        )

    def frame():
        s = socket.socket(socket.AF_UNIX)
        s.settimeout(20)
        s.connect(path())
        s.sendall(b'{"ask":"preview"}\n')
        data, fds, _, _ = socket.recv_fds(s, 512, 1)
        head = json.loads(data.decode().strip())
        assert head["say"] == "preview", head
        w, h = head["width"], head["height"]
        fd = fds[0]
        size = 16 + w * h * 4
        read = lambda slot: os.pread(fd, w * h * 4, slot * size + 16)
        # THE PROGRAM COMES BEFORE THE CAMERA. The studio composes from the
        # moment a viewer asks, and the camera opens at that same moment: until
        # its ffmpeg is spawned, has opened the device and has given a frame,
        # the program is the empty canvas, black. How long that takes is the
        # machine's, not the studio's — a fixed ten frames (a third of a
        # second) was enough on a quiet host, and a busy one handed over ten
        # black frames. So the frames are read until the camera's first one is
        # in the program, for at most ten seconds, and the one judged comes a
        # few frames later. Waiting asks only whether ANYTHING arrived; whether
        # it is the picture is still the caller's assertion, and a camera that
        # never arrives leaves the last frame black for it to refuse.
        waited = 0
        deadline = time.monotonic() + 10
        slot = s.recv(1)[0]
        while black(read(slot)) and time.monotonic() < deadline:
            slot = s.recv(1)[0]
            waited += 1
        for _ in range(3):
            slot = s.recv(1)[0]
        pixels = read(slot)
        s.close()
        os.close(fd)
        return w, h, waited, pixels

    def stats(pixels):
        n = len(pixels) // 4
        step = max(1, n // 20000)
        r = g = b = spread = lit = 0
        count = 0
        for i in range(0, n, step):
            pr, pg, pb = pixels[4 * i], pixels[4 * i + 1], pixels[4 * i + 2]
            r += pr; g += pg; b += pb
            spread += max(pr, pg, pb) - min(pr, pg, pb)
            lit += 1 if pr + pg + pb > 60 else 0
            count += 1
        return {"r": r / count, "g": g / count, "b": b / count,
                "chroma": spread / count, "lit": lit / count}

    what = sys.argv[1]
    if what == "frame":
        w, h, waited, pixels = frame()
        print(json.dumps({"w": w, "h": h, "black_frames": waited, **stats(pixels)}))
    else:
        print(json.dumps(ask(json.loads(what))))
  '';

  # WHO OPENS A CAMERA NODE: one fanotify group per node, so the event names
  # its node without decoding a handle, and FAN_REPORT_FID, so no event
  # carries a descriptor — a descriptor would be the watch opening the camera.
  # One JSON line per event: the node, open and/or close, the pid and what
  # that pid is. Ready (sd_notify) only once every mark is in place, and then
  # it writes the kernel's uevent count: a uevent numbered past it was emitted
  # with the marks in place, so whatever udev opened for it, the watch saw.
  watch = pkgs.writeText "camera-opens.py" ''
    import ctypes, json, os, select, socket, struct, sys, time

    libc = ctypes.CDLL(None, use_errno=True)
    libc.fanotify_init.argtypes = [ctypes.c_uint, ctypes.c_uint]
    libc.fanotify_mark.argtypes = [
        ctypes.c_int, ctypes.c_uint, ctypes.c_uint64, ctypes.c_int, ctypes.c_char_p,
    ]
    FAN_CLOEXEC, FAN_REPORT_FID, FAN_MARK_ADD = 0x1, 0x200, 0x1
    FAN_CLOSE_WRITE, FAN_CLOSE_NOWRITE, FAN_OPEN = 0x8, 0x10, 0x20
    AT_FDCWD = -100

    out = open(sys.argv[1], "a", buffering=1)
    groups = {}
    for path in sys.argv[3:]:
        fd = libc.fanotify_init(FAN_CLOEXEC | FAN_REPORT_FID, os.O_RDONLY)
        if fd < 0:
            raise OSError(ctypes.get_errno(), "fanotify_init")
        mask = FAN_OPEN | FAN_CLOSE_WRITE | FAN_CLOSE_NOWRITE
        if libc.fanotify_mark(fd, FAN_MARK_ADD, mask, AT_FDCWD, path.encode()) < 0:
            raise OSError(ctypes.get_errno(), "fanotify_mark " + path)
        groups[fd] = path

    def proc(pid, name):
        try:
            with open(f"/proc/{pid}/{name}", "rb") as f:
                return f.read().replace(b"\0", b" ").decode(errors="replace").strip()
        except OSError:
            return None

    def about(pid):
        status = proc(pid, "status") or ""
        uid = [line.split()[1] for line in status.splitlines() if line.startswith("Uid:")]
        return {"comm": proc(pid, "comm"), "cmdline": proc(pid, "cmdline"),
                "uid": uid[0] if uid else None}

    with open("/sys/kernel/uevent_seqnum") as f:
        born = int(f.read())
    with open(sys.argv[2], "w") as f:
        f.write(f"{born}\n")

    notify = os.environ.get("NOTIFY_SOCKET")
    if notify:
        address = "\0" + notify[1:] if notify.startswith("@") else notify
        socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM).sendto(b"READY=1", address)

    while True:
        ready, _, _ = select.select(list(groups), [], [])
        for fd in ready:
            buf = os.read(fd, 65536)
            at = round(time.time(), 3)
            off = 0
            while off + 24 <= len(buf):
                length, _, _, _, mask, _, pid = struct.unpack_from("=IBBHQii", buf, off)
                what = []
                if mask & FAN_OPEN:
                    what.append("open")
                if mask & (FAN_CLOSE_WRITE | FAN_CLOSE_NOWRITE):
                    what.append("close")
                event = {"at": at, "node": groups[fd], "what": what, "pid": pid}
                out.write(json.dumps({**event, **about(pid)}) + "\n")
                off += length
  '';

  # A studio file with nothing that needs an audio server.
  studioJson = pkgs.writeText "studio.json" (
    builtins.toJSON {
      microphone = false;
      desktop = false;
      pictures = "/root/out";
      videos = "/root/out";
      container = "mkv";
      encoder = "software";
    }
  );
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-camera";

  nodes.machine =
    { config, ... }:
    {
      # THE PERSON'S SESSION IS THE DISTRO'S: `base.nix` declares the user and
      # PipeWire, `studio.nix` what WirePlumber does with cameras. The studio's
      # own unit stays off — this test runs the daemon by hand, as root, and
      # loads its own two loopbacks below.
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/studio.nix
      ];
      _module.args.ronoctua = ronoctua;
      ronoctua.studio.enable = false;
      networking.hostName = lib.mkForce "noctua-camera";
      users.users.root.hashedPassword = lib.mkForce null;
      # A LOGIN AT THE SEAT, as the machine boots into one: the person's
      # manager starts at boot, after the watch below.
      services.getty.autologinUser = config.ronoctua.user;
      # THE ONE INSTRUMENT ON WIREPLUMBER: its component loader says, at
      # debug, every component it loads — what the two states are compared
      # by. Everything else logs at the default level.
      systemd.user.services.wireplumber.environment.WIREPLUMBER_DEBUG = "2,wp-internal-comp-loader:D";

      # THE SAME MACHINE WITH THE MONITORS ON, switched to at the end.
      specialisation.pipewire-cameras.configuration.ronoctua.camera.pipewireCameras = true;

      virtualisation.memorySize = 3072;
      virtualisation.cores = 4;

      # TWO loopbacks: node 10 is the "webcam" `testsrc` writes into, node 42
      # is OURS — the label is how the studio tells the two apart, and it is
      # the same line `nix/modules/studio.nix` writes, with a second device.
      # NO QUOTES in a list: modprobe keeps them inside the labels, and the
      # first run of this test saw a camera called `Test camera"`.
      boot.extraModulePackages = [ config.boot.kernelPackages.v4l2loopback ];
      boot.kernelModules = [ "v4l2loopback" ];
      boot.extraModprobeConfig = ''
        options v4l2loopback devices=2 video_nr=10,42 card_label=Webcam,ronoctua exclusive_caps=1,1
      '';

      environment.systemPackages = [
        scroll
        ronoctua-shell
        pkgs.ffmpeg-full
        pkgs.python3
        pkgs.imagemagick
        pkgs.v4l-utils
      ]
      ++ xvfb.packages;

      fonts.enableDefaultPackages = false;
      fonts.packages = [ fonts.noctua-iosevka-core ];
      hardware.graphics.enable = true;
      services.dbus.enable = true;

      # THE WEBCAM IS PLUGGED IN BEFORE ANYBODY LOGS IN, the way a laptop's
      # is: the writer gives node 10 its capture side (`exclusive_caps`), so
      # what WirePlumber would find at login is a camera, not an output.
      systemd.services.webcam = {
        wantedBy = [ "multi-user.target" ];
        after = [ "systemd-modules-load.service" ];
        serviceConfig.ExecStart =
          "${pkgs.ffmpeg-full}/bin/ffmpeg -loglevel error -re "
          + "-f lavfi -i testsrc2=size=640x480:rate=30 -pix_fmt yuyv422 -f v4l2 /dev/video10";
      };

      # THE WATCH IS UP BEFORE ANY LOGIN IS ALLOWED: ready only once both
      # marks are in place, and `systemd-user-sessions` (the gate every login
      # waits for) is ordered after it. It starts once the writer holds its
      # node and udev has settled, so neither of those is counted, and once
      # udev's events are being logged, so no event numbered past the watch's
      # birth goes unlogged.
      systemd.services.login-opens = {
        wantedBy = [ "multi-user.target" ];
        after = [
          "webcam.service"
          "login-uevents.service"
        ];
        before = [ "systemd-user-sessions.service" ];
        path = [
          pkgs.coreutils
          pkgs.gnugrep
          config.systemd.package
        ];
        preStart = ''
          for _ in $(seq 300); do
            if ls -l /proc/[0-9]*/fd 2>/dev/null | grep -q /dev/video10; then break; fi
            sleep 0.1
          done
          udevadm settle
        '';
        script = "exec ${pkgs.python3}/bin/python3 -u ${watch} /tmp/login-opens.jsonl /tmp/login-opens.born /dev/video10 /dev/video42";
        serviceConfig = {
          Type = "notify";
          NotifyAccess = "main";
        };
      };

      # WHAT UDEV DOES TO THE NODES MEANWHILE. As logind opens the session,
      # udev processes a `change` on every video node (measured: at the
      # login, once per node), runs its rules again, and
      # `60-persistent-v4l.rules` runs `v4l_id`, which opens the node for a
      # `QUERYCAP`. That open is udev's, not the session's, and this log is
      # how the test tells the two apart: every event with its `SEQNUM`, and
      # only one numbered past the watch's birth may excuse an open — an
      # earlier one (the boot's `add`, still in udev's hands) may have opened
      # its node before the marks were placed. Ready only once it listens
      # (udevadm says its header after binding), and the watch is born after.
      systemd.services.login-uevents = {
        wantedBy = [ "multi-user.target" ];
        before = [ "login-opens.service" ];
        path = [
          pkgs.gnugrep
          config.systemd.package
        ];
        script = ''
          udevadm monitor --udev --property --subsystem-match=video4linux > /tmp/login-uevents &
          until grep -q '^UDEV - ' /tmp/login-uevents; do sleep 0.05; done
          systemd-notify --ready
          wait
        '';
        serviceConfig = {
          Type = "notify";
          NotifyAccess = "all";
        };
      };
    };

  testScript = ''
    import json
    import re
    ${xvfb.prelude}

    def studio(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def opened():
        # Every `IN_OPEN` of a camera node since the watch began, as
        # `inotifywait` wrote them.
        return machine.succeed("cat /tmp/opens").strip()

    def readers():
        # Who has the "webcam" open — the question the modeline's mark asks —
        # leaving out the `testsrc` that plays the camera itself.
        out = machine.execute(
            "for p in /proc/[0-9]*; do "
            "if ls -l $p/fd 2>/dev/null | grep -q /dev/video10; then "
            "tr '\\0' ' ' < $p/cmdline; echo; fi; done"
        )[1]
        return [line for line in out.splitlines() if line.strip() and "testsrc2" not in line]

    def login_opens():
        # Every open or close of a camera node since before the login, with
        # who did it, as the fanotify watch wrote them.
        out = machine.succeed("cat /tmp/login-opens.jsonl 2>/dev/null || true")
        return [json.loads(line) for line in out.splitlines() if line.strip()]

    def udev_events():
        # udev's events on the video nodes, as the monitor wrote them: a
        # `UDEV  [time] action devpath` line, then `KEY=VALUE` lines.
        events, event = [], None
        for line in machine.succeed("cat /tmp/login-uevents").splitlines():
            if line.startswith("UDEV  ["):
                event = {}
                events.append(event)
            elif event is not None and "=" in line:
                key, _, value = line.partition("=")
                event[key] = value
            else:
                event = None
        return events

    def ran_after(events, born, node):
        # The events udev ran its rules for on `node` — the rules that open it
        # for `v4l_id` run on `add` and `change` — numbered past `born`. One
        # without a number is not counted: it can excuse nothing.
        return [
            e for e in events
            if e.get("DEVNAME") == node and e.get("ACTION") in ("add", "change")
            and e.get("SEQNUM", "").isdigit() and int(e["SEQNUM"]) > born
        ]

    # THE WINDOW in which WirePlumber, once up, would have opened the cameras:
    # with the monitors on, the first open lands well inside it (the last
    # subtest asserts exactly that), so an empty window with them off is an
    # answer and not an early look.
    WINDOW = 10

    def as_person(command):
        # In the person's manager, as a session's program runs. By path: the
        # manager's `PATH` has no profile in it (a bare name exits 203).
        return machine.succeed(
            f"systemd-run --machine={person}@ --user --pipe --wait --quiet /run/current-system/sw/bin/{command}"
        )

    def camera_nodes():
        # The PipeWire nodes WirePlumber made of a V4L2 or libcamera device.
        nodes = []
        for o in json.loads(as_person("pw-dump")):
            props = (o.get("info") or {}).get("props") or {}
            camera = props.get("device.api") in ("v4l2", "libcamera") or any(
                k.startswith(("api.v4l2.", "api.libcamera.")) for k in props
            )
            if o.get("type") == "PipeWire:Interface:Node" and camera:
                nodes.append({k: props.get(k) for k in ("node.name", "media.class", "api.v4l2.path")})
        return nodes

    def spa_plugins():
        # The SPA plugins the person's WirePlumber has loaded: every monitor
        # loads its own as it starts, and one that is off never does.
        pid = machine.succeed(f"pgrep -u {person} -x wireplumber").strip()
        maps = machine.succeed(f"cat /proc/{pid}/maps")
        return set(re.findall(r"libspa-[a-z0-9]+", maps))

    CAMERA_PLUGINS = {"libspa-v4l2", "libspa-libcamera"}

    def components():
        # Every component the person's running WirePlumber loaded, as its
        # loader said it (the one debug topic the node turns on).
        pid = machine.succeed(f"pgrep -u {person} -x wireplumber").strip()
        said = machine.succeed(f"journalctl -b _PID={pid} --no-pager -o cat")
        return set(re.findall(r"loading component '([^']+)'", said))

    def camera_component(name):
        return any(word in name for word in ("v4l2", "libcamera", "video-capture"))

    def start_wireplumber(verb):
        # What the session's first audio client does: PipeWire is socket
        # activated, and WirePlumber is wanted by it.
        machine.succeed(f"systemctl --machine={person}@ --user {verb} pipewire.service wireplumber.service")
        machine.wait_until_succeeds(
            f"systemctl --machine={person}@ --user is-active wireplumber.service", timeout=60
        )
        machine.wait_until_succeeds(
            f"systemd-run --machine={person}@ --user --pipe --wait --quiet /run/current-system/sw/bin/wpctl status",
            timeout=60,
        )
        machine.sleep(WINDOW)

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("a camera nobody made for this distro"):
        machine.succeed("mkdir -p /run/user/0 /root/.config/ronoctua /root/out && chmod 700 /run/user/0")
        machine.succeed("cp ${studioJson} /root/.config/ronoctua/studio.json")
        machine.wait_for_unit("webcam.service")
        machine.wait_for_unit("login-opens.service")

    with subtest("nothing opens a camera at login"):
        # The person, from the seat's login and not from a name written here.
        machine.wait_until_succeeds("loginctl list-users --no-legend | grep -q .", timeout=120)
        person = machine.succeed("loginctl list-users --no-legend | awk 'NR == 1 { print $2 }'").strip()
        machine.wait_until_succeeds(
            f"systemctl --machine={person}@ --user is-system-running --wait | grep -E 'running|degraded'",
            timeout=120,
        )
        machine.succeed("udevadm settle")
        # THE LOGIN: what opened a node between the watch and a settled seat.
        # Only udev may have, once per event it ran its rules for — the
        # `v4l_id` that reads the capabilities — and only per event the kernel
        # numbered past the watch's birth: an earlier one may have opened its
        # node before the marks, and would excuse an open it did not do. An
        # open whose process was gone before the watch read `/proc` (no comm,
        # no uid) is excused by those events and by nothing else. A process
        # of the person, or one still alive (a daemon), is the session opening
        # a camera.
        at_login = login_opens()
        born = int(machine.succeed("cat /tmp/login-opens.born"))
        events = udev_events()
        print(json.dumps(at_login, indent=1))
        print("the watch was born after uevent", born)
        print([(e.get("SEQNUM"), e.get("ACTION"), e.get("DEVNAME")) for e in events])
        for node in ("/dev/video10", "/dev/video42"):
            opens = [e for e in at_login if e["node"] == node and "open" in e["what"]]
            ran = ran_after(events, born, node)
            assert len(opens) <= len(ran), (
                f"{node} opened {len(opens)} times at login, and udev ran its rules for it "
                f"{len(ran)} times after the watch was in place: {opens}"
            )
        strangers = [e for e in at_login if e["comm"] not in ("v4l_id", None) or e["uid"] not in ("0", None)]
        assert strangers == [], f"the login opened a camera, and not udev: {strangers}"
        # THE SESSION'S AUDIO, which starts on its first client: nothing at all.
        before = len(at_login)
        start_wireplumber("start")
        seen = login_opens()[before:]
        print(json.dumps(seen, indent=1))
        assert seen == [], f"the person's PipeWire opened a camera with nobody asking: {seen}"
        # Neither monitor loaded its plugin, and libcamera's camera manager —
        # which logs its version as it starts — never started.
        plugins = spa_plugins()
        print(sorted(plugins))
        assert not plugins & CAMERA_PLUGINS, f"WirePlumber loaded a camera plugin: {sorted(plugins)}"
        # AND THE REST OF THE PROFILE STAYED: the fragment is merged into
        # `main`, not put in its place — an empty `main` would be a session
        # with no sound and no camera either. Its sound and Bluetooth
        # monitors are two of what `main` requires.
        assert {"libspa-alsa", "libspa-bluez5"} <= plugins, f"WirePlumber's main profile is gone: {sorted(plugins)}"
        loaded = components()
        print(len(loaded), sorted(loaded))
        assert not [c for c in loaded if camera_component(c)], f"a camera component loaded: {sorted(loaded)}"
        assert any(c.startswith("policy.standard") for c in loaded), f"no policy loaded: {sorted(loaded)}"
        plugins_off, components_off = plugins, loaded
        journal = machine.succeed(f"journalctl -b _UID=$(id -u {person}) --no-pager")
        assert "libcamera v" not in journal, "the libcamera monitor started at login"
        assert camera_nodes() == [], f"a camera is a PipeWire node with the monitors off: {camera_nodes()}"
        print(as_person("wpctl status"))

    with subtest("nothing opens a camera before somebody asks for one"):
        # THE WATCH FIRST, after the `testsrc` writer opened its node: from
        # here on, every open of either node is somebody reading a camera.
        machine.succeed(
            "systemd-run --unit=opens ${pkgs.inotify-tools}/bin/inotifywait -m -e open "
            "--format '%w' -o /tmp/opens /dev/video10 /dev/video42"
        )
        machine.wait_until_succeeds(
            "journalctl -u opens --no-pager | grep -q 'Watches established'", timeout=30
        )
        machine.succeed(
            "systemd-run --unit=studio --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=RUST_LOG=ronoctua_studio=debug "
            "--setenv=RONOCTUA_FFMPEG=${pkgs.ffmpeg-full}/bin/ffmpeg "
            "${studio}/bin/ronoctua-studio"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q studio", timeout=30)
        start_xvfb()
        start_compositor("compositor", "${scroll}")
        sock = socket_of("compositor")
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            f"--setenv=HOME=/root --setenv=SCROLLSOCK={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=info "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'footer layout resolved'", timeout=60
        )
        # The footer watches the studio, and the studio is asked for its
        # state: neither is asked for a camera.
        state = studio({"ask": "look"})
        assert state["cameras"] == [], f"an idle studio read the cameras: {state['cameras']}"
        # Both read every camera within a second of starting, before.
        machine.sleep(3)
        assert opened() == "", f"a camera was opened with nobody asking: {opened()}"
        # THE GESTURE: the camera's settings, which show the cameras.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            f"SCROLLSOCK={sock} ${ronoctua-shell}/bin/ronoctua-shell command camera-settings-open"
        )
        machine.wait_until_succeeds("grep -q /dev/video10 /tmp/opens", timeout=30)
        machine.succeed("systemctl stop footer")

    with subtest("at rest the studio holds nothing, and reads no camera"):
        state = studio({"ask": "look"})
        print(state)
        assert state["cameras"] == [], f"at rest, the studio keeps a camera list: {state}"
        # At rest nobody holds the camera: the light stays off.
        assert readers() == [], f"at rest, the camera is held: {readers()}"

    with subtest("the preview arrives through the memfd, with the picture in it"):
        before = json.loads(machine.succeed("XDG_RUNTIME_DIR=/run/user/0 python3 ${client} frame"))
        print(before)
        assert before["lit"] > 0.3, f"the preview is dark: {before}"
        assert before["chroma"] > 20, f"testsrc2 is colourful and the preview is not: {before}"

    with subtest("an effect changes the pixels, live"):
        said = studio({"ask": "effect-turn", "by": 1})
        assert said["say"] == "done", said
        machine.sleep(1)
        grey = json.loads(machine.succeed("XDG_RUNTIME_DIR=/run/user/0 python3 ${client} frame"))
        print(grey)
        assert grey["chroma"] < before["chroma"] / 4, f"the grey effect left the colour: {grey}"
        studio({"ask": "effect-turn", "by": -1})

    with subtest("a recording, and a scene switched under it keeps the encoder"):
        said = studio({"ask": "record", "on": True})
        assert said["say"] == "done", said
        machine.wait_until_succeeds("pgrep -f 'ffmpeg.*matroska' >/dev/null", timeout=20)
        encoder = machine.succeed("pgrep -f 'ffmpeg.*matroska' | head -1").strip()
        # The source opens beside the encoder, not before it: waited for.
        for _ in range(20):
            if readers():
                break
            machine.sleep(1)
        assert readers(), "recording, and nobody holds the camera"
        # WITH A CAMERA IN USE the studio reads them: it sees the webcam, and
        # sees ours as not a camera. The watch's first reading is its own
        # thread's, a moment after the camera opened.
        for _ in range(20):
            if any(c["card"] == "Webcam" for c in studio({"ask": "look"})["cameras"]):
                break
            machine.sleep(1)
        state = studio({"ask": "look"})
        print(state)
        cards = [c["card"] for c in state["cameras"]]
        assert "Webcam" in cards, f"the webcam was not seen: {state['cameras']}"
        assert "ronoctua" not in cards, f"our own output was listed as a camera: {cards}"
        assert state["camera"] is not None, "no default camera chosen"
        assert state["camera"].startswith("Webcam|"), (
            f"the default camera is not the webcam: {state['camera']}"
        )
        # A photo while recording: the same program, one frame of it.
        said = studio({"ask": "photo"})
        assert said["say"] == "done", said
        machine.wait_until_succeeds("ls /root/out/*.png", timeout=20)
        machine.succeed("${pkgs.imagemagick}/bin/identify /root/out/*.png")
        machine.sleep(2)
        # The scene with the camera over a colour, then back: the program
        # changes, the encoder does not.
        for scene in ["screen-and-camera", "camera"]:
            said = studio({"ask": "scene", "name": scene})
            print(scene, said)
            machine.sleep(1)
        after = machine.succeed("pgrep -f 'ffmpeg.*matroska' | head -1").strip()
        assert after == encoder, f"switching scenes restarted the encoder ({encoder} -> {after})"
        said = studio({"ask": "record", "on": False})
        assert said["say"] == "done", said
        machine.wait_until_succeeds("ls /root/out/*.mkv", timeout=30)
        probe = machine.succeed(
            "${pkgs.ffmpeg-full}/bin/ffprobe -v error -show_entries stream=codec_type,codec_name "
            "-show_entries format=duration -of json /root/out/*.mkv"
        )
        print(probe)
        streams = json.loads(probe)
        assert any(s["codec_type"] == "video" for s in streams["streams"]), probe
        assert float(streams["format"]["duration"]) >= 3.0, probe

    with subtest("the virtual camera is a camera another program can read"):
        said = studio({"ask": "virtual", "on": True})
        assert said["say"] == "done", said
        # THE SECOND READER IS `v4l2-ctl`, a program that is not ours: what a
        # browser does is open the node and take buffers, which is this.
        # (An ffmpeg with the VM's console on its stdin stalled before it
        # opened anything, which proved nothing about the camera.)
        try:
            machine.wait_until_succeeds(
                # A SECOND OF FRAMES, and the LAST one is judged: switching
                # the virtual camera on reopens the camera, and until its first
                # frame arrives the program is an empty, black canvas.
                "timeout 10 v4l2-ctl -d /dev/video42 --stream-mmap --stream-count=30 "
                "--stream-to=/tmp/raw && test -s /tmp/raw",
                timeout=60,
            )
        except Exception:
            print(machine.execute("journalctl -u studio --no-pager | tail -40")[1])
            print(machine.execute("cat /tmp/ronoctua-studio-virtual-*.log")[1])
            raise
        fmt = machine.succeed("cat /sys/class/video4linux/video42/format").strip()
        print(f"the virtual camera says {fmt}")
        size = fmt.split(":")[1].split("@")[0]
        fw, fh = (int(n) for n in size.split("x"))
        machine.succeed(f"tail -c {fw * fh * 2} /tmp/raw > /tmp/last.raw")
        machine.succeed(
            "${pkgs.ffmpeg-full}/bin/ffmpeg -nostdin -loglevel error -y -f rawvideo "
            f"-pix_fmt yuyv422 -video_size {size} -i /tmp/last.raw -frames:v 1 /tmp/other.png"
        )
        mean = float(machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/other.png -format '%[fx:mean]' info:"
        ))
        assert mean > 0.1, f"the other program read a black camera: {mean}"
        studio({"ask": "virtual", "on": False})

    with subtest("the app shows the camera in a window of its own"):
        machine.succeed(
            "systemd-run --unit=app ${xvfb.env} --setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=HOME=/root "
            "${ronoctua-shell}/bin/ronoctua-shell camera"
        )
        await_windows(sock, 1, timeout=60)
        machine.sleep(3)
        w, h, x, y = compositor_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/screen.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/screen.xwd -crop {w}x{h}+{x}+{y} +repage /tmp/app.png"
        )
        spread = float(machine.succeed(
            "${pkgs.imagemagick}/bin/magick /tmp/app.png -colorspace HSL -channel G -separate "
            "-format '%[fx:mean]' info:"
        ))
        print(f"saturation: {spread}")
        assert spread > 0.1, "the app's window has no colour: the picture did not reach it"
        machine.succeed("systemctl stop app")

    with subtest("with nothing watching, the camera is let go"):
        for _ in range(20):
            if readers() == []:
                break
            machine.sleep(1)
        assert readers() == [], f"nobody is watching, and the camera is still held: {readers()}"
        journal = machine.succeed("journalctl -u studio --no-pager")
        assert "panicked" not in journal, journal

    with subtest("with pipewireCameras on, WirePlumber opens the camera and publishes it"):
        # LAST, because from here on the person's WirePlumber holds a say over
        # the webcam. The same machine with the one option flipped.
        machine.succeed("/run/current-system/specialisation/pipewire-cameras/bin/switch-to-configuration test")
        machine.succeed(f"systemctl --machine={person}@ --user daemon-reload")
        before = len(login_opens())
        start_wireplumber("restart")
        seen = login_opens()[before:]
        print(json.dumps(seen, indent=1))
        by_wireplumber = [e for e in seen if e["node"] == "/dev/video10" and "open" in e["what"]
                          and e["comm"] == "wireplumber"]
        assert by_wireplumber, f"with the monitors on, WirePlumber did not open the webcam: {seen}"
        plugins = spa_plugins()
        print(sorted(plugins))
        # THE SWITCH SWITCHES THE CAMERAS AND NOTHING ELSE: the two plugins and
        # the camera components are the whole difference.
        assert plugins - plugins_off == CAMERA_PLUGINS, (
            f"on {sorted(plugins)} against off {sorted(plugins_off)}"
        )
        assert plugins_off <= plugins, f"on {sorted(plugins)} against off {sorted(plugins_off)}"
        loaded = components()
        print(len(loaded), sorted(loaded - components_off))
        assert components_off <= loaded, f"on, these did not load: {sorted(components_off - loaded)}"
        added = loaded - components_off
        assert added and all(camera_component(c) for c in added), f"on, these loaded too: {sorted(added)}"
        nodes = camera_nodes()
        print(nodes)
        assert any(n["api.v4l2.path"] == "/dev/video10" for n in nodes), (
            f"with the monitors on, the webcam is not a PipeWire node: {nodes}"
        )
  '';
}
