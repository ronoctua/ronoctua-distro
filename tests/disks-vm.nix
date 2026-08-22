# Disks, partitions and pendrives — what only a booted machine answers.
#
# WHAT THIS PROVES, AND WHAT IT CANNOT
# ------------------------------------
# There are two halves to the sidebar's device side, and neither the machine
# nor a VM can prove both.
#
# The REMOVABLE half — `HintSystem = false`, mounting with no password,
# unmounting — needs a device the kernel calls removable, and QEMU does not
# hand one out for free: `emptyDiskImages` gives a virtio disk, which udev
# marks as fixed. That half is proved against a real pendrive, and the numbers
# it produced are the fixture of `ronoctua_disks`'s unit tests.
#
# So this proves the OTHER half, which is the one a real machine will not let
# anybody prove safely: **a system volume, refused, with no authentication
# agent in the session**. Doing that on somebody's laptop means unmounting
# their `/boot` and hoping.
#
# Written down because an absence that does not explain itself reads as an
# oversight, and the next person would spend an afternoon adding the subtest
# that cannot work.
#
# AND WHY A `-vm` AT ALL
# ----------------------
# Three of these are facts about a SYSTEM and not about our code: that udisks
# is D-Bus activatable, that its udev rules arrived, and that upstream's polkit
# defaults are what the whole design rests on. A nixpkgs bump can change any of
# them, and the symptom on a desktop is a stick that does not appear with
# nothing anywhere saying why.
{
  pkgs,
  lib,
  ronoctua,
  # The formatter, extracted on its own (`nix/rust.nix`): it is three D-Bus
  # calls, and the workspace it comes out of carries wgpu and the wasm runtime.
  formatDisk,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-disks";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/disks.nix
      ];
      _module.args.ronoctua = ronoctua;
      virtualisation.memorySize = 2048;
      # A disk of its own to format and offer to udisks. Virtio, so the kernel
      # calls it FIXED — which is the point here: it is the `HintSystem = true`
      # case, and the refusal is what is being measured.
      virtualisation.emptyDiskImages = [ 64 ];
      networking.hostName = lib.mkForce "noctua-discos";
      # No /persist in this VM — the same reason as the file manager's and the
      # rescue's: `base.nix` points the password hash there and nothing here
      # logs in.
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
      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("o servico responde ao barramento"):
          # D-BUS ACTIVATABLE, and not "the unit is enabled": udisks starts on
          # the first call and a unit nobody ever calls proves nothing.
          objects = machine.succeed(
              "busctl --system call org.freedesktop.UDisks2 /org/freedesktop/UDisks2 "
              "org.freedesktop.DBus.ObjectManager GetManagedObjects"
          )
          assert "block_devices" in objects, (
              f"o udisks2 nao listou dispositivo nenhum:\n{objects[:400]}"
          )

      with subtest("a politica do upstream esta instalada, com os padroes que o desenho assume"):
          # THE WHOLE DESIGN RESTS ON THESE TWO NUMBERS. `filesystem-mount` is
          # `yes` for an active local session, which is why the sidebar mounts a
          # stick with no password; `filesystem-mount-system` is
          # `auth_admin_keep`, which is why it does NOT offer to mount an
          # internal partition. A nixpkgs bump that changes the posture has to
          # fail here and not on somebody's desk.
          policy = machine.succeed(
              "cat /run/current-system/sw/share/polkit-1/actions/"
              "org.freedesktop.UDisks2.policy"
          )
          import re
          def allow_active(action):
              block = re.search(
                  r'<action id="org\.freedesktop\.udisks2\.' + action + r'">(.*?)</action>',
                  policy, re.S,
              )
              assert block, f"a acao {action} sumiu da politica do udisks2"
              found = re.search(r"<allow_active>([^<]+)</allow_active>", block.group(1))
              assert found, f"a acao {action} nao declara allow_active"
              return found.group(1).strip()

          assert allow_active("filesystem-mount") == "yes", (
              "montar um dispositivo removivel deixou de ser livre para a sessao "
              "ativa: a barra lateral passa a pedir senha para um pendrive"
          )
          assert allow_active("filesystem-mount-system") == "auth_admin_keep", (
              "montar um volume de sistema deixou de pedir o administrador: a "
              "recusa que a barra lateral antecipa nao vale mais"
          )
          # AND FORMATTING SPLITS THE SAME WAY, which is what makes the format
          # buffer possible at all: writing a filesystem onto a stick is free
          # for the active session, and writing one onto an internal disk asks
          # for an administrator this session has no agent to answer with. The
          # buffer's own refusals sit IN FRONT of these two — they are the
          # design, not the enforcement.
          assert allow_active("modify-device") == "yes", (
              "formatar um dispositivo removivel passou a pedir senha: o buffer "
              "de formatar nao tem agente nenhum para responder"
          )
          assert allow_active("modify-device-system") == "auth_admin_keep", (
              "formatar um disco de sistema deixou de pedir o administrador"
          )

      with subtest("e nenhuma regra polkit nossa fala de udisks"):
          # The inverse of `files-polkit-is-narrow`, and read from the BUILT
          # system rather than from the source: upstream's rule already splits
          # on `HintSystem`, and ours could only widen it.
          rules = machine.succeed(
              "grep -RIl udisks /etc/polkit-1/rules.d/ 2>/dev/null || true"
          ).strip()
          assert not rules, f"alguem escreveu uma regra polkit para o udisks: {rules}"

      with subtest("as regras udev chegaram, e um disco anexado tem os hints"):
          # THIS IS WHAT `services.udev.packages` BUYS, and it can silently not
          # happen. Without the hints every property the sidebar reads comes
          # back empty and every disk looks the same.
          # A PARTITION AND NOT THE WHOLE DISK. udisks does not offer a
          # filesystem written straight onto a disk with no partition table as
          # mountable — it answers "not a mountable filesystem" and never adds
          # the `Filesystem` interface, which is a different refusal wearing the
          # same clothes as the one being measured below. A partition is also
          # what a real stick has.
          machine.succeed("sfdisk /dev/vdb <<< 'label: dos\n,,c'")
          machine.succeed("udevadm settle")
          machine.succeed("mkfs.vfat -n PROVA /dev/vdb1")
          machine.succeed("udevadm settle")
          info = machine.succeed("udisksctl info -b /dev/vdb1")
          assert "IdLabel:" in info and "PROVA" in info, (
              f"o udisks nao leu o rotulo de /dev/vdb1:\n{info}"
          )
          # AND THE INTERFACE IS THERE, which is what makes the next subtest
          # measure polkit and not a race with udisks' re-probe.
          assert "MountPoints:" in info, (
              f"o udisks nao deu a interface Filesystem a /dev/vdb1; o proximo "
              f"subteste mediria a coisa errada:\n{info}"
          )
          assert "HintSystem:" in info, (
              f"o udisks nao publicou HintSystem, que e a propriedade de que "
              f"todo o desenho depende:\n{info}"
          )

      with subtest("sem sessao ativa, montar e recusado com nome — e NAO trava"):
          # WHAT THIS MEASURES, AND WHAT IT DOES NOT.
          #
          # It does not measure the `HintSystem` split: a blank secondary disk in
          # a VM comes out NOT system, and the test driver runs as root, who
          # passes every polkit check. The split itself is measured above, by
          # reading the two `allow_active` values out of the installed policy.
          #
          # What it measures is the other property the whole design leans on:
          # `allow_active` only applies to an ACTIVE local session, and there is
          # no authentication agent anywhere in this distro's graphical session.
          # A caller without one gets `allow_any`, which is `auth_admin` — and
          # WITHOUT `auth.no_user_interaction` polkit would wait for an agent
          # that never registers. On screen that is a row pressed that does
          # nothing, for ever.
          #
          # So the `timeout 20` is half the assertion: it has to come BACK, and
          # it has to come back saying which no it was.
          user = "${nodes.machine.ronoctua.user}"
          machine.succeed(f"loginctl show-user {user} > /dev/null 2>&1 || true")
          refusal = machine.fail(
              f"timeout 20 systemd-run --pipe --wait --uid={user} "
              "--setenv=XDG_RUNTIME_DIR=/run/user/1000 "
              "udisksctl mount --no-user-interaction -b /dev/vdb1 2>&1"
          )
          lowered = refusal.lower()
          # ONE ASSERTION COVERS BOTH, and the first draft used two — the
          # second one matched the word "terminated" out of systemd-run's own
          # summary and failed on a run that had answered in 69 ms.
          #
          # A hang would have been killed by `timeout` above, which prints
          # nothing and carries no error name. So a refusal that NAMES itself
          # is proof that the call came back: `auth.no_user_interaction`
          # reached polkit, and polkit answered instead of waiting for an
          # agent that never registers.
          assert "not authorized" in lowered or "notauthorized" in lowered, (
              f"esperava a recusa NOMEADA do polkit — sem nome, ou a chamada "
              f"travou e o timeout a matou, ou ela nem chegou la:\n{refusal}"
          )

      with subtest("formatar escreve mesmo, e o rotulo volta pela mesma porta"):
          # WHAT THIS PROVES THAT NO UNIT TEST CAN. `ronoctua_disks::format`
          # is three D-Bus calls with a race between them: `Block.Format`
          # writes a partition table, and `PartitionTable` appears on the
          # object through a UEVENT that has not happened yet. A unit test
          # would mock the bus and prove the argument order; this proves that
          # the interface ARRIVES, which is the half that was guessed.
          #
          # The whole disk, so the `whole` path is the one exercised — the
          # partition path is a single call and is the easy one.
          machine.succeed(
              "${formatDisk}/bin/ronoctua-format-disk /dev/vdb vfat PROVAFMT"
          )
          machine.succeed("udevadm settle")
          info = machine.succeed("udisksctl info -b /dev/vdb1")
          assert "PROVAFMT" in info, (
              f"formatar nao escreveu o rotulo pedido em /dev/vdb1:\n{info}"
          )
          # AND THE PARTITION TYPE FOLLOWED THE FILESYSTEM. Without
          # `update-partition-type` an MBR partition holding vfat is typed
          # `0x83`, Linux — the filesystem is right, the byte beside it is
          # wrong, and Windows does not offer to open the stick with nothing
          # anywhere saying why.
          kind = machine.succeed("sfdisk --part-type /dev/vdb 1").strip().lower()
          assert kind in ("b", "c", "0b", "0c"), (
              f"a particao ficou com o tipo {kind!r}, e nao um dos de FAT: "
              "o `update-partition-type` nao chegou ao udisks"
          )

      print("OK: udisks responde, a politica e a do upstream, os hints chegam, "
            "quem nao tem sessao ativa e recusado com nome sem travar, "
            "e formatar escreve o rotulo e o tipo de particao")
    '';
}
