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
# NO SOUND: the VM has no PipeWire session, and the recording is asked for with
# the microphone and the desktop off. Sound is proved on the host (a stereo AAC
# track, `ffprobe`); here a missing audio server must not stop the video.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  studio,
  fonts,
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
    };

  testScript = ''
    import json
    ${xvfb.prelude}

    def studio(what):
        return json.loads(machine.succeed(
            f"XDG_RUNTIME_DIR=/run/user/0 python3 ${client} '{json.dumps(what)}'"
        ))

    def readers():
        # Who has the "webcam" open — the question the modeline's mark asks —
        # leaving out the `testsrc` that plays the camera itself.
        out = machine.execute(
            "for p in /proc/[0-9]*; do "
            "if ls -l $p/fd 2>/dev/null | grep -q /dev/video10; then "
            "tr '\\0' ' ' < $p/cmdline; echo; fi; done"
        )[1]
        return [line for line in out.splitlines() if line.strip() and "testsrc2" not in line]

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("a camera nobody made for this distro"):
        machine.succeed("mkdir -p /run/user/0 /root/.config/ronoctua /root/out && chmod 700 /run/user/0")
        machine.succeed("cp ${studioJson} /root/.config/ronoctua/studio.json")
        machine.succeed(
            "systemd-run --unit=webcam ${pkgs.ffmpeg-full}/bin/ffmpeg -loglevel error -re "
            "-f lavfi -i testsrc2=size=640x480:rate=30 -pix_fmt yuyv422 -f v4l2 /dev/video10"
        )
        machine.wait_until_succeeds("test -e /dev/video10")
        machine.sleep(2)

    with subtest("the studio sees it, and sees ours as not a camera"):
        machine.succeed(
            "systemd-run --unit=studio --setenv=XDG_RUNTIME_DIR=/run/user/0 "
            "--setenv=HOME=/root --setenv=RUST_LOG=ronoctua_studio=debug "
            "--setenv=RONOCTUA_FFMPEG=${pkgs.ffmpeg-full}/bin/ffmpeg "
            "${studio}/bin/ronoctua-studio"
        )
        machine.wait_until_succeeds("ls /run/user/0 | grep -q studio", timeout=30)
        state = studio({"ask": "look"})
        print(state)
        cards = [c["card"] for c in state["cameras"]]
        assert "Webcam" in cards, f"the webcam was not seen: {state['cameras']}"
        assert state["camera"] is not None, "no default camera chosen"
        assert state["camera"].startswith("Webcam|"), (
            f"the default camera is not the webcam: {state['camera']}"
        )
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
        start_xvfb()
        start_compositor("compositor", "${scroll}")
        sock = socket_of("compositor")
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
  '';
}
