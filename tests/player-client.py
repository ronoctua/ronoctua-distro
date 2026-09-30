# A client of the player's socket, written here and not in the crates: what a
# VM asks the daemon, and how it reads the pictures back, from OUTSIDE it.
#
# Shared by `tests/player-vm.nix` (which proves the chain) and `nix/shot.nix`
# (which photographs it), so the two ask the same way.
#
#   python3 player-client.py '<ask as JSON>'           one ask, its answer
#                                                      (`{"ask":"queue"}` is the
#                                                      queue's outline, which the
#                                                      state no longer carries)
#   python3 player-client.py frame <lane> [count]      what a frame measures
#   python3 player-client.py twice <lane>              two views, same bytes?
#   python3 player-client.py dump <lane> <out.ppm> [w h] one frame, as a picture
#   python3 player-client.py colours <in.ppm> <RRGGBB>... where each colour is
#                                                      in a picture: how many
#                                                      pixels, and the rows
#   python3 player-client.py quit                      what a watcher hears of a quit
#
# `<lane>` is JSON: `null` for the first picture shown, a lane's id for that
# lane's picture.
import glob, json, os, socket, sys, time


def path():
    found = glob.glob(os.environ.get("XDG_RUNTIME_DIR", "/run/user/0") + "/*player.sock")
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


def view(lane, width=320, height=180):
    """A preview of `lane`: the socket, the memfd, and the size it was given."""
    s = socket.socket(socket.AF_UNIX)
    # A PAUSED PICTURE SENDS ONE FRAME OR NONE: a deadline, so "none"
    # fails the test instead of hanging it.
    s.settimeout(10)
    s.connect(path())
    s.sendall((json.dumps({"ask": "preview", "width": width, "height": height, "lane": lane}) + "\n").encode())
    data, fds, _, _ = socket.recv_fds(s, 512, 1)
    head = json.loads(data.decode().strip())
    assert head["say"] == "preview", head
    return s, fds[0], head["width"], head["height"]


def slot_of(s):
    """The slot the next frame is in: one byte the daemon sends a frame.
    A view the daemon closed is SAID, not an IndexError on an empty read."""
    got = s.recv(1)
    if not got:
        raise EOFError("the player closed the view before sending a frame")
    return got[0]


def pixels(s, fd, w, h, count):
    """The RGBA of the `count`th frame announced: 16 bytes of header a slot."""
    slot = 0
    for _ in range(count):
        slot = slot_of(s)
    size = 16 + w * h * 4
    return os.pread(fd, w * h * 4, slot * size + 16)


def frame(lane, count=10):
    s, fd, w, h = view(lane)
    px = pixels(s, fd, w, h, count)
    s.close()
    os.close(fd)
    n = len(px) // 4
    lit = sum(1 for i in range(0, n, 97) if px[4*i] + px[4*i+1] + px[4*i+2] > 60) / len(range(0, n, 97))
    opaque = sum(1 for i in range(0, n, 97) if px[4*i+3] == 255) / len(range(0, n, 97))
    return {"w": w, "h": h, "lit": lit, "opaque": opaque}


def dump(lane, into, width=640, height=360, count=3):
    """One frame of `lane` written as a PPM: what the daemon drew, subtitles
    and all, with nothing of ours in between. `count` frames are let go by
    first, so a picture opened a moment ago has settled; a paused one sends
    one frame and no more, so the last one that came is the one kept."""
    s, fd, w, h = view(lane, width, height)
    slot = None
    try:
        for _ in range(count):
            slot = slot_of(s)
            # AFTER THE FIRST, a short wait: a paused picture's second never
            # comes, and ten seconds of nothing is not a better frame.
            s.settimeout(2)
    # A VIEW CLOSED OR QUIET after its first frame keeps that frame: it is
    # still in the memfd this process holds. Before the first, there is none.
    except (socket.timeout, EOFError):
        if slot is None:
            raise
    px = os.pread(fd, w * h * 4, slot * (16 + w * h * 4) + 16)
    s.close()
    os.close(fd)
    rgb = bytearray(w * h * 3)
    rgb[0::3] = px[0::4]
    rgb[1::3] = px[1::4]
    rgb[2::3] = px[2::4]
    with open(into, "wb") as out:
        out.write(b"P6\n%d %d\n255\n" % (w, h) + bytes(rgb))
    return {"w": w, "h": h}


def colours(path, wanted, fuzz=8):
    """Where each of `wanted` (hex, no `#`) is in the PPM at `path`: the
    pixels within `fuzz` of it on every channel, and the first and last row
    they are on. A small fuzz, and asked of a plain dark picture: a letter's
    inside is its colour exactly, and its rim and the ground are far from it."""
    data = open(path, "rb").read()
    head, size, _, px = data.split(b"\n", 3)
    w, h = map(int, size.split())
    found = {}
    for hex_ in wanted:
        c = [int(hex_[i:i + 2], 16) for i in (0, 2, 4)]
        count, rows = 0, []
        for y in range(h):
            line = px[y * w * 3:(y + 1) * w * 3]
            here = sum(
                1 for x in range(w)
                if abs(line[3 * x] - c[0]) <= fuzz
                and abs(line[3 * x + 1] - c[1]) <= fuzz
                and abs(line[3 * x + 2] - c[2]) <= fuzz
            )
            if here:
                count += here
                rows.append(y)
        found[hex_] = {"pixels": count, "rows": [rows[0], rows[-1]] if rows else None}
    return {"w": w, "h": h, "found": found}


def twice(lane):
    # TWO VIEWS AT ONCE, the second opened while the first is held.
    views = []
    for _ in range(2):
        s = socket.socket(socket.AF_UNIX)
        s.settimeout(10)
        s.connect(path())
        s.sendall((json.dumps({"ask": "preview", "width": 320, "height": 180, "lane": lane}) + "\n").encode())
        data, fds, _, _ = socket.recv_fds(s, 512, 1)
        head = json.loads(data.decode().strip())
        views.append((s, fds[0], head["width"], head["height"]))
    frames = []
    for s, fd, w, h in views:
        slot = slot_of(s)
        frames.append(os.pread(fd, w * h * 4, slot * (16 + w * h * 4) + 16))
    return "same" if frames[0] == frames[1] else "different"


def quit_heard():
    """A watcher, and then a quit on a connection of its own that does not
    wait for the answer, as a door sends it: every line the watcher heard
    until its stream ended — the outline and the state first, as every
    watch begins."""
    w = socket.socket(socket.AF_UNIX)
    w.settimeout(10)
    w.connect(path())
    w.sendall((json.dumps({"ask": "watch"}) + "\n").encode())
    lines = w.makefile()
    said = [json.loads(lines.readline())["say"]]
    q = socket.socket(socket.AF_UNIX)
    q.connect(path())
    q.sendall((json.dumps({"ask": "quit"}) + "\n").encode())
    q.close()
    for line in lines:
        said.append(json.loads(line)["say"])
    w.close()
    return {"heard": said, "last": said[-1]}


what = sys.argv[1]
if what == "quit":
    print(json.dumps(quit_heard()))
elif what == "twice":
    print(twice(json.loads(sys.argv[2])))
elif what == "frame":
    print(json.dumps(frame(json.loads(sys.argv[2]), *map(int, sys.argv[3:4]))))
elif what == "colours":
    print(json.dumps(colours(sys.argv[2], sys.argv[3:])))
elif what == "dump":
    print(json.dumps(dump(json.loads(sys.argv[2]), sys.argv[3], *map(int, sys.argv[4:6]))))
else:
    print(json.dumps(ask(json.loads(what))))
