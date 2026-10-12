# Declaring a program on a BOOTED machine, offline: `declare-installs-now-and-records`.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# `ronoctua-software`'s own tests run the declaration's orders against a
# `nix` and a `systemctl` of their own making (`declare::tests`). Here the
# real ones answer, and the order goes in the way the queue hands it — one
# JSON line on the standard input of `ronoctua-software run --progress`, the
# CLI's WRAPPER, run as the person with no terminal and no polkit agent:
#
#   1. DECLARE INSTALLS NOW: `nix profile add nixpkgs#hello` — the machine's
#      own locked nixpkgs, evaluated in the VM — puts `hello` in the person's
#      profile at once, and it runs;
#   2. AND RECORDS: the person's mirror (`~/.local/state/ronoctua/person/`)
#      names `nix:hello`, edited on THIS machine's name; the staging was
#      laid with a nonce; `ronoctua-person-apply.service` — started by the
#      person through its polkit rule, with nobody to ask — swapped it into
#      the machine directory as root, canonical and root's; and its verdict
#      answers that staging's nonce;
#   3. A DECLINE ON ANOTHER MACHINE is recorded and applied the same way and
#      rebuilds nothing here;
#   4. FORGETTING takes the entry out of the machine's copy too, and the copy
#      by hand out of the profile — with no rebuild, since the system never
#      had it (there is no `person-applied.json`).
#
# OFFLINE. Nothing reaches a substituter: `hello` is in the VM's store
# (`additionalPaths`), the machine's nixpkgs is the registry's locked path,
# and the belongings' CLI is absent, so the push is "with the next sync" —
# said in the journal, never a failure.
{
  pkgs,
  lib,
  inputs,
  ronoctua,
}:

