# The person's containers on a BOOTED machine: Podman, rootless, compatible
# with Docker.
#
# WHAT ONLY THE VM PROVES
# -----------------------
#   1. A ROOTLESS CONTAINER RUNS, and while it runs crun keeps a folder for it
#      under `$XDG_RUNTIME_DIR/crun` — the one fact the brain's `containers`
#      module watches with inotify (no process, no poll). crun's layout is
#      private, so the pinned one is held to it HERE; when the container stops
#      the folder goes, and the mark with it.
#   2. `docker` IS PODMAN, and `docker ps` sees the same container.
#   3. THE ROOTLESS API SOCKET ANSWERS the Docker API at the address
#      `DOCKER_HOST` names — for shells (`environment.extraInit`) and for user
#      units (`environment.d`, `${XDG_RUNTIME_DIR}`) — and the ROOT socket is
#      not there.
#   4. COMPOSE IS `podman-compose`: `podman compose up` makes containers with
#      compose's labels (what `ronoctua_containers::compose_of` groups by) —
#      the file as it was given, relative, and the folder made real — and
#      `down`, run the way the app's queue runs it, removes them.
#   5. THE JSON HAS THE SHAPES THE CRATE READS. `ronoctua-containers` parses
#      what podman 5.8.2 printed into its fixtures; the VM asks the same
#      questions of the installed podman and checks every key the parsers
#      read — including `stats`, which the fixtures could not show (a stats
#      round needs a running container).
#   6. THE STORAGE SUBVOLUME: the unit `filesystem.nix` makes for
#      `~/.local/share/containers` makes it a subvolume on a btrfs home, the
#      folders on the way owned by the person, never root.
#
#   7. THE BRAIN'S MARK: the `containers` module, in the brain the person
#      runs, counts what crun keeps — the one container already running when
#      it starts (`started`), a second one while it runs (inotify, no poll),
#      and nothing once both have stopped: the mark goes with them. A click
#      on it opens the containers' view (`containers-open`).
#
# THE IMAGE is built with `dockerTools` (busybox, nothing else) and loaded with
# `podman load`: no registry, no network.
{
  pkgs,
  lib,
  ronoctua,
}:

