# The file manager's PRIVILEGED half — and the refusals, which are the point.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The listing, the menu, the picker, the columns and every operation on one's
# own files are pure and are tested where they live: `ronoctua-files` has
# sixty-five unit tests against a fake tree, and `files::View` has fifteen more
# against a real temporary one. None of that needs a machine.
#
# What needs a machine is the door to root. Four things, and a booted system is
# the only thing that answers any of them:
#
#   1. the unit EXISTS and runs the extracted binary — a `systemd.services`
#      entry that does not evaluate is a `chown` that fails in front of the
#      person with "Unit not found";
#   2. the polkit rule NAMES it, and names nothing else. A rule is JavaScript
#      that polkit parses at runtime: a syntax error in it is silent, and the
#      symptom is every privileged action being refused;
#   3. the staging directory is the PERSON's. `systemd-tmpfiles` writes one line
#      at boot and carries on — the rescue console already paid for a rule that
#      named a group which did not exist, and the failure surfaced far away as
#      an edit that saved and installed nothing;
#   4. and the helper REFUSES what it should. A gate that has never refused
#      anything has not been tested, and this one guards a program that runs as
#      root.
{
  pkgs,
  lib,
  ronoctua,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-files";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/files.nix
      ];
      _module.args.ronoctua = ronoctua;
      virtualisation.memorySize = 2048;
      networking.hostName = lib.mkForce "noctua-arquivos";
      # No /persist in this VM — the same reason as the keyboard's and the
      # rescue's: `base.nix` points the password hash there and the test never
      # logs in.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };
      users.users.root.hashedPassword = lib.mkForce null;
    };

  # `{ nodes, ... }`: the user's name lives in `config.ronoctua.user` and
  # nowhere else (invariant 16). Written by hand here, this test would go on
  # passing after the system's user changed name.
  testScript =
    { nodes, ... }:
    ''
      user_name = "${nodes.machine.ronoctua.user}"
      helper = "${ronoctua.filesRoot}/bin/ronoctua-files-root"

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("a unidade privilegiada existe e roda o binario extraido"):
          machine.succeed("systemctl cat ronoctua-files-root.service > /dev/null")
          started = machine.succeed(
              "systemctl show ronoctua-files-root.service -p ExecStart --value"
          )
          assert helper in started, (
              f"a unidade nao roda o binario extraido:\n{started}"
          )

      with subtest("e o binario NAO carrega o desktop junto"):
          # It runs as root. Handing it the whole workspace would put wgpu, the
          # wasm runtime, the renderer and its three embedded fonts inside a
          # unit that runs as root — a hundred megabytes to perform four
          # syscalls.
          closure = machine.succeed(f"nix-store -qR {helper} | wc -l").strip()
          for unwanted in ["wgpu", "wasmtime", "noctua-iosevka", "mesa"]:
              found = machine.succeed(f"nix-store -qR {helper} | grep -c -- '{unwanted}' || true")
              assert found.strip() == "0", (
                  f"o ajudante privilegiado carrega {unwanted}"
              )
          print(f"{closure} caminhos no ajudante, e nenhum e do desktop")

      with subtest("o diretorio onde o pedido e escrito e da pessoa"):
          # AND THE BOOT DID NOT COMPLAIN. `systemd-tmpfiles` writes one line
          # and carries on: a rule naming a group that does not exist leaves the
          # directory uncreated, and the failure shows up much later as an
          # operation that says nothing and does nothing.
          boot = machine.succeed("journalctl -b -u systemd-tmpfiles-setup --no-pager")
          assert "ronoctua-files" not in boot, (
              "o tmpfiles reclamou da nossa regra:\n" + boot
          )
          owner = machine.succeed("stat -c %U /run/ronoctua-files/staged").strip()
          assert owner == user_name, (
              f"/run/ronoctua-files/staged e de `{owner}`; o gerenciador roda "
              "como a pessoa e nao conseguiria escrever"
          )
          mode = machine.succeed("stat -c %a /run/ronoctua-files/staged").strip()
          assert mode == "700", (
              f"o diretorio esta {mode}: outra pessoa da maquina poderia deixar "
              "um pedido para uma unidade que roda como root"
          )

      with subtest("a regra polkit nomeia a unidade, o verbo e o usuario"):
          # polkit rules are JavaScript parsed at RUNTIME: a syntax error is
          # silent, and the symptom is every privileged action being refused
          # with nothing saying why. Reading it back from the machine is what
          # proves it was installed and parsed.
          rules = machine.succeed(
              "grep -Rl ronoctua-files-root /etc/polkit-1/rules.d/ | head -1"
          ).strip()
          body = machine.succeed(f"cat {rules}")
          assert 'action.lookup("verb") == "start"' in body, body
          assert f'subject.user == "{user_name}"' in body, body
          assert "AUTH_SELF_KEEP" in body, body
          machine.succeed("systemctl status polkit.service > /dev/null")

      with subtest("o ajudante APLICA o que lhe pedem"):
          machine.succeed("mkdir -p /var/teste && install -m 0644 /dev/null /var/teste/um.txt")
          machine.succeed("printf chown > /run/ronoctua-files/staged/verb")
          machine.succeed("printf /var/teste/um.txt > /run/ronoctua-files/staged/path")
          machine.succeed(f"id -u {user_name} > /run/ronoctua-files/staged/value")
          machine.succeed("systemctl start ronoctua-files-root.service")
          became = machine.succeed("stat -c %U /var/teste/um.txt").strip()
          assert became == user_name, f"o dono ficou `{became}`"

      with subtest("e consome o pedido, para que nao seja repetido"):
          # A request left behind would be replayed by the next
          # `systemctl start` — from anywhere — which turns one authorisation
          # into a standing one.
          left = machine.succeed("ls /run/ronoctua-files/staged | wc -l").strip()
          assert left == "0", f"sobraram {left} arquivos do pedido"

      with subtest("um link simbolico e RECUSADO, e nao seguido"):
          # The sharp one: the person is shown a link, and a swap between the
          # click and the unit makes root act on whatever it points at.
          machine.succeed("install -m 0600 /dev/null /var/teste/alvo")
          machine.succeed("ln -s /var/teste/alvo /var/teste/atalho")
          machine.succeed("printf chmod > /run/ronoctua-files/staged/verb")
          machine.succeed("printf /var/teste/atalho > /run/ronoctua-files/staged/path")
          machine.succeed("printf 777 > /run/ronoctua-files/staged/value")
          machine.fail("systemctl start ronoctua-files-root.service")
          mode = machine.succeed("stat -c %a /var/teste/alvo").strip()
          assert mode == "600", (
              f"o alvo do link virou {mode}: a unidade seguiu o link"
          )

      with subtest("e o store e os pseudo-sistemas tambem"):
          for path in ["/proc/sys/kernel/hostname", "/sys/kernel/vmcoreinfo"]:
              machine.succeed("printf chmod > /run/ronoctua-files/staged/verb")
              machine.succeed(f"printf {path} > /run/ronoctua-files/staged/path")
              machine.succeed("printf 777 > /run/ronoctua-files/staged/value")
              machine.fail("systemctl start ronoctua-files-root.service")

      with subtest("onde o gerenciador abre vem da opcao, e nao de um literal"):
          # A directory somebody keeps their work in is a preference of the
          # MACHINE (invariant 13). Baked into the Rust it would be personal
          # data in a public repository.
          where = machine.succeed(
              "systemctl show-environment | grep RONOCTUA_FILES_START || "
              "grep -R RONOCTUA_FILES_START /etc/set-environment"
          )
          assert "${nodes.machine.ronoctua.files.start}" in where, where
    '';
}
