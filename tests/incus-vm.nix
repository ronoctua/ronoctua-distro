# Incus on a BOOTED machine: the daemon as `nix/modules/incus.nix` declares it,
# asked by the person, and the shapes `ronoctua-incus` reads held to it.
#
# WHAT ONLY THE VM PROVES
# -----------------------
#   1. THE PRESEED IS WHAT WAS DECLARED: images never refresh by themselves
#      (`images.auto_update_interval` 0), the bridge on its fixed subnet with
#      NAT, the pool btrfs, the profile's NIC and root disk.
#   2. THE SOCKET IS THE PERSON'S, through `incus-admin`, and nobody else's.
#   3. A CONTAINER LAUNCHES FROM A LOCAL IMAGE — a tarball built here (busybox,
#      a DHCP client and a sleep), imported with no network at all — and gets
#      an address on the bridge from the daemon's dnsmasq: the bridge, the
#      NAT and the trusted interface together.
#   4. THE JSON HAS EVERY KEY THE CRATE READS. The REST answers the crate
#      parses (`crates/ronoctua-incus/src/fixtures.rs` holds their shapes),
#      and the CLI's remotes, are asked of this daemon, every key read is
#      checked, and the answers are CAPTURED into this test's output
#      (`fixtures/`): a change of the daemon's shapes is a diff to read, not
#      a guess.
#   5. THE QUEUE'S CHILD WORKS AGAINST THE REAL DAEMON: `ronoctua-incus-job`
#      pushes a file into the instance and pulls it back byte for byte,
#      exports the instance into a new 0600 file that is a backup tarball,
#      refuses to export over a file that is there, and refuses a launch from
#      a remote nobody let — before asking the daemon anything.
#   6. THE KNOB A VM'S DISK USES: a block volume made with
#      `btrfs.compression=none` on this compressed pool is NO-COPY-ON-WRITE
#      (`lsattr` C) — what the crate asks for every VM it creates.
#   7. THE PERSISTED STATE'S SUBVOLUME: the program the unit runs, against a
#      btrfs made here, in each case it handles — nothing there, an empty
#      folder, a folder with data (converted, the data kept), a folder with
#      data whose live folder is in use (left for the next boot), a folder
#      holding Incus's own subvolumes (left a folder, every one intact), a
#      conversion interrupted before and after its swap, and a draft that
#      holds a subvolume (left, said).
#
# NO NESTED KVM in the test VM: containers only. A VM instance is the crate's
# unit tests' (its request) and the knob's (6); whether the knob reaches a
# VM made FROM AN IMAGE (a snapshot of the image's volume) needs a VM, and is
# `docs/PENDING.md`'s.
{
  pkgs,
  lib,
  ronoctua,
}:

