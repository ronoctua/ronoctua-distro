# The disk-space module's own checks, in the shape every module's `checks.nix`
# has: the arguments it asks for by name (`nix/brain-modules.nix`,
# `checkArguments`), and its checks, each named after its id.
{
  mkCheck,
  pkgs,
  lib,
  self,
  rust,
  pyPrelude,
  module,
  ...
}:

let
  catalogue = import ../../../../assets/cache.nix;
  vm = self.nixosConfigurations.vm.config;
  service = unit: lib.removeSuffix ".service" unit.name;
  unitFacts =
    unit:
    let
      s = vm.systemd.user.services.${service unit}.serviceConfig;
    in
    {
      name = unit.name;
      # READ AS WORDS, WITHOUT ITS STORE PATHS: the command line names the
      # relief's binary, and a file quoting it would make this check build
      # the whole workspace to compare two strings.
      execStart = builtins.unsafeDiscardStringContext s.ExecStart;
      runtimeDirectory = s.RuntimeDirectory or "";
      readWritePaths = s.ReadWritePaths or [ ];
    };

  # THE CATALOGUE WITH ONE NUMBER CHANGED, through the one generator that
  # refuses it — `nix/cache.nix`, called as the flake calls it. `true` is a
  # table that built.
  builds =
    changes:
    (builtins.tryEval
      (pkgs.callPackage ../../../../nix/cache.nix {
        cacheFile = builtins.toFile "cache.nix" (lib.generators.toPretty { } (catalogue // changes));
      }).drvPath
    ).success;
in
{
  checks = {
    # THE BRAIN READS WHERE THE RELIEF WRITES, and starts the units that exist.
    #
    # The runner watches `<REPORTS_LEAF>/<id>.json` — `<id>-rehearsal.json`
    # for a rehearsal — under the runtime directory, and starts the units the
    # manifest names. The units are `module.nix`'s, and their `--report` is
    # built from the folder's name. Four places one name has to agree, and
    # when one of them moved the relief sat until its patience ran out and
    # came back `runaway`, with the disk still short. This reads the units as
    # the machine gets them — the evaluated configuration, not the file.
    #
    # AND EVERY UNIT THAT WRITES THERE MAKES THE DIRECTORY FIRST: its sandbox
    # opens it with `ReadWritePaths`, and a path that is not there fails the
    # unit with 226/NAMESPACE before the program runs (measured).
    disk-space-writes-where-the-brain-reads =
      let
        facts = pkgs.writeText "disk-space-units.json" (
          builtins.toJSON {
            inherit (module) id;
            run = unitFacts module.units.run;
            rehearse = unitFacts module.units.rehearse;
          }
        );
      in
      mkCheck "disk-space-writes-where-the-brain-reads" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the two units arrive as the evaluated configuration, JSON.
        # FILE-WIDE: from the runner, one constant and one function's body
        # (`body_of`), comments off and strings kept (`uncommented`) — the
        # needles ARE strings.
        facts = json.load(open("${facts}"))
        runner = uncommented(pathlib.Path("${../../../ronoctua-brain-daemon/src/runner.rs}").read_text())
        floor(len(runner), 1000, "characters of runner.rs")
        wrong = []

        leaf = re.search(r'pub const REPORTS_LEAF: &str = "([^"]+)";', runner)
        if not leaf:
            sys.exit("ERROR: `REPORTS_LEAF` went away; it is the name the relief has to agree with")
        leaf = leaf.group(1)
        built = must(body_of(runner, "report_in"), "the runner's `report_in`")
        for shape in ('"{task}.json"', '"{task}-rehearsal.json"'):
            if shape not in built:
                wrong.append(f"the runner no longer names a report {shape}")

        i = facts["id"]
        for which, report, acts in (("run", f"{i}.json", True), ("rehearse", f"{i}-rehearsal.json", False)):
            u = facts[which]
            start = u["execStart"]
            if f"--report %t/{leaf}/{report} " not in start + " ":
                wrong.append(f"{u['name']} does not write %t/{leaf}/{report}, which the brain watches")
            if ("--execute" in start.split()) != acts:
                wrong.append(f"{u['name']} " + ("does not act" if acts else "acts: a rehearsal deleted something"))
            if u["runtimeDirectory"] != leaf:
                wrong.append(f"{u['name']} does not make %t/{leaf} before its sandbox opens it")
            if f"%t/{leaf}" not in u["readWritePaths"]:
                wrong.append(f"{u['name']} may not write into %t/{leaf}")

        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the relief's two units write %t/{leaf}/{i}.json and its rehearsal, and make the directory first")
        END
      '';

    # THE "NEARLY FULL" LINE IS UNDER THE FLOOR, and the build refuses it
    # otherwise.
    #
    # Under the floor the relief makes room by itself; the modeline's mark is
    # for what it could not free. A line at or above the floor would bring the
    # mark up every time the relief starts and take it down every time it
    # works — a modeline speaking while nothing is asked of anybody. So
    # `nix/cache.nix` refuses both the share and the cap out of order, and
    # this proves it REFUSES: the catalogue as it is builds, and with either
    # number moved onto the floor's it does not. And the table the module
    # compiles carries the line the catalogue declares.
    disk-space-nearly-full-is-under-the-floor =
      let
        facts = pkgs.writeText "disk-space-lines.json" (
          builtins.toJSON {
            asIs = builds { };
            shareAtTheFloor = builds { diskWarnPercent = catalogue.diskFloorPercent; };
            capAtTheFloor = builds { diskWarnCapGiB = catalogue.diskFloorCapGiB; };
            shareBelow = builds { diskWarnPercent = catalogue.diskFloorPercent - 1; };
            percent = catalogue.diskWarnPercent;
            cap = catalogue.diskWarnCapGiB;
          }
        );
        table = (import ./tables.nix { inherit pkgs lib; })."cache.rs";
      in
      mkCheck "disk-space-nearly-full-is-under-the-floor" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: what the generator did with each catalogue arrives as JSON
        # evaluated here; the table is generated Rust, read for two constants.
        # FILE-WIDE: the two are module-level `pub const`s of a generated file
        # that has no function to narrow to.
        facts = json.load(open("${facts}"))
        table = pathlib.Path("${table}").read_text()
        floor(len(table), 200, "characters of the module's cache.rs")
        wrong = []
        if not facts["asIs"]:
            wrong.append("the catalogue as it is does not build")
        if not facts["shareBelow"]:
            wrong.append("a line one per cent under the floor was refused")
        if facts["shareAtTheFloor"]:
            wrong.append("`diskWarnPercent` equal to `diskFloorPercent` was accepted")
        if facts["capAtTheFloor"]:
            wrong.append("`diskWarnCapGiB` equal to `diskFloorCapGiB` was accepted")
        for name, value in (("DISK_WARN_PERCENT", facts["percent"]), ("DISK_WARN_CAP_GIB", facts["cap"])):
            if not re.search(r"pub const " + name + r": \w+ = " + str(value) + r";", table):
                wrong.append(f"the module's table does not carry {name} = {value}")
        if wrong:
            for w in wrong:
                print("ERROR: " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: the nearly-full line is under the floor, the build refuses it otherwise, and the module compiles it")
        END
      '';

    # THE UNIT THAT MAKES ROOM IS NARROW: what it carries and what it may touch.
    #
    # It runs as the person, without asking, the moment the disk is short —
    # so it has to be the smallest thing that can do the job. Its binary is
    # extracted (`rust.diskRelief`) so its closure is the sweep's and never
    # the desktop's: a relief that dragged the renderer, Vulkan or the
    # compiler in would itself fill the disk it is there to free. And its
    # sandbox is the sweep's, line for line (`module.nix`): the system
    # read-only, the home read-only but for the cache and the brain's
    # reports, no network but the daemon's Unix socket, no new privileges.
    # Both units — the rehearsal is the one nobody reads, so it is held to
    # the same lines — and only the real one acts.
    disk-space-relief-unit-is-narrow =
      let
        closure = pkgs.closureInfo { rootPaths = [ rust.diskRelief ]; };
        sandbox =
          unit:
          let
            s = vm.systemd.user.services.${service unit}.serviceConfig;
          in
          {
            execStart = builtins.unsafeDiscardStringContext s.ExecStart;
            readWritePaths = s.ReadWritePaths or [ ];
            protectSystem = s.ProtectSystem or "";
            protectHome = s.ProtectHome or "";
            privateNetwork = s.PrivateNetwork or false;
            addressFamilies = s.RestrictAddressFamilies or [ ];
            noNewPrivileges = s.NoNewPrivileges or false;
            restrictSuid = s.RestrictSUIDSGID or false;
            privateDevices = s.PrivateDevices or false;
            syscalls = s.SystemCallFilter or [ ];
            namespaces = s.RestrictNamespaces or false;
          };
        facts = pkgs.writeText "disk-space-relief-units.json" (
          builtins.toJSON {
            run = sandbox module.units.run;
            rehearse = sandbox module.units.rehearse;
          }
        );
      in
      mkCheck "disk-space-relief-unit-is-narrow" [ pkgs.python3 ] ''
        python3 - <<'END'
        import json, pathlib, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the evaluated units arrive as JSON, and the closure as the
        # list of store paths `closureInfo` writes, one a line.
        # FILE-WIDE: neither has a function to narrow to.
        units = json.load(open("${facts}"))
        paths = pathlib.Path("${closure}/store-paths").read_text().split()
        floor(len(paths), 5, "store paths in the relief's closure")
        wrong = []
        for unwanted in ("-zig-", "-clang-", "-llvm-", "-mesa-", "-vulkan-loader-", "-wayland-",
                         "-noctua-iosevka-", "-icons-superpack-", "-ronoctua-shell", "-wasmtime"):
            hits = [p for p in paths if unwanted in p]
            if hits:
                wrong.append(f"the relief's binary drags {unwanted.strip('-')} in: {hits[:3]}")
        for name, u in units.items():
            say = lambda what: wrong.append(f"the {name} unit {what}")
            if u["protectSystem"] != "strict":
                say(f"has ProtectSystem={u['protectSystem']!r}, not strict")
            if u["protectHome"] != "read-only":
                say(f"has ProtectHome={u['protectHome']!r}, not read-only")
            if sorted(u["readWritePaths"]) != sorted(["-%C", "%t/ronoctua-brain"]):
                say(f"may write {u['readWritePaths']}: the cache and the brain's reports, nothing else")
            if not u["privateNetwork"] or u["addressFamilies"] != ["AF_UNIX"]:
                say("reaches a network: the store is collected through the daemon's Unix socket only")
            for key in ("noNewPrivileges", "restrictSuid", "privateDevices", "namespaces"):
                if not u[key]:
                    say(f"lost `{key}`")
            if "@system-service" not in u["syscalls"]:
                say("has no system-call filter")
        if "--execute" not in units["run"]["execStart"].split():
            wrong.append("the real unit does not act (`--execute`): this check reads the wrong unit")
        if "--execute" in units["rehearse"]["execStart"].split():
            wrong.append("the REHEARSAL acts (`--execute`): it deletes what it was meant to describe")

        for w in wrong:
            print("ERROR: " + w, file=sys.stderr)
        if wrong:
            sys.exit(1)
        print(f"OK: the relief carries {len(paths)} store paths and none of them draws; both units are the sweep's sandbox")
        END
      '';

    # THE RELIEF ON A MACHINE: its two units, in their sandbox, on a home AND
    # a store that are really short (`docs/PENDING.md`, "A folga do disco").
    #
    # A small btrfs is the person's home; an old compiler's builds and the
    # session compiler's own are planted in its cache, and a file fills it
    # below the floor. The store is short too: the VM's writable store is an
    # overlay whose upper layer is a small tmpfs (`/nix/.rw-store`, the
    # filesystem `statvfs` answers for `/nix/store`), and on it the person
    # adds GARBAGE (paths nothing roots) and a path her own profile holds, and
    # a file outside the store fills the rest — so the room the store step
    # wants is MORE than all the garbage, and the collection takes every dead
    # path it can. The rehearsal deletes nothing and says what it would; the
    # real unit takes the old compiler's builds whole — never the session
    # compiler's — until the home's goal is free, then collects the store's
    # garbage through the daemon. What the person's profile holds is the same
    # size as each dead path, so a collection that ignored her root would have
    # taken it too: that it stays is the root being honoured, not luck. (The
    # paths the system's closure holds are never planted here: they would stay
    # whatever the relief did.)
    disk-space-vm = pkgs.testers.runNixOSTest {
      name = "ronoctua-disk-space";
      nodes.machine = {
        imports = [ ./module.nix ];
        _module.args.ronoctua = {
          inherit (rust) diskRelief;
          # THE SESSION'S COMPILER is `rustc-test` here: the planted
          # `rustc-0.0.1` is another compiler's, which the relief may take.
          cargoDirs = import ../../../../nix/cargo-dirs.nix { version = "test"; };
        };
        virtualisation.memorySize = 2048;
        virtualisation.emptyDiskImages = [ 1024 ];
        virtualisation.fileSystems."/home" = {
          device = "/dev/vdb";
          fsType = "btrfs";
          autoFormat = true;
        };
        # THE STORE'S FILESYSTEM, small enough to be short: the overlay's
        # upper layer, where everything added at run time lands.
        virtualisation.writableStore = true;
        virtualisation.writableStoreUseTmpfs = true;
        virtualisation.fileSystems."/nix/.rw-store".options = [ "size=384m" ];
        boot.supportedFilesystems = [ "btrfs" ];
        users.users.alice = {
          isNormalUser = true;
          linger = true;
        };
        # HER HOME ON THE SMALL DISK, made after it is mounted: the activation
        # makes `/home/alice` on the root filesystem, before `/home` (not
        # needed for boot) is mounted over it and hides it. The tmpfiles setup
        # runs after the local filesystems, so it makes the home where it is.
        systemd.tmpfiles.rules = [ "d /home/alice 0700 alice users -" ];
      };
      testScript = ''
        import json

        gib, mib = 1024 ** 3, 1024 ** 2
        machine.wait_for_unit("multi-user.target")
        machine.wait_for_unit("user@1000.service")

        def as_alice(command):
            return machine.succeed(f"su - alice -c {json.dumps(command)}")

        # SYNCED FIRST: btrfs counts what was just written (or deleted) only
        # when its transaction commits, so a `df` right after planting reads
        # the room from before it, and a filler sized by it does not fit.
        def avail(at="/home"):
            return int(machine.succeed(f"sync -f {at} && df -B1 --output=avail {at} | tail -1").strip())

        def size(at="/home"):
            return int(machine.succeed(f"df -B1 --output=size {at} | tail -1").strip())

        def goal_of(at):
            return min(size(at) * ${toString catalogue.diskGoalPercent} // 100, ${toString catalogue.diskGoalCapGiB} * gib)

        def unit(name):
            machine.succeed(f"systemctl --machine=alice@.host --user start --wait {name}")

        def added(name, mebibytes):
            """A path the person adds to the store: random, so no two are one."""
            return as_alice(
                f"head -c {mebibytes * mib} /dev/urandom > /tmp/{name} && nix-store --add /tmp/{name} && rm /tmp/{name}"
            ).strip()

        with subtest("the store's filesystem is the small tmpfs, and nothing planted is in the system's closure"):
            fs = machine.succeed("stat -f -c %T /nix/.rw-store").strip()
            assert fs == "tmpfs", f"the store's upper layer is {fs}, not the small tmpfs"
            assert size("/nix/store") <= 400 * mib, f"the store's filesystem is {size('/nix/store')} bytes: not the small one"

        with subtest("a home below the floor: an old compiler's builds, the session's, a filler"):
            old = "/home/alice/.cache/cargo-target/rustc-0.0.1"
            kept = "/home/alice/.cache/cargo-target/rustc-test"
            as_alice(f"mkdir -p {old}/target/release {kept}/target/debug")
            as_alice(f"printf 'Signature: 8a477f597d28d172789f06886806bc55\\n' > {old}/target/CACHEDIR.TAG")
            as_alice(f"printf 'Signature: 8a477f597d28d172789f06886806bc55\\n' > {kept}/target/CACHEDIR.TAG")
            as_alice(f"head -c {300 * mib} /dev/urandom > {old}/target/release/build.bin")
            as_alice(f"head -c {50 * mib} /dev/urandom > {kept}/target/debug/build.bin")
            as_alice(f"find {old} -exec touch -h -d '200 days ago' {{}} +")
            floor_bytes = size() * ${toString catalogue.diskFloorPercent} // 100
            room = avail() - floor_bytes + 30 * mib
            as_alice(f"fallocate -l {room} /home/alice/filler")
            print(f"home: size {size()}, avail {avail()}, floor {floor_bytes}")
            assert avail() < floor_bytes, "the home is not below the floor: the test proves nothing"

        with subtest("a store short by more than its garbage: dead paths, the person's root, a filler"):
            # NO GARBAGE BUT WHAT IS PLANTED: a dead path the VM registered
            # lives on the read-only lower layer, and deleting it frees nothing
            # of the tmpfs while Nix counts its size as freed — a collection
            # asked for the step's bytes stopped after one such path (measured:
            # "1 store paths deleted, 263.8 MiB freed", 12 MiB free before and
            # after), with the planted garbage still there.
            machine.succeed("nix-store --gc")
            each = 12
            dead = [added(f"dead-{n}", each) for n in range(3)]
            path = added("kept-by-alice", each)
            as_alice(f"mkdir -p ~/.local/state/nix/profiles && nix-env -p ~/.local/state/nix/profiles/test --set {path}")
            # DEAD, as the store says it: nothing roots the three, and the
            # person's path is not among them.
            listed = set(machine.succeed("nix-store --gc --print-dead 2>/dev/null").split())
            assert set(dead) <= listed, f"the planted garbage is not dead: {set(dead) - listed}"
            assert path not in listed, "the person's profile does not root her path: the test proves nothing"
            # THE REST OF THE STORE'S FILESYSTEM, filled outside the store:
            # nothing a collection frees. What the step wants is then more
            # than every dead path together.
            goal = goal_of("/nix/store")
            fill = avail("/nix/store") - 12 * mib
            machine.succeed(f"fallocate -l {fill} /nix/.rw-store/filler")
            wanted = goal - avail("/nix/store")
            print(f"store: size {size('/nix/store')}, avail {avail('/nix/store')}, goal {goal}, wanted {wanted}")
            assert wanted > len(dead) * each * mib, (
                f"the store wants {wanted} bytes, no more than its garbage: the collection could stop before the root mattered"
            )

        with subtest("the rehearsal says what it would take, and takes nothing"):
            before, before_store = avail(), avail("/nix/store")
            unit("${module.units.rehearse.name}")
            machine.succeed(f"test -e {old}/target/release/build.bin")
            for p in dead:
                machine.succeed(f"test -e {p}")
            report = json.loads(machine.succeed("cat /run/user/1000/ronoctua-brain/${module.id}-rehearsal.json"))
            print(json.dumps(report)[:600])
            assert abs(avail() - before) < 8 * mib, f"the rehearsal freed {avail() - before} bytes of the home"
            assert abs(avail("/nix/store") - before_store) < 2 * mib, "the rehearsal collected the store"
            said = machine.succeed("journalctl --no-pager _SYSTEMD_USER_UNIT=${module.units.rehearse.name}")
            assert "store-garbage: would collect" in said, f"the rehearsal never reached the store step: {said[-800:]}"

        with subtest("the relief takes the old compiler's builds, then every dead path, and nothing rooted"):
            before_store = avail("/nix/store")
            unit("${module.units.run.name}")
            machine.fail(f"test -e {old}")
            machine.succeed(f"test -e {kept}/target/debug/build.bin")
            machine.succeed("test -e /home/alice/filler")
            goal = size() * ${toString catalogue.diskGoalPercent} // 100
            assert avail() >= min(goal, ${toString catalogue.diskGoalCapGiB} * gib), (
                f"{avail()} bytes free after the relief, under the goal {goal}"
            )
            report = json.loads(machine.succeed("cat /run/user/1000/ronoctua-brain/${module.id}.json"))
            print(json.dumps(report)[:600])
            steps = {s["id"]: s for s in report.get("steps", [])}
            assert "store-garbage" in steps and steps["store-garbage"].get("ok"), f"the store step did not run whole: {steps}"
            for p in dead:
                machine.fail(f"test -e {p}")
            freed = avail("/nix/store") - before_store
            assert freed >= len(dead) * each * mib * 9 // 10, f"the store freed {freed} bytes, not its garbage"
            # THE PERSON'S ROOT, honoured by a collection that took every
            # dead path it found.
            machine.succeed(f"test -e {path}")
            as_alice("test -L ~/.local/state/nix/profiles/test")
            assert as_alice("readlink -f ~/.local/state/nix/profiles/test").strip() == path
      '';
    };
  };
  slow = [ ];
}