let
  image = pkgs.dockerTools.buildImage {
    name = "localhost/ronoctua-probe";
    tag = "1";
    copyToRoot = pkgs.buildEnv {
      name = "probe-root";
      paths = [ pkgs.busybox ];
      pathsToLink = [ "/bin" ];
    };
    config.Cmd = [
      "/bin/sleep"
      "600"
    ];
  };
  compose = pkgs.writeText "compose.yaml" ''
    services:
      sleeper:
        image: localhost/ronoctua-probe:1
        command: ["/bin/sleep", "600"]
        network_mode: none
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-containers";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/filesystem.nix
        ../nix/modules/containers.nix
      ];
      _module.args.ronoctua = ronoctua;
      ronoctua.containers.enable = true;
      # The subvolume units exist only where btrfs is; the root here is not.
      # Forced on, so the unit is there to be run against a btrfs home made
      # below — on this ext4 root it says so and leaves the folder alone.
      ronoctua.filesystem.btrfs = true;
      # …and with no btrfs mount there is nothing to scrub (nixpkgs asserts
      # a filesystem for it).
      services.btrfs.autoScrub.enable = lib.mkForce false;
      virtualisation.memorySize = 3072;
      virtualisation.diskSize = 4096;
      networking.hostName = lib.mkForce "noctua-containers";
      # No /persist in this VM (the file manager's reason): `base.nix` points
      # the password hash there and nothing here logs in with it.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
        # A lingering manager: the person's sockets and runtime directory
        # without a login.
        linger = true;
        uid = 1000;
      };
      users.users.root.hashedPassword = lib.mkForce null;
      environment.systemPackages = [
        pkgs.btrfs-progs
        pkgs.curl
        pkgs.jq
      ];
    };

  testScript =
    { nodes, ... }:
    let
      user = nodes.machine.ronoctua.user;
      uid = toString nodes.machine.users.users.${user}.uid;
    in
    ''
      import json

      user = "${user}"
      uid = "${uid}"
      run_dir = f"/run/user/{uid}"

      def as_person(command):
          # A login shell of the person's, with the runtime directory and the
          # user bus a lingering manager gives them.
          return machine.succeed(
              f"su - {user} -c 'export XDG_RUNTIME_DIR={run_dir} "
              f"DBUS_SESSION_BUS_ADDRESS=unix:path={run_dir}/bus; {command}'"
          )

      machine.start()
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit(f"user@{uid}.service")

      with subtest("the root API socket is not there, the person's is"):
          # Not wanted by anything (a unit with no `[Install]` reads `static`,
          # which `is-enabled` counts as success — the link is the question).
          machine.fail("test -e /etc/systemd/system/sockets.target.wants/podman.socket")
          machine.fail("systemctl is-active podman.socket")
          machine.wait_until_succeeds(f"test -S {run_dir}/podman/podman.sock", timeout=60)

      with subtest("DOCKER_HOST and RONOCTUA_PODMAN reach shells and user units"):
          wanted = f"unix://{run_dir}/podman/podman.sock"
          # A login shell as a login gives it: the runtime directory set (by
          # pam_systemd, on a real login), then the profile read.
          said = machine.succeed(
              f"runuser -u {user} -- env -i HOME=/home/{user} XDG_RUNTIME_DIR={run_dir} "
              "bash -l -c 'echo $DOCKER_HOST'"
          ).strip()
          assert said == wanted, f"a login shell has DOCKER_HOST={said!r}, not {wanted!r}"
          units = as_person("systemctl --user show-environment")
          assert f"DOCKER_HOST={wanted}" in units, f"user units are not told DOCKER_HOST:\n{units}"
          podman = machine.succeed(
              f"runuser -u {user} -- env -i HOME=/home/{user} bash -l -c 'echo $RONOCTUA_PODMAN'"
          ).strip()
          assert podman.startswith("/nix/store/") and podman.endswith("/bin/podman"), podman

      with subtest("a rootless container runs, and crun keeps a folder while it does"):
          as_person("podman load -i ${image}")
          cid = as_person(
              "podman run -d --network none --name probe localhost/ronoctua-probe:1"
          ).strip()
          assert len(cid) == 64, cid
          machine.wait_until_succeeds(f"test -d {run_dir}/crun/{cid}", timeout=30)
          running = as_person(f"ls {run_dir}/crun").split()
          assert running == [cid], f"crun lists {running}, wanted exactly the one container"

      with subtest("the brain's mark counts what runs, as it runs"):
          # A BRAIN OF THE PERSON'S, quiet (nothing it schedules starts), with
          # the environment its unit gives it (`nix/modules/brain.nix`).
          # Written as root: `as_person` quotes its command in single quotes.
          machine.succeed(
              f"mkdir -p /home/{user}/.config/ronoctua && "
              f"echo '{{\"quiet\":true}}' > /home/{user}/.config/ronoctua/brain.json && "
              f"chown -R {user}:users /home/{user}/.config"
          )
          as_person(
              "systemd-run --user --unit=ronoctua-brain-probe "
              "--setenv=RONOCTUA_SYSTEMCTL=${pkgs.systemd}/bin/systemctl "
              "--setenv=RONOCTUA_JOURNALCTL=${pkgs.systemd}/bin/journalctl "
              "${ronoctua.brain}/bin/ronoctua-brain"
          )
          machine.wait_until_succeeds(f"test -S {run_dir}/ronoctua-brain.sock", timeout=60)

          def mark():
              mind = json.loads(as_person("${ronoctua.brain}/bin/ronoctua-brain look"))
              ours = [m["what"] for m in mind.get("marks", []) if m["module"] == "containers"]
              assert len(ours) <= 1, f"more than one containers mark: {ours}"
              return ours[0] if ours else None

          def counts(n):
              # `wait_until_succeeds` asks the machine; the mark is asked here,
              # until it says `n` (None: no mark at all).
              for _ in range(60):
                  m = mark()
                  if (m["count"] if m else None) == n:
                      return m
                  machine.sleep(1)
              raise AssertionError(f"the containers mark never said {n}: {mark()}")

          # THE ONE ALREADY RUNNING, counted when the watch starts.
          m = counts(1)
          assert m["id"].get("opens") == "containers-open", f"a click on the mark opens {m['id']}"
          # A SECOND ONE, heard while the brain runs: inotify, not a restart.
          as_person("podman run -d --network none --name probe2 localhost/ronoctua-probe:1")
          counts(2)
          as_person("podman stop -t 0 probe2 && podman rm probe2")
          counts(1)

      with subtest("docker is podman"):
          listed = as_person("docker ps --quiet --no-trunc")
          assert cid in listed, f"docker ps does not see the container:\n{listed}"

      with subtest("the socket answers the Docker API"):
          answer = as_person(
              f"curl -sf --unix-socket {run_dir}/podman/podman.sock http://d/v1.40/containers/json"
          )
          ids = [c["Id"] for c in json.loads(answer)]
          assert cid in ids, f"the API does not list the container: {ids}"

      with subtest("the JSON has every key the crate reads"):
          def keys(question, wanted):
              data = json.loads(as_person(f"podman {question} --format json"))
              assert data, f"podman {question} answered nothing"
              missing = [k for k in wanted if k not in data[0]]
              assert not missing, f"podman {question} lost {missing}: {sorted(data[0])}"
          keys("ps --all", ["Id", "Names", "Image", "State", "Status", "Created", "Ports", "Pod", "Labels", "IsInfra"])
          keys("images", ["Id", "Names", "Size", "Created", "Containers"])
          keys("network ls", ["id", "name", "driver", "subnets", "dns_enabled"])
          as_person("podman volume create probe-volume")
          keys("volume ls", ["Name", "Driver", "Mountpoint", "CreatedAt"])
          as_person("podman pod create --name probe-pod")
          keys("pod ps", ["Id", "Name", "Status", "Containers", "Created", "InfraId"])
          keys("stats --no-stream", ["id", "name", "cpu_percent", "mem_usage", "mem_percent", "net_io", "block_io", "pids"])
          events = as_person("podman events --since 10m --stream=false --format json").splitlines()
          assert events, "no events at all"
          event = json.loads(events[-1])
          missing = [k for k in ["Type", "Status", "ID", "Name"] if k not in event]
          assert not missing, f"an event lost {missing}: {event}"

      with subtest("stopping it takes crun's folder away"):
          as_person("podman stop -t 0 probe")
          machine.wait_until_succeeds(f"test ! -e {run_dir}/crun/{cid}", timeout=30)
          as_person("podman rm probe")
          # …AND THE MARK WITH IT: at rest, none (invariant 14).
          counts(None)
          as_person("systemctl --user stop ronoctua-brain-probe")

      with subtest("compose is podman-compose, with compose's labels"):
          as_person("mkdir -p ~/site && cp ${compose} ~/site/compose.yaml")
          as_person("cd ~/site && podman compose up -d")
          made = json.loads(as_person("podman ps --all --format json"))
          labelled = [c for c in made if (c.get("Labels") or {}).get("com.docker.compose.project") == "site"]
          assert labelled, f"compose made no container with its project label: {made}"
          labels = labelled[0]["Labels"] or {}
          assert labels.get("com.docker.compose.service") == "sleeper"
          # COMPOSE WRITES THE FILE AS IT WAS GIVEN IT — relative to wherever
          # it ran — and the folder made real; `ronoctua_containers` resolves
          # the one against the other (`parse::resolved_files`). The premise
          # the crate's captured fixture holds, held to the installed compose:
          site = f"/home/{user}/site"
          assert labels.get("com.docker.compose.project.working_dir") == site, labels
          assert labels.get("com.docker.compose.project.config_files") == "compose.yaml", labels
          # …AND DOWN AS THE APP'S JOB RUNS IT (`Job::ComposeDown`): in the
          # first file's folder, every file resolved, the project's own name.
          as_person(f"cd {site} && podman compose --project-name site --file {site}/compose.yaml down")
          left = json.loads(as_person("podman ps --all --format json"))
          assert not [c for c in left if (c.get("Labels") or {}).get("com.docker.compose.project") == "site"], left

      with subtest("the storage is a subvolume of its own, and nothing on the way is root's"):
          # The unit on this ext4 root: it says so and leaves the folder be.
          unit = f"ronoctua-containers-subvolume-{user}.service"
          machine.succeed(f"systemctl cat {unit}")
          said = machine.succeed(f"journalctl -b -u {unit} --no-pager")
          assert "not on btrfs" in said, said
          # The same program, against a btrfs home made here.
          line = machine.succeed(f"systemctl show -p ExecStart --value {unit}")
          # systemd prints `{ path=/nix/store/… ; argv[]=/nix/store/… … }`:
          # the program is the value after `path=`, never the whole word.
          words = (w.split("=", 1)[-1] for w in line.replace(";", " ").split())
          program = next(w for w in words if w.startswith("/") and w.endswith("/bin/ronoctua-cache-subvolume"))
          machine.succeed(
              "truncate -s 256M /tmp/home.img && mkfs.btrfs -q /tmp/home.img "
              "&& mkdir -p /mnt/b && mount -o loop /tmp/home.img /mnt/b "
              f"&& mkdir /mnt/b/{user} && chown {user}:users /mnt/b/{user}"
          )
          machine.succeed(
              f"{program} /mnt/b/{user} .local/share/containers {user} users /tmp/note 7"
          )
          target = f"/mnt/b/{user}/.local/share/containers"
          assert machine.succeed(f"stat -c %i {target}").strip() == "256", "not a subvolume"
          for folder in [f"/mnt/b/{user}/.local", f"/mnt/b/{user}/.local/share", target]:
              owner = machine.succeed(f"stat -c %U {folder}").strip()
              assert owner == user, f"{folder} belongs to {owner}"
          # And a second run finds it done.
          machine.succeed(
              f"{program} /mnt/b/{user} .local/share/containers {user} users /tmp/note 7"
          )
          machine.fail(f"{program} /mnt/b/{user} ../escape {user} users /tmp/note 7")
    '';
}