let
  # THE PROGRAM, the way `nix profile add nixpkgs#hello` would build it: the
  # flake's overlays (rust-overlay, yt-dlp) do not reach it, so this is the
  # same derivation the machine's locked nixpkgs evaluates to — and the test
  # asserts that before it relies on it.
  hello = pkgs.hello;
  host = "noctua-declare";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-declare";

  # `home.nix` and the browser's module reach Home Manager through `inputs`.
  node.specialArgs = { inherit inputs; };

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        # THE OPTIONS the software module answers (`ronoctua.update.steps`,
        # the transfer queue's switch, the icons' and the browser's), from
        # the modules that declare them — `software-vm`'s set, without
        # Flatpak and AppImage: a Nix package is what is declared here, and
        # Flatpak wants a portal this sessionless machine has no use for.
        ../nix/modules/brain.nix
        ../nix/modules/transfers.nix
        ../nix/modules/media.nix
        ../nix/modules/icons.nix
        ../nix/modules/home.nix
        ../nix/modules/browser.nix
        ../nix/modules/software.nix
        # ROOT'S HALF: the apply unit and its one polkit rule.
        ../nix/modules/person-apply.nix
        inputs.home-manager.nixosModules.home-manager
      ];

      # THE TWO OPTIONS the apply module reads, declared here: their own
      # modules (`machine.nix`, `person.nix`) carry the whole machine flake's
      # consumer — the `person-vm` precedent.
      options.ronoctua.machine.dir = lib.mkOption {
        type = lib.types.str;
        default = "/etc/nixos";
      };
      options.ronoctua.person.tool = lib.mkOption { type = lib.types.package; };

      config = {
        _module.args.ronoctua = ronoctua;
        ronoctua.person.tool = ronoctua.person;

        virtualisation.memorySize = 3072;
        virtualisation.cores = 2;
        networking.hostName = lib.mkForce host;
        users.users.root.hashedPassword = lib.mkForce null;
        # No password file of the machine's: the person never logs in here.
        users.users.${config.ronoctua.user}.hashedPasswordFile = lib.mkForce null;
        nix.settings.experimental-features = [
          "nix-command"
          "flakes"
        ];

        # NOTHING STARTS A SESSION: the person is a process with no terminal,
        # as the queue's child is; the brain and the update stay off.
        ronoctua.media.enable = false;
        ronoctua.icons.superpack = false;
        ronoctua.browser.enable = false;
        ronoctua.update.enable = false;
        ronoctua.brain.enable = false;

        # OFFLINE, and saying so. And no GLOBAL registry: Nix downloads it
        # before it reads any entry, the system's pinned `nixpkgs` included,
        # so offline every `nixpkgs#…` failed on `channels.nixos.org`.
        nix.settings.substituters = lib.mkForce [ ];
        nix.settings.flake-registry = "";
        virtualisation.additionalPaths = [ hello ];
      };
    };

  testScript =
    { nodes, ... }:
    ''
      import json
      import shlex

      user = "${nodes.machine.ronoctua.user}"
      home = f"/home/{user}"
      host = "${host}"
      # THE WRAPPER, which carries the machine's tools (`RONOCTUA_NIX`,
      # `RONOCTUA_SYSTEMCTL`) — the program the queue runs.
      software = "${nodes.machine.ronoctua.software.package}/bin/ronoctua-software"
      machine_dir = "/etc/nixos"
      mirror = f"{home}/.local/state/ronoctua/person/software"


      def person(command, stdin=None):
          """The person, as the queue's child: their uid, their HOME, the
          session's PATH, no terminal and no agent."""
          feed = f"printf '%s\\n' {shlex.quote(stdin)} | " if stdin is not None else ""
          return (
              feed
              + f"systemd-run --wait --pipe --quiet --uid={uid} --gid=users "
              + f"--setenv=HOME={home} "
              + f"--setenv=PATH=/run/wrappers/bin:{home}/.nix-profile/bin:/run/current-system/sw/bin "
              + f"-- ${pkgs.bash}/bin/bash -c {shlex.quote(command)}"
              + ("" if stdin is not None else " < /dev/null")
          )


      def order(body):
          """One order, exactly as the queue hands it, and the CLI's lines."""
          out = machine.succeed(person(f"{software} run --progress", json.dumps(body)))
          print(out)
          return out


      def doc(path):
          return json.loads(machine.succeed(f"cat {path}"))


      def ids(path):
          return [a["id"] for a in doc(path)["apps"]]


      start_all()
      machine.wait_for_unit("multi-user.target")
      uid = machine.succeed(f"id -u {user}").strip()
      # THE MACHINE DIRECTORY, root's, as an installed machine has it.
      machine.succeed(f"mkdir -p {machine_dir} && chmod 0755 {machine_dir}")

      with subtest("the machine's locked nixpkgs evaluates hello to the program in the store"):
          evaluated = machine.succeed(
              person("nix eval --raw nixpkgs#hello.outPath")
          ).strip()
          assert evaluated == "${hello}", f"nixpkgs#hello is {evaluated}, not ${hello}"

      with subtest("declare installs now and records"):
          order({"order": "declare", "key": {"source": "nix", "attr": "hello"}})
          # INSTALLED NOW, by today's path, in the person's own profile.
          listed = machine.succeed(person("nix profile list"))
          assert "hello" in listed, listed
          said = machine.succeed(person(f"{home}/.nix-profile/bin/hello")).strip()
          assert said == "Hello, world!", said
          # RECORDED in the mirror, with this machine's name.
          entry = next(a for a in doc(f"{mirror}/declared.json")["apps"] if a["id"] == "nix:hello")
          assert entry["source"] == "nix" and entry["attr"] == "hello", entry
          assert entry["edited-on"] == host, entry
          assert entry["machines"] is None, entry
          # STAGED, and APPLIED BY ROOT through the polkit rule.
          verdict = json.loads(machine.succeed("cat /run/ronoctua-person/verdict.json"))
          nonce = machine.succeed("cat /run/ronoctua-person/staged/nonce").strip()
          assert verdict["outcome"] == "applied", verdict
          assert verdict["nonce"] == nonce, (verdict, nonce)
          copy = f"{machine_dir}/person/software/declared.json"
          assert ids(copy) == ["nix:hello"], doc(copy)
          owner = machine.succeed(f"stat -c %U:%a {copy}").strip()
          assert owner == "root:644", owner
          # ROOT'S COPY IS THE MIRROR'S, byte for byte: one canonical form.
          machine.succeed(f"cmp {copy} {mirror}/declared.json")

      with subtest("declared again, nothing is installed twice"):
          before = machine.succeed(person("nix profile history | grep -c Version || true")).strip()
          order({"order": "declare", "key": {"source": "nix", "attr": "hello"}})
          after = machine.succeed(person("nix profile history | grep -c Version || true")).strip()
          assert before == after, f"a second generation was made: {before} -> {after}"

      with subtest("a decline on another machine is recorded and applied, and rebuilds nothing"):
          out = order({"order": "decline", "id": "htop", "machines": ["laptop"]})
          declined = doc(f"{machine_dir}/person/software/declined.json")["apps"]
          assert [(d["id"], d["machines"]) for d in declined] == [("htop", ["laptop"])], declined
          # THE REBUILD is a step the CLI says before it starts it.
          assert "ronoctua-update-apply.service" not in out, out

      with subtest("forgetting takes it out of the machine's copy and out of the profile"):
          out = order({"order": "forget", "key": {"source": "nix", "attr": "hello"}})
          assert "ronoctua-update-apply.service" not in out, "a system that never had it was rebuilt"
          assert ids(f"{machine_dir}/person/software/declared.json") == [], (
              doc(f"{machine_dir}/person/software/declared.json")
          )
          assert ids(f"{mirror}/declared.json") == []
          listed = machine.succeed(person("nix profile list"))
          assert "hello" not in listed, listed
          machine.fail(f"test -e {home}/.nix-profile/bin/hello")
    '';
}
