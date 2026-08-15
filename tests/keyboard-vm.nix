# The leader is the Super key on its own — §7.4.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# `keyd` sits BETWEEN the keyboard and everything else. An error in it is a
# keyboard that does not type — it is the most dangerous piece of the system
# per line of code, and the only one whose failure mode takes the machine
# down for the user.
#
# An evaluation test would prove that `settings.main.leftmeta` is written as
# expected. That is not what matters: what matters is the service COMING UP,
# `keyd` ACCEPTING the file we generate and niri having the bind for the key
# it emits. All three can fail with a perfect config.
#
# WHAT IT DOES NOT PROVE
# ----------------------
# That really touching Super fires the leader. That would require injecting
# an event into evdev below keyd — and what would be under test there would
# be keyd, not our system. The contract that is ours is: the service is
# standing, the remapping is what we asked for, and niri listens for the key
# that comes out of it.
{
  pkgs,
  lib,
  ronoctua,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-keyboard";

  nodes.machine =
    { config, ... }:
    {
      # ALL THREE modules: the test checks both ends of the same contract — the
      # key keyd emits and the one niri listens for. With only one, each half
      # would pass on its own while the pair was broken.
      #
      # The `base.nix` goes in because the safety net depends on it: the polkit
      # rule allows `config.ronoctua.user`, and without the real user the test
      # would prove that root restarts a service — which is not the question.
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/keyboard.nix
        ../nix/modules/niri.nix
      ];
      _module.args.ronoctua = ronoctua;
      virtualisation.memorySize = 2048;
      # The test framework names the node `machine`, and `base.nix` requires the
      # `noctua-` prefix (section 2). The two definitions collide at equal
      # priority; the `mkForce` decides, and the prefix goes on being asserted.
      networking.hostName = lib.mkForce "noctua-teclado";
      # No /persist in this VM: `base.nix` points the password hash there, and
      # the test does not log in — it uses `machine.succeed` as root and `sudo -u`.
      #
      # A single attrset: a dynamic attribute in two sentences is an evaluation
      # error ("dynamic attribute already defined") — see the CLAUDE.md.
      users.users.${config.ronoctua.user} = {
        hashedPasswordFile = lib.mkForce null;
        password = "";
      };

      # TWO SOURCES FOR ROOT'S PASSWORD, and NixOS warns with good reason.
      #
      # The `base.nix` defines `hashedPassword = "!"` — root with no login, which
      # is the decision of section 9. The NixOS test framework defines its own so
      # the driver can get in. With both non-null, it prints
      # "the user 'root' has multiple of the options … set to a non-null
      # value", and which of them wins depends on the internal order of the users
      # module: exactly the kind of thing that changes in an update with nobody
      # noticing.
      #
      # Here `base.nix` yields: it is the test that needs to get in, and the
      # production decision stays intact in its own module.
      users.users.root.hashedPassword = lib.mkForce null;
    };

  # `{ nodes, ... }`: the user's name comes from `config.ronoctua.user`, which
  # is the only place where it exists (invariant 16). Written by hand here, the
  # test would start proving that "noctua" restarts keyd even after the
  # system's user changed name.
  testScript =
    { nodes, ... }:
    ''
      user_name = "${nodes.machine.ronoctua.user}"

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("o keyd sobe"):
          # `wait_for_unit` and not `is-active`: the service comes up in parallel with
          # the rest of the boot, and asking too early answers "activating".
          machine.wait_for_unit("keyd.service")

      with subtest("o remapeamento e o que pedimos"):
          # THE FILE KEYD READS, and not the Nix attribute.
          #
          # Between one and the other there is the NixOS module: an option name
          # changed in a new version would generate a different file with no error
          # at all, and the symptom would be the leader stopping working.
          conf = machine.succeed("cat /etc/keyd/*.conf")
          print(conf)
          assert "overload(meta, f13)" in conf, conf
          assert "leftmeta" in conf, conf

      with subtest("o keyd ACEITA o arquivo e CASA um teclado"):
          # Writing a valid config in Nix and writing a config keyd understands are
          # different things. If it refused, the service would stay standing without
          # remapping anything — and nothing in systemd's journal would say so.
          #
          # The POSITIVE signal is the one that counts: `DEVICE: match` proves that
          # it caught a real keyboard. Looking only for error words would pass with
          # a keyd that came up, read the file and matched no device at all — which
          # is silent and useless.
          journal = machine.succeed("journalctl -u keyd --no-pager")
          print(journal)
          assert "DEVICE: match" in journal, journal
          assert "all.conf" in journal, journal
          for bad in ["error", "invalid", "syntax"]:
              assert bad not in journal.lower(), f"o keyd reclamou: {journal}"

      with subtest("ele nao mexe em NENHUMA outra tecla"):
          # What limits the damage of a mistake here: one key remapped, all the
          # others passing straight through. One more line in the `[main]` section is
          # one key that stops working the way the person expects.
          main = conf.split("[main]")[1]
          lines = [l.strip() for l in main.splitlines() if l.strip() and "=" in l]
          assert len(lines) == 1, f"o keyd remapeia mais de uma tecla: {lines}"

      with subtest("o niri escuta a tecla que o keyd emite"):
          # The two ends of the same contract: keyd emits `f13`, niri binds `F13` to
          # the leader. If one changes without the other, touching Super does nothing
          # — with no error, anywhere.
          kdl = machine.succeed("cat /etc/niri/config.kdl")
          assert "F13 repeat=false { spawn" in kdl, kdl
          # And a HELD Super goes on being a modifier in the chords.
          assert "Mod+Left { focus-column-left; }" in kdl, kdl

      with subtest("as opcoes de xkb chegam ao compositor da SESSAO"):
          # `keyboard-options-do-what-they-say` compiles that same string and
          # reads the symbols back — what is missing there, and only a booted
          # system answers, is whether the string SURVIVES the module: it is
          # built from `ronoctua.keyboard.compose` plus what the system adds,
          # and an option that never reaches the config is an accent key that
          # does not accent, with the keymap perfectly correct one layer below.
          assert 'options "${nodes.machine.ronoctua.keyboard.options}"' in kdl, (
              "as opcoes de xkb nao chegaram ao config do niri:\n" + kdl
          )

      with subtest("o rodape sobe como servico da sessao, por caminho do store"):
          # IT DID NOT COME UP. The binary was not even installed, and the leader's
          # bind spawned the NAME `ronoctua-shell` — in production the whole system
          # was left with no modeline, no minibuffer and no leader, and no check
          # reported it because none of them brings the session up for real.
          #
          # Here there is no graphical session (there is no GPU and no seat), so what
          # is asserted is what the configuration PROMISES: the unit exists, it points
          # at a store path, and so does the bind.
          unit = machine.succeed(
              "systemctl --user cat ronoctua-footer.service --no-pager "
              "|| cat /etc/systemd/user/ronoctua-footer.service"
          )
          print(unit)
          assert "/nix/store/" in unit, unit
          assert "bin/ronoctua-shell" in unit, unit
          assert 'spawn "/nix/store/' in kdl, (
              "o bind da leader spawna um NOME; o PATH de uma unidade e minimo "
              f"e o nome nao resolve:\n{kdl}"
          )

      with subtest("os binds de emergencia existem, e FORA da leader"):
          # They only serve for when the leader does not answer. With `Mod+` in
          # front, they would be the key locked inside the safe.
          for press_key in ["Ctrl+Alt+L", "Ctrl+Shift+L", "Ctrl+Alt+Delete"]:
              assert f"\n    {press_key} " in kdl, f"falta o bind {press_key}:\n{kdl}"
              assert f"Mod+{press_key}" not in kdl, f"{press_key} saiu com o prefixo da leader"
          assert "keyboard-restore" in kdl, kdl

      with subtest("com o keyd DERRUBADO, o USUARIO o levanta de volta"):
          # THE DESTRUCTIVE TEST. It is the only proof that the net catches: without
          # it, "there is a way back" is a claim.
          #
          # And it is not enough for the polkit rule to exist in the file — polkit is
          # a daemon that interprets it, and a wrong condition (`verb`, `unit`, user)
          # gives "Interactive authentication required", which on a screen with no
          # trustworthy keyboard is the same as having no way out at all.
          #
          # No `wait_for_unit("polkit.service")`: it is activated by D-Bus, and until
          # someone asks for authorization the unit is `inactive` — whoever waits for
          # it waits forever. What wakes it is the `systemctl` right below, which is
          # precisely what we want to exercise.
          prior = machine.succeed(
              "journalctl -u keyd --no-pager | grep -c 'DEVICE: match' || true"
          )

          machine.succeed("systemctl stop keyd.service")
          machine.fail("systemctl is-active --quiet keyd.service")

          # AS THE USER, and with no password. It is what `keyboard-restore` does
          # underneath; the `Ctrl+Shift+L` only calls it through the footer's socket,
          # which in this VM has no graphical session to come up.
          machine.succeed(f"su - {user_name} -c 'systemctl restart keyd.service'")
          machine.wait_for_unit("keyd.service")

          # And it came back REMAPPING, not merely standing: a keyd that comes up
          # without matching a device is silent and useless — indistinguishable from
          # success.
          #
          # Counting what SHOWED UP AFTERWARDS: the boot's lines are still in the
          # journal, and looking for them would pass with the service stopped.
          machine.wait_until_succeeds(
              "test $(journalctl -u keyd --no-pager | grep -c 'DEVICE: match') "
              f"-gt {int(prior.strip() or 0)}",
              timeout=30,
          )
    '';
}