let
  busybox = pkgs.pkgsStatic.busybox;
  # THE IMAGE: a unified tarball (`metadata.yaml` + `rootfs/`), the format
  # `incus image import` takes with no network.
  image =
    pkgs.runCommand "ronoctua-incus-probe-image"
      {
        nativeBuildInputs = [ pkgs.gnutar ];
      }
      ''
        mkdir -p image/rootfs/{bin,sbin,etc,dev,proc,sys,tmp,root,run,usr/share/udhcpc}
        cp ${busybox}/bin/busybox image/rootfs/bin/busybox
        for applet in sh sleep ip udhcpc cat ls echo mkdir; do
          ln -s busybox image/rootfs/bin/$applet
        done
        cat > image/rootfs/usr/share/udhcpc/default.script <<'EOF'
        #!/bin/sh
        case "$1" in
          bound|renew)
            ip addr flush dev "$interface"
            ip addr add "$ip/''${mask:-24}" dev "$interface"
            ;;
        esac
        EOF
        cat > image/rootfs/sbin/init <<'EOF'
        #!/bin/sh
        ip link set lo up
        ip link set eth0 up
        udhcpc -i eth0 -b -s /usr/share/udhcpc/default.script
        while :; do sleep 3600; done
        EOF
        chmod 0755 image/rootfs/sbin/init image/rootfs/usr/share/udhcpc/default.script
        cat > image/metadata.yaml <<'EOF'
        architecture: x86_64
        creation_date: 1791201600
        properties:
          description: ronoctua probe (busybox)
          os: busybox
          release: probe
        EOF
        mkdir -p $out
        tar --owner=0 --group=0 --numeric-owner -C image -czf $out/probe.tar.gz metadata.yaml rootfs
      '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-incus";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/filesystem.nix
        ../nix/modules/incus.nix
      ];
      _module.args.ronoctua = ronoctua;
      ronoctua.incus.enable = true;
      ronoctua.incus.job = ronoctua.incusJob;
      # Incus refuses a firewall on iptables.
      networking.nftables.enable = true;
      # THE DAEMON'S FOLDER ON BTRFS, compressed as the distro's disk is: the
      # pool is btrfs here as there, and a block volume meets the same mount.
      virtualisation.emptyDiskImages = [ 4096 ];
      virtualisation.fileSystems."/var/lib/incus" = {
        device = "/dev/vdb";
        fsType = "btrfs";
        autoFormat = true;
        options = [ "compress=zstd" ];
      };
      # One btrfs mount, and nothing for the monthly scrub to do in a test.
      services.btrfs.autoScrub.enable = lib.mkForce false;
      virtualisation.memorySize = 2048;
      virtualisation.diskSize = 4096;
      networking.hostName = lib.mkForce "noctua-incus";
      # No /persist in this VM: `base.nix` points the password hash there.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
        uid = 1000;
      };
      users.users.root.hashedPassword = lib.mkForce null;
      users.users.stranger = {
        isNormalUser = true;
        uid = 1001;
      };
      environment.systemPackages = [
        pkgs.btrfs-progs
        pkgs.curl
        pkgs.e2fsprogs
        pkgs.jq
      ];
    };

  testScript =
    { nodes, ... }:
    let
      user = nodes.machine.ronoctua.user;
      cli = "${nodes.machine.virtualisation.incus.clientPackage}/bin/incus";
      job = "${ronoctua.incusJob}/bin/ronoctua-incus-job";
      subvolume = "${nodes.machine.ronoctua.incus.subvolumeProgram}/bin/ronoctua-incus-subvolume";
    in
    ''
      import json, shlex

      user = "${user}"
      cli = "${cli}"
      job = "${job}"
      home = f"/home/{user}"
      socket = "/var/lib/incus/unix.socket"

      def as_person(command):
          # A login shell of the person's: their groups, their session's
          # variables.
          return machine.succeed(f"su - {user} -c {shlex.quote(command)}")

      def rest(path, keep=None):
          # THE DAEMON'S ANSWER, as the crate asks for it — over the socket,
          # as the person — and, when named, KEPT for this test's output.
          tee = f" | tee /tmp/fixtures/{keep}" if keep else ""
          answer = json.loads(as_person(f"curl -sf --unix-socket {socket} http://incus{path}{tee}"))
          assert answer["type"] == "sync", f"{path}: {answer}"
          return answer

      def has(where, data, keys):
          missing = [k for k in keys if k not in data]
          assert not missing, f"{where} lost {missing}: {sorted(data)}"

      def job_said(work):
          # The queue's way: one JSON line on stdin, the lines on stdout.
          line = shlex.quote(json.dumps(work))
          return as_person(
              f"echo {line} | RONOCTUA_INCUS={cli} {job}; echo exit=$?"
          ).strip().splitlines()

      machine.start()
      machine.succeed("mkdir -m 1777 -p /tmp/fixtures")
      machine.wait_for_unit("incus.service")
      machine.wait_for_unit("incus-preseed.service")

      with subtest("the preseed is what was declared"):
          assert machine.succeed(f"{cli} config get images.auto_update_interval").strip() == "0"
          # `query` and not `show`: this client's `show` prints YAML only (it
          # refuses `--format`), and the driver has no YAML reader.
          network = json.loads(machine.succeed(f"{cli} query /1.0/networks/incusbr0"))
          assert network["config"]["ipv4.address"] == "${nodes.machine.ronoctua.incus.subnet}", network
          assert network["config"]["ipv4.nat"] == "true", network
          assert network["managed"] is True, network
          pool = json.loads(machine.succeed(f"{cli} query /1.0/storage-pools/default"))
          assert pool["driver"] == "btrfs", pool
          profile = json.loads(machine.succeed(f"{cli} query /1.0/profiles/default"))
          assert profile["devices"]["root"]["pool"] == "default", profile
          assert profile["devices"]["eth0"]["network"] == "incusbr0", profile

      with subtest("the socket is the person's and nobody else's"):
          rest("/1.0")
          machine.fail(f"su - stranger -c {shlex.quote(f'curl -sf --unix-socket {socket} http://incus/1.0')}")
          said = as_person("echo $RONOCTUA_INCUS").strip()
          assert said == cli, f"the session's RONOCTUA_INCUS is {said!r}"

      with subtest("a container launches from a local image, and the bridge gives it an address"):
          as_person(f"{cli} image import ${image}/probe.tar.gz --alias probe")
          as_person(f"{cli} launch probe web")
          listed = f"su - {user} -c {shlex.quote(f'{cli} list web --format json')}"
          machine.wait_until_succeeds(
              listed + " | jq -e '.[0].status == \"Running\"'",
              timeout=60,
          )
          machine.wait_until_succeeds(
              listed + " | jq -e '[.[0].state.network.eth0.addresses[] | select(.family == \"inet\")] | length == 1'",
              timeout=90,
          )
          as_person(f"{cli} snapshot create web before-upgrade")

      with subtest("the JSON has every key the crate reads, and is captured"):
          instances = rest("/1.0/instances?recursion=2", "instances.json")
          web = next(i for i in instances["metadata"] if i["name"] == "web")
          has("an instance", web, ["name", "type", "status_code", "project", "description", "profiles",
                                   "created_at", "expanded_config", "config", "state", "snapshots"])
          assert web["type"] == "container" and web["status_code"] == 103, web
          state = web["state"]
          has("a state", state, ["disk", "memory", "network", "processes", "cpu"])
          has("a state's memory", state["memory"], ["usage"])
          has("a state's cpu", state["cpu"], ["usage"])
          nic = state["network"]["eth0"]
          has("a nic", nic, ["addresses", "type"])
          inet = [a for a in nic["addresses"] if a["family"] == "inet"]
          has("an address", inet[0], ["family", "address", "scope"])
          assert inet[0]["scope"] == "global", inet
          assert state["network"]["lo"]["type"] == "loopback", state["network"]["lo"]
          has("a snapshot", web["snapshots"][0], ["name", "created_at"])
          rest("/1.0/instances/web/state", "state.json")
          snaps = rest("/1.0/instances/web/snapshots?recursion=1", "snapshots.json")
          assert snaps["metadata"][0]["name"].rsplit("/", 1)[-1] == "before-upgrade", snaps
          images = rest("/1.0/images?recursion=1", "images.json")
          has("an image", images["metadata"][0], ["fingerprint", "aliases", "properties", "type", "size"])
          for path, name, keys in [
              ("/1.0/profiles?recursion=1", "profiles.json", ["name", "description"]),
              ("/1.0/networks?recursion=1", "networks.json", ["name", "type", "managed"]),
              ("/1.0/storage-pools?recursion=1", "pools.json", ["name", "driver"]),
              ("/1.0/storage-pools/default/volumes?recursion=1", "volumes.json", ["name", "type"]),
              ("/1.0/projects?recursion=1", "projects.json", ["name", "description"]),
          ]:
              answer = rest(path, name)
              has(path, answer["metadata"][0], keys)
          profile = rest("/1.0/profiles/default")["metadata"]
          root = [d for d in profile["devices"].values() if d.get("type") == "disk" and d.get("path") == "/"]
          assert root and root[0]["pool"] == "default", profile
          assert rest("/1.0/storage-pools/default")["metadata"]["driver"] == "btrfs"
          remotes = json.loads(as_person(f"{cli} remote list --format json | tee /tmp/fixtures/remotes.json"))
          has("a remote", remotes["images"], ["Addrs", "Protocol", "Public"])
          assert remotes["images"]["Protocol"] == "simplestreams", remotes["images"]
          # THE EVENTS' SHAPE, by the CLI's own monitor (the same JSON the
          # WebSocket carries, one event per line): a snapshot taken while it
          # listens is a lifecycle event and an operation.
          as_person(
              f"timeout 60 {cli} monitor --type=lifecycle --type=operation --format=json "
              "> /tmp/fixtures/events.jsonl 2>/dev/null &"
          )
          # A snapshot a try, until the monitor (which may still be
          # connecting) has heard one.
          snapshot = shlex.quote(f"{cli} snapshot create web probe-$(date +%s%N)")
          machine.wait_until_succeeds(
              f"su - {user} -c {snapshot} && sleep 1 && grep -q instance-snapshot-created /tmp/fixtures/events.jsonl",
              timeout=60,
          )
          events = [json.loads(line) for line in machine.succeed("cat /tmp/fixtures/events.jsonl").splitlines() if line.strip()]
          life = next(e for e in events if e["type"] == "lifecycle" and e["metadata"]["action"] == "instance-snapshot-created")
          # THE PATH AND NOT A NAME: the daemon's metadata carries `source`
          # (the crate reads the name out of it), and the project is the
          # event's own.
          has("a lifecycle event", life["metadata"], ["action", "source"])
          has("an event", life, ["type", "metadata", "project"])
          ops = [e for e in events if e["type"] == "operation"]
          assert ops, f"no operation event: {events}"
          has("an operation event", ops[0]["metadata"], ["id", "description", "status_code", "resources", "may_cancel", "err"])
          machine.copy_from_vm("/tmp/fixtures", "fixtures")

      with subtest("the queue's child pushes, pulls, exports and refuses"):
          as_person("printf 'from the host\\n' > ~/note.txt && chmod 0640 ~/note.txt")
          said = job_said({"job": "push", "name": "web", "from": f"{home}/note.txt", "to": "/root/note.txt"})
          assert said[-2:] == ["done", "exit=0"], said
          said = job_said({"job": "pull", "name": "web", "from": "/root/note.txt", "to": f"{home}/back.txt"})
          assert said[-2:] == ["done", "exit=0"], said
          assert as_person("cat ~/back.txt") == "from the host\n"
          said = job_said({"job": "export", "name": "web", "to": f"{home}/web.tar.gz"})
          assert said[-2:] == ["done", "exit=0"], said
          assert any(s.startswith("bytes ") for s in said), said
          assert as_person("stat -c %a ~/web.tar.gz").strip() == "600"
          listed = as_person("tar tzf ~/web.tar.gz")
          assert "backup/index.yaml" in listed, listed
          backups = rest("/1.0/instances/web/backups")["metadata"]
          assert backups == [], f"the export left its backup on the daemon: {backups}"
          said = job_said({"job": "export", "name": "web", "to": f"{home}/web.tar.gz"})
          assert said[-2:] == ["refused invalid", "exit=1"], said
          said = job_said({"job": "launch", "create": {
              "name": "far", "kind": "container",
              "source": {"from": "remote", "server": "https://images.linuxcontainers.org",
                         "protocol": "simple-streams", "alias": "debian/13"},
              "profiles": [], "limits": {"cpu": None, "memory_bytes": None, "processes": None}}})
          assert said[-2:] == ["refused not-let", "exit=1"], said
          machine.fail(f"su - {user} -c {shlex.quote(f'{cli} info far')}")

      with subtest("a block volume without compression is no-copy-on-write"):
          as_person(f"{cli} storage volume create default blk --type=block size=8MiB btrfs.compression=none")
          attrs = machine.succeed(
              "lsattr /var/lib/incus/storage-pools/default/custom/default_blk/root.img"
          ).split()[0]
          assert "C" in attrs, f"the block volume is copy-on-write: {attrs}"

      with subtest("the persisted state becomes a subvolume of its own, in every case"):
          machine.succeed(
              "truncate -s 256M /tmp/persist.img && mkfs.btrfs -q /tmp/persist.img "
              "&& mkdir -p /mnt/p && mount -o loop /tmp/persist.img /mnt/p && mkdir -p /mnt/live"
          )
          kept = "/mnt/p/var/lib/incus"
          is_subvolume = lambda: machine.succeed(f"stat -c %i {kept}").strip() == "256"
          # NOTHING THERE: made.
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert is_subvolume(), "not made"
          machine.succeed(f"btrfs subvolume delete {kept}")
          # AN EMPTY FOLDER (what a switch's tmpfiles makes): replaced.
          machine.succeed(f"mkdir -p {kept}")
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert is_subvolume(), "the empty folder was not replaced"
          machine.succeed(f"btrfs subvolume delete {kept}")
          # A FOLDER WITH DATA, the live folder not in use: converted, kept.
          machine.succeed(f"mkdir -p {kept}/images && echo data > {kept}/images/one")
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert is_subvolume(), "not converted"
          assert machine.succeed(f"cat {kept}/images/one").strip() == "data", "the data was lost"
          machine.fail(f"test -e {kept}.subvolume-in-progress")
          # AND A SECOND RUN FINDS IT DONE.
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          machine.succeed(f"btrfs subvolume delete {kept}")
          # A FOLDER WITH DATA WHILE THE LIVE ONE IS IN USE: left alone.
          machine.succeed(f"mkdir -p {kept} && echo data > {kept}/two && mount --bind {kept} /mnt/live")
          said = machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert "next boot" in said, said
          assert not is_subvolume(), "converted under a live bind"
          machine.succeed("umount /mnt/live")
          machine.succeed(f"rm -rf {kept}")
          draft = kept + ".subvolume-in-progress"
          # A FOLDER HOLDING INCUS'S OWN SUBVOLUMES (it ran on a btrfs pool
          # there): left a folder, whole, said — a copy would make each a
          # plain folder, and the old one could not be removed.
          machine.succeed(
              f"mkdir -p {kept}/storage-pools {kept}/database && echo db > {kept}/database/global "
              f"&& btrfs subvolume create {kept}/storage-pools/default "
              f"&& mkdir {kept}/storage-pools/default/containers "
              f"&& btrfs subvolume create {kept}/storage-pools/default/containers/web "
              f"&& echo rootfs > {kept}/storage-pools/default/containers/web/f"
          )
          said = machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert "stays a folder" in said, said
          assert not is_subvolume(), "a folder of subvolumes was converted"
          assert machine.succeed(f"cat {kept}/storage-pools/default/containers/web/f").strip() == "rootfs"
          web_inode = machine.succeed(f"stat -c %i {kept}/storage-pools/default/containers/web").strip()
          assert web_inode == "256", "an instance's subvolume became a folder"
          machine.fail(f"test -e {draft}")
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          machine.succeed(
              f"btrfs subvolume delete {kept}/storage-pools/default/containers/web "
              f"&& btrfs subvolume delete {kept}/storage-pools/default && rm -rf {kept}"
          )
          # INTERRUPTED BEFORE THE SWAP (the draft is the half-made
          # subvolume): undone, and the folder converted.
          machine.succeed(
              f"mkdir -p {kept} && echo data > {kept}/one "
              f"&& btrfs subvolume create {draft} && echo partial > {draft}/one"
          )
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert is_subvolume(), "not converted after the undo"
          assert machine.succeed(f"cat {kept}/one").strip() == "data", "the draft's copy won"
          machine.fail(f"test -e {draft}")
          machine.succeed(f"btrfs subvolume delete {kept}")
          # INTERRUPTED AFTER THE SWAP (the draft is the old folder): removed.
          machine.succeed(
              f"btrfs subvolume create {kept} && echo data > {kept}/one "
              f"&& mkdir -p {draft} && echo old > {draft}/one"
          )
          machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert is_subvolume() and machine.succeed(f"cat {kept}/one").strip() == "data"
          machine.fail(f"test -e {draft}")
          machine.succeed(f"btrfs subvolume delete {kept}")
          # A DRAFT HOLDING A SUBVOLUME: left as it is, said, and the unit
          # ends well (it would fail at every boot otherwise).
          machine.succeed(
              f"mkdir -p {kept} {draft} && echo data > {kept}/one "
              f"&& btrfs subvolume create {draft}/nested"
          )
          said = machine.succeed("${subvolume} " + kept + " /mnt/live")
          assert "left as it is" in said, said
          machine.succeed(f"test -d {draft}/nested")
          assert not is_subvolume()
    '';
}
