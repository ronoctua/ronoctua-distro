# The installer stick's root writer, against a USB stick plugged into a
# booted machine — what only a booted machine answers.
#
# WHAT THIS PROVES, AND WHAT IT LEAVES TO OTHERS
# ----------------------------------------------
# The unit tests judge planted readings (`ronoctua_stick::device`,
# `drives`) and copy into buffers; this plugs a REAL usb-storage device into
# the running machine (QEMU's `device_add`, the way a hand does it) and runs
# the unit `nix/modules/stick.nix` installs, as the system runs it:
#
#   - a stick is written whole, through `O_DIRECT`, and read back to the
#     image's digest — from the kernel's own `/dev/disk/by-id` link and udev's
#     serial, which no fixture can stand in for;
#   - a drive that changed (another size), an image that is not the person's
#     (`/etc/shadow`), a mounted stick and bytes that are not the image's are
#     each refused BY NAME in `progress.json`, and the last leaves no
#     bootable half on the stick;
#   - a `cancel` ends a writer that is still waiting, and the queue's reason
#     in it is the writer's answer;
#   - every answer about a request names the request's number;
#   - a writer STOPPED while it waits leaves a final answer (`ExecStopPost`),
#     so no view waits on a writer that is gone.
#
# The download (aria2, the consent passes) is the queue's and the sudo door's
# start of the unit is the security helper's; each has its own proof
# (`transfers-vm`, `stick-root-is-narrow`). A release is not fetched: the
# image is a few mebibytes of noise the person owns.
{
  pkgs,
  lib,
  ronoctua,
  # The writer, extracted on its own (`nix/rust.nix`).
  stickWrite,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-stick";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        (import ../nix/modules/stick.nix { inherit stickWrite; })
      ];
      _module.args.ronoctua = ronoctua;
      virtualisation.memorySize = 1536;
      # A USB CONTROLLER, and nothing on it yet: the stick is plugged in by
      # the script, as a hand plugs one.
      virtualisation.qemu.options = [
        "-device"
        "qemu-xhci,id=xhci"
      ];
      networking.hostName = lib.mkForce "noctua-stick";
      # No /persist in this VM — `base.nix` points the password hash there.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };
      users.users.root.hashedPassword = lib.mkForce null;
      environment.systemPackages = [ pkgs.dosfstools ];
    };

  testScript =
    { nodes, ... }:
    ''
      import json, shlex

      user = "${nodes.machine.ronoctua.user}"
      staged = "/run/ronoctua-stick/staged"
      image = f"/home/{user}/.cache/ronoctua/stick/image.iso"

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("a stick plugged in has a stable name and a serial"):
          stick = machine.state_dir / "stick.img"
          with open(stick, "wb") as f:
              f.truncate(64 << 20)
          machine.send_monitor_command(
              f"drive_add 0 if=none,id=stick,file={stick},format=raw"
          )
          machine.send_monitor_command(
              "device_add usb-storage,bus=xhci.0,drive=stick,id=stickdev,"
              "serial=RONOCTUA0001,removable=on"
          )
          machine.wait_until_succeeds(
              "ls /dev/disk/by-id/ | grep -E '^usb-.*RONOCTUA0001.*-0:0$'"
          )
          name = machine.succeed(
              "ls /dev/disk/by-id/ | grep -E '^usb-.*RONOCTUA0001.*-0:0$'"
          ).split()[0]
          by_id = "/dev/disk/by-id/" + name
          dev = machine.succeed(f"readlink -f {by_id}").strip()
          size = int(machine.succeed(f"blockdev --getsize64 {dev}").strip())
          serial = machine.succeed(
              f"udevadm info --query=property --name={dev} | sed -n 's/^ID_SERIAL_SHORT=//p'"
          ).strip()
          assert serial, f"udev gave {dev} no short serial"
          assert size == 64 << 20, f"{dev} is {size} bytes"

      # THE IMAGE, the person's: three mebibytes and a ragged tail, so the
      # last block is padded.
      machine.succeed(
          f"runuser -u {user} -- sh -c 'mkdir -p $(dirname {image}) && "
          f"head -c 3145851 /dev/urandom > {image}'"
      )
      digest = machine.succeed(f"sha256sum {image}").split()[0]
      length = 3145851

      def request(**change):
          r = {
              "format": 1,
              "nonce": 4242,
              "image": image,
              "sha256": digest,
              "target": {
                  "drive": "/org/freedesktop/UDisks2/drives/stick",
                  "by-id": by_id,
                  "serial": serial,
                  "size-bytes": size,
                  "label": "QEMU stick",
              },
          }
          for k, v in change.items():
              if k in r["target"]:
                  r["target"][k] = v
              else:
                  r[k] = v
          return r

      def write(req, ready=True, reason=""):
          # AS THE PERSON: the staging folder is theirs and nobody else's.
          machine.succeed(f"runuser -u {user} -- rm -f {staged}/ready {staged}/cancel {staged}/request.json")
          # NO RESET BETWEEN STARTS: the unit has no start limit, so this
          # script starting the writer faster than a hand does is still one
          # gesture each.
          machine.succeed("systemctl start ronoctua-stick-write.service")
          machine.succeed(
              f"echo {shlex.quote(json.dumps(req))} | "
              f"runuser -u {user} -- sh -c 'umask 077; cat > {staged}/request.json'"
          )
          if ready:
              machine.succeed(f"runuser -u {user} -- touch {staged}/ready")
          else:
              machine.succeed(
                  f"runuser -u {user} -- sh -c 'umask 077; printf %s {shlex.quote(reason)} > {staged}/cancel'"
              )
          machine.wait_until_fails(
              "systemctl is-active --quiet ronoctua-stick-write.service", timeout=120
          )
          return json.loads(machine.succeed("cat /run/ronoctua-stick/progress.json"))

      def on_the_stick(n):
          return machine.succeed(f"head -c {n} {dev} | sha256sum").split()[0]

      with subtest("the stick is written whole and reads back as the image"):
          said = write(request())
          assert said["phase"] == "done" and said["why"] is None, said
          assert said["nonce"] == 4242, f"the answer does not name its request: {said}"
          assert said["done-bytes"] == length, said
          assert on_the_stick(length) == digest, "the stick does not hold the image"
          tail = machine.succeed(
              f"dd if={dev} bs=1 skip={length} count={4096 - length % 4096} 2>/dev/null | tr -d '\\000' | wc -c"
          ).strip()
          assert tail == "0", f"the padding after the image is not zeros ({tail} bytes)"

      with subtest("a drive that changed is not written"):
          said = write(request(**{"size-bytes": size * 2}))
          assert said["phase"] == "failed" and said["why"] == "changed", said
          said = write(request(serial="ANOTHER"))
          assert said["why"] == "changed", said
          assert on_the_stick(length) == digest, "a refused write touched the stick"

      with subtest("root never copies a file the person does not own"):
          said = write(request(image="/etc/shadow"))
          assert said["phase"] == "failed" and said["why"] == "failed", said
          assert on_the_stick(length) == digest, "a refused write touched the stick"

      with subtest("a mounted stick is refused, and stays mounted"):
          machine.succeed(f"mkfs.vfat -I {dev}")
          machine.succeed(f"mkdir -p /mnt/stick && mount {dev} /mnt/stick")
          said = write(request())
          assert said["why"] == "not-writable", said
          machine.succeed("mountpoint -q /mnt/stick")
          machine.succeed("umount /mnt/stick")

      with subtest("bytes that are not the image's leave nothing bootable"):
          said = write(request(sha256="0" * 64))
          assert said["why"] == "mismatch", said
          left = machine.succeed(
              f"head -c 1048576 {dev} | tr -d '\\000' | wc -c"
          ).strip()
          assert left == "0", f"the first mebibyte kept {left} bytes of what was written"

      with subtest("a cancel ends a writer that is still waiting"):
          said = write(request(), ready=False)
          assert said["phase"] == "failed" and said["why"] is None, said
          assert said["nonce"] is None, f"no request was read: {said}"
          said = write(request(), ready=False, reason="mismatch")
          assert said["phase"] == "failed" and said["why"] == "mismatch", said

      with subtest("a writer stopped while it waits leaves a final answer"):
          machine.succeed(f"runuser -u {user} -- rm -f {staged}/ready {staged}/cancel")
          machine.succeed("systemctl start ronoctua-stick-write.service")
          machine.wait_until_succeeds(
              "grep -q '\"phase\":\"waiting\"' /run/ronoctua-stick/progress.json"
          )
          machine.succeed("systemctl stop ronoctua-stick-write.service")
          said = json.loads(machine.succeed("cat /run/ronoctua-stick/progress.json"))
          assert said["phase"] == "failed" and said["why"] == "failed", said

      with subtest("the unit reaches sticks and cards, and no other disk"):
          policy = machine.succeed(
              "systemctl show -p DevicePolicy -p DeviceAllow ronoctua-stick-write.service"
          )
          assert "DevicePolicy=closed" in policy, policy
          assert "block-sd" in policy and "block-mmc" in policy, policy
          assert "virtblk" not in policy and "blkext" not in policy, policy
    '';
}
