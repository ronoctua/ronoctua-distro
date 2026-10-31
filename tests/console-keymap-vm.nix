# A MACHINE'S CONSOLE TYPES ITS ANSWER, AND SO DOES ITS INITRD.
#
# WHAT ONLY THIS PROVES
# ---------------------
# `console-keymap-is-the-answers` reads the evaluation: the map the console
# is given, the `vconsole.conf` of the system and of the initrd, the initrd's
# copy. Whether the initrd's prompt for the disk's passphrase READS keys on
# that map is only seen by typing on it — the one prompt typed blind, where a
# map the session does not type makes a passphrase that cannot be typed again.
#
# The machine is UK (`gb`), with the distro's `base.nix` (the keyboard's
# options) and console module (the setup that loads the map, early), and
# `host.nix`'s four lines. Its root becomes LUKS, with a passphrase set from a
# shell that holds `" @ # ~`; at the initrd's prompt it is typed on the
# KEYBOARD (QEMU's keys are a US keyboard's positions): the keys that give
# `" @ # ~` on a UK map give `@ " \ |` on a US one, so the disk opens only if
# the prompt reads the UK map. Booted, the same keys typed on a console come
# out the same.
{
  pkgs,
  lib,
  consoleModule,
  ronoctua,
}:

let
  # The keys QEMU presses (a US keyboard's names), and what a UK map makes
  # of them — what a US map makes is `uk@"\|map`.
  keys = [
    "u"
    "k"
    "shift-2"
    "shift-apostrophe"
    "backslash"
    "shift-backslash"
    "m"
    "a"
    "p"
  ];
  passphrase = ''uk"@#~map'';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-console-keymap";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        consoleModule
      ];
      _module.args.ronoctua = ronoctua;
      # AS THE INSTALLER WRITES THEM in `host.nix`.
      ronoctua.keyboard.layout = "gb";
      ronoctua.keyboard.variant = "";
      ronoctua.keyboard.compose = "compose:caps";
      ronoctua.keyboard.console = "gb";

      networking.hostName = lib.mkForce "noctua-console";
      # No /persist in this VM: `base.nix` points the password hash there,
      # and nobody logs in.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };
      users.users.root.hashedPassword = lib.mkForce null;

      # THE ROOT BECOMES LUKS, as nixpkgs' own `systemd-initrd-luks-password`
      # does it: a second disk, formatted from the first boot, and a
      # specialisation that boots from it.
      virtualisation = {
        memorySize = 1536;
        emptyDiskImages = [ 512 ];
        useBootLoader = true;
        mountHostNixStore = true;
        useEFIBoot = true;
      };
      environment.systemPackages = [
        pkgs.cryptsetup
        pkgs.kbd
      ];
      specialisation.boot-luks.configuration = {
        boot.initrd.luks.devices = lib.mkVMOverride { cryptroot.device = "/dev/vdb"; };
        virtualisation.rootDevice = "/dev/mapper/cryptroot";
      };
    };

  testScript =
    { nodes, ... }:
    let
      luks = nodes.machine.specialisation.boot-luks.configuration.system.build.toplevel;
    in
    ''
      import datetime
      import json
      import time

      KEYS = json.loads(r'${builtins.toJSON keys}')
      PASSPHRASE = json.loads(r'${builtins.toJSON passphrase}')
      MAP = "${toString nodes.machine.console.keyMap}"


      def type_keys():
          for k in KEYS:
              machine.send_key(k)


      def typed_on_a_console(vt):
          """What the keys come out as on console `vt`, read by a shell there."""
          machine.succeed("rm -f /tmp/typed")
          machine.succeed(f"openvt -c {vt} -s -- sh -c 'read -r line; printf %s \"$line\" > /tmp/typed'")
          machine.sleep(duration=datetime.timedelta(seconds=2))
          type_keys()
          machine.send_key("ret")
          machine.wait_for_file("/tmp/typed")
          return machine.succeed("cat /tmp/typed")


      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("the system's console types the UK map"):
          conf = machine.succeed("cat /etc/vconsole.conf")
          assert f"KEYMAP={MAP}" in conf, conf
          said = typed_on_a_console(5)
          assert said == PASSPHRASE, f"the console typed {said!r}, and the UK map gives {PASSPHRASE!r}"

      with subtest("the root becomes LUKS, its passphrase set from a shell"):
          machine.succeed(f"printf %s '{PASSPHRASE}' | cryptsetup luksFormat -q --iter-time=1 /dev/vdb -")
          machine.succeed(f"printf %s '{PASSPHRASE}' | cryptsetup luksOpen -q /dev/vdb cryptroot")
          machine.succeed("mkfs.ext4 -q /dev/mapper/cryptroot")
          machine.succeed("cryptsetup close cryptroot")
          machine.succeed("${luks}/bin/switch-to-configuration boot")
          machine.succeed("sync")
          machine.crash()

      with subtest("the initrd's prompt reads the passphrase on the UK map"):
          machine.start()
          machine.wait_for_console_text("Please enter passphrase for disk cryptroot")
          # The console is set up before the prompt is answered: give the
          # keys to the prompt as a person would, a moment after it shows.
          # Not `machine.sleep`, which runs in the booted system's shell and
          # would wait for a boot that waits for this passphrase.
          time.sleep(2)
          type_keys()
          machine.send_key("ret")
          # A passphrase typed on another map is refused and asked again, and
          # the boot waits for it — and so would `wait_for_unit`, whose shell
          # only answers a booted system. The initrd says which, on the
          # serial line (measured with the initrd's map forced to `us`:
          # `Failed to activate with specified passphrase. (Passphrase
          # incorrect?)`).
          machine.wait_for_console_text(
              r"Reached target Initrd Root Device|Passphrase incorrect",
              timeout=datetime.timedelta(minutes=3),
          )
          assert "Passphrase incorrect" not in machine.get_console_log(), (
              "the passphrase typed at the initrd's prompt did not open the disk: not the UK map"
          )
          machine.wait_for_unit("multi-user.target")
          mounted = machine.succeed("mount")
          assert "/dev/mapper/cryptroot on / " in mounted, mounted
          said = typed_on_a_console(5)
          assert said == PASSPHRASE, f"booted from LUKS, the console typed {said!r}"
    '';
}
