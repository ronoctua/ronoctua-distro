# A pendrive's own trash — what only a SECOND filesystem answers.
#
# WHAT THIS PROVES
# ----------------
# Deleting a file on a stick cannot go to the home's trash: `rename` fails
# with `EXDEV` between filesystems, and copying instead would turn "to the
# trash" into gigabytes of I/O behind a gesture that promises to be instant.
# So the manager hands the queue the VOLUME's own bin, `$topdir/.Trash-$uid`
# (`ronoctua_files::trash::Bin::of_volume`), and the queue puts the file
# there — on the stick, in one `rename`.
#
# Every unit test of that runs inside one filesystem, so none of them can say
# the rename stays on the stick, that a vfat volume (no owners, no modes)
# takes the bin at all, or that the note's RELATIVE `Path=` brings the file
# back after the stick was mounted somewhere else. This does, with a real
# vfat on a second disk and the queue's own daemon, spoken to over its socket
# the way the manager speaks to it.
#
# And that the trash's expiry (`ronoctua-empty-trash --media`) finds the stick
# as a mount under the media folder and empties ITS trash too — which is what
# makes the manager's "the space stays taken until that trash lets it go" true.
#
# WHAT IT DOES NOT
# ----------------
# The manager's listing that gathers the bins and the sentence on unmounting
# are unit tests (`files::tests::the_trash_lists_every_mounted_volumes_own_
# trash_too`, `unmounting_with_a_full_volume_trash_says_so`): they need no
# second filesystem.
{
  pkgs,
  # The queue's daemon, extracted (`nix/rust.nix`): the process the manager
  # hands a trash to.
  transfers,
  # The trash's expiry, extracted: what empties a stick's own trash too.
  emptyTrash,
}:

let
  # ONE JSON LINE to the queue's socket, written whole — what
  # `ronoctua_transfers::client::tell` does — and one line of answer read.
  ask = pkgs.writeText "ask-the-queue.py" ''
    import os, socket, sys
    s = socket.socket(socket.AF_UNIX)
    s.connect(os.environ["XDG_RUNTIME_DIR"] + "/ronoctua-transfers.sock")
    s.sendall(sys.argv[1].encode() + b"\n")
    s.recv(65536)
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-trash-volume";

  nodes.machine = {
    virtualisation.memorySize = 1024;
    # THE STICK: a disk of its own, formatted vfat below.
    virtualisation.emptyDiskImages = [ 64 ];
    users.users.ana = {
      isNormalUser = true;
      uid = 1000;
    };
    environment.systemPackages = [
      pkgs.dosfstools
      pkgs.python3
    ];
  };

  testScript = ''
    import json

    machine.start()
    machine.wait_for_unit("multi-user.target")

    RUN = "/run/user/1000"
    SOCKET = RUN + "/ronoctua-transfers.sock"

    def as_ana(command):
        return machine.succeed(
            f"su ana -s /bin/sh -c 'XDG_RUNTIME_DIR={RUN} {command}'"
        )

    def ask(line):
        # THE MANAGER'S OWN REQUEST, as the person, through `ask-the-queue.py`.
        payload = json.dumps(line).replace('"', '\\"')
        machine.succeed(
            f"su ana -s /bin/sh -c \"XDG_RUNTIME_DIR={RUN} python3 ${ask} '{payload}'\""
        )

    with subtest("a vfat stick, mounted for the person the way udisks mounts one"):
        machine.succeed("mkfs.vfat -n STICK /dev/vdb")
        machine.succeed("mkdir -p /run/media/ana/STICK /run/user/1000")
        machine.succeed("chown ana /run/user/1000 && chmod 700 /run/user/1000")
        machine.succeed("mount -o uid=1000,gid=100,umask=077 /dev/vdb /run/media/ana/STICK")
        as_ana("mkdir -p /run/media/ana/STICK/fotos && echo praia > /run/media/ana/STICK/fotos/praia.jpg")

    with subtest("the queue's daemon, as the person"):
        machine.succeed(
            f"su ana -s /bin/sh -c 'XDG_RUNTIME_DIR={RUN} setsid ${transfers}/bin/ronoctua-transfers "
            "> /tmp/transfers.log 2>&1 &'"
        )
        machine.wait_until_succeeds(f"test -S {SOCKET}")

    with subtest("thrown away on the stick, it lands in the STICK's own trash"):
        ask({
            "ask": "add",
            "work": {
                "work": "trash",
                "root": "/run/media/ana/STICK/.Trash-1000",
                "paths": ["/run/media/ana/STICK/fotos/praia.jpg"],
            },
        })
        machine.wait_until_succeeds("test -f /run/media/ana/STICK/.Trash-1000/files/praia.jpg")
        machine.fail("test -e /run/media/ana/STICK/fotos/praia.jpg")
        note = machine.succeed("cat /run/media/ana/STICK/.Trash-1000/info/praia.jpg.trashinfo")
        # RELATIVE TO THE STICK, as the spec asks of a `$topdir` trash.
        assert "Path=fotos/praia.jpg\n" in note, f"the note is not relative to the volume:\n{note}"
        # AND NOTHING WENT TO THE HOME: no copy across filesystems.
        machine.fail("test -e /home/ana/.local/share/Trash/files/praia.jpg")

    with subtest("mounted somewhere else, it goes back where the stick is now"):
        machine.succeed("umount /run/media/ana/STICK")
        machine.succeed("mkdir -p /run/media/ana/OTHER")
        machine.succeed("mount -o uid=1000,gid=100,umask=077 /dev/vdb /run/media/ana/OTHER")
        ask({
            "ask": "add",
            "work": {
                "work": "restore",
                "root": "/run/media/ana/OTHER/.Trash-1000",
                "names": ["praia.jpg"],
            },
        })
        machine.wait_until_succeeds("test -f /run/media/ana/OTHER/fotos/praia.jpg")
        assert machine.succeed("cat /run/media/ana/OTHER/fotos/praia.jpg").strip() == "praia"
        machine.fail("test -e /run/media/ana/OTHER/.Trash-1000/info/praia.jpg.trashinfo")

    with subtest("a stick's own trash expires like the home's"):
        # THROWN AWAY AGAIN, and its note back-dated: the expiry reads the
        # note's `DeletionDate`, so this is a file thrown away long ago.
        ask({
            "ask": "add",
            "work": {
                "work": "trash",
                "root": "/run/media/ana/OTHER/.Trash-1000",
                "paths": ["/run/media/ana/OTHER/fotos/praia.jpg"],
            },
        })
        NOTE = "/run/media/ana/OTHER/.Trash-1000/info/praia.jpg.trashinfo"
        machine.wait_until_succeeds(f"test -f {NOTE}")
        as_ana(f"sed -i 's/^DeletionDate=.*/DeletionDate=2000-01-01T00:00:00/' {NOTE}")
        # THE UNIT'S OWN LINE, as the person: the home's trash and the drives
        # mounted under the media folder. The stick is found as the ROOT OF A
        # MOUNT there, not by name.
        said = as_ana(
            "${emptyTrash}/bin/ronoctua-empty-trash --data-home /home/ana/.local/share "
            "--media /run/media/ana --days 4 --execute"
        )
        assert "/run/media/ana/OTHER/.Trash-1000/files" in said, said
        machine.fail("test -e /run/media/ana/OTHER/.Trash-1000/files/praia.jpg")
        machine.fail(f"test -e {NOTE}")

    print("OK: a file thrown away on a vfat stick went to the stick's own trash, "
          "with a note relative to the stick, came back where the stick was mounted next, "
          "and the stick's trash expired like the home's")
  '';
}
