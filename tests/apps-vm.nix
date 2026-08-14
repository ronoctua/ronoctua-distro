# The app finder, proved ON SCREEN and on the clock.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# Three things, and the last one is the reason this file exists:
#
#   1. that the sweep happens AT STARTUP, and not on pressing the key. By unit
#      test one can prove that it works; that it has already happened when the
#      list opens depends on the real event loop;
#   2. that the application's name reaches the PIXEL — catalogue -> expansion
#      -> span -> screen. The whole chain, closed by OCR;
#   3. that the footer GOES ON RESPONDING after opening an application.
#
# The third is a defect that existed before this feature and that the feature
# would make central: `systemd-run --scope` is SYNCHRONOUS — from the manual,
# "execution in this case is synchronous, and will return only when the
# command finishes". With the `calloop` on a single thread, waiting for it
# froze keyboard, painting and niri events for the application's whole
# lifetime.
#
# The test application sleeps TWO MINUTES on purpose. With the defect, the
# footer goes mute for those two minutes and this check blows the `timeout`;
# without it, the next key is served in milliseconds. There is no way for the
# check to pass by accident.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
  # A NAME WITH NO ACCENT AND NO OCR AMBIGUITY. Tesseract confuses `l`/`I` and
  # eats accents in a small font; what is proved here is that the name reaches
  # the pixel, and not the quality of the OCR.
  item_name = "Adormecido";
  # TWO MINUTES. See the header: it is what separates "it did not freeze" from
  # "the test waited long enough".
  sleepSecs = 120;
  # THE PATTERN WITH A BRACKET, and not a raw `sleep ${toString sleepSecs}`.
  #
  # The `pgrep -f` matches the command line of EVERY process — including that of
  # the shell running the `pgrep` itself, whose line contains the pattern.
  # Without the bracket, `pgrep -f 'sleep 120'` finds itself and gives back
  # success even with the application dead: the assertion "it is still alive"
  # was EMPTY, and an empty check is worse than none. The `[s]` matches `s` and
  # does not match `[s]`.
  sleepPattern = "[s]leep ${toString sleepSecs}";
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-apps";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
      pkgs.coreutils
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    def leader():
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )


    def leaders_in_log():
        return int(
            machine.succeed(
                "journalctl -u footer --no-pager | grep -c 'leader ativa' || true"
            ).strip()
            or 0
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("o `.desktop` de teste existe ANTES de o rodape subir"):
        # The sweep is at startup: a file written afterwards would not go into
        # this round, and the test would start measuring the re-sweep instead of
        # the main path.
        machine.succeed("mkdir -p /tmp/apps/applications")
        machine.succeed(
            "cat > /tmp/apps/applications/adormecido.desktop <<'EOF'\n"
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=${item_name}\n"
            "Comment=Fica parado por ${toString sleepSecs} segundos\n"
            "Exec=${pkgs.coreutils}/bin/sleep ${toString sleepSecs}\n"
            "Categories=Utility;\n"
            "[Desktop Action outra]\n"
            "Name=NaoDeviaAparecer\n"
            "Exec=/bin/false\n"
            "EOF"
        )
        # And one that must NOT show up: `NoDisplay` is what takes 38% of the list out.
        machine.succeed(
            "cat > /tmp/apps/applications/escondido.desktop <<'EOF'\n"
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=NaoDeviaAparecer\n"
            "Exec=/bin/false\n"
            "NoDisplay=true\n"
            "EOF"
        )

    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("o rodape sobe e VARRE na partida"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            "--setenv=LANG=pt_BR.UTF-8 "
            "--setenv=XDG_DATA_DIRS=/tmp/apps "
            "--setenv=XDG_DATA_HOME=/tmp/vazio "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug "
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'socket de controle aberto'",
            timeout=90,
        )
        # THE SWEEP IS THE STARTUP ONE, and the log carries the count and the
        # time. If it only happened on opening, this line would not exist yet.
        journal = machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'catalogo de aplicativos varrido'",
            timeout=60,
        )
        print(journal.strip())
        assert "apps=1" in journal, (
            "a varredura devia achar UM aplicativo — o `NoDisplay` e as acoes\n"
            f"do desktop entry nao podem entrar. Log:\n{journal}"
        )
        focus_niri()

    with subtest("`LEADER a` abre com a lista JA PRONTA"):
        leader()
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )
        press_key("a")
        journal = machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'localizador aberto'", timeout=30
        )
        print(journal.strip())
        assert "apps=1" in journal, (
            f"o localizador abriu com o catalogo vazio; log:\n{journal}"
        )

    with subtest("o nome do aplicativo esta NA TELA"):
        # It closes the chain: catalogue -> expansion -> span -> pixel. A test
        # that only looked at the log would not tell "it found" from "it draws".
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            "-colorspace gray -negate -resize 200% /tmp/apps.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/apps.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR: {read_back!r}")
        assert "dormecido" in read_back.lower(), (
            f"o nome do aplicativo nao esta na tela; OCR leu {read_back!r}"
        )
        assert "naodeviaaparecer" not in read_back.lower().replace(" ", ""), (
            f"a acao do desktop entry ou o `NoDisplay` vazaram para a lista; OCR: {read_back!r}"
        )

    with subtest("digitar FILTRA, e o Enter abre o aplicativo"):
        # `dorm` matches by subsequence inside the name, which is the search the
        # finder offers.
        for c in "dorm":
            press_key(c)
        prior = leaders_in_log()
        press_key("Return")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'abrindo aplicativo'", timeout=30
        )
        machine.wait_until_succeeds(
            "pgrep -f '${sleepPattern}' >/dev/null", timeout=30
        )

    with subtest("E O RODAPE CONTINUA RESPONDENDO — o achado"):
        # HERE IS THE CHECK. With the synchronous `.status()` in the event loop,
        # the footer would go mute for the `sleep`'s ${toString sleepSecs} seconds, and
        # this block would blow the 20 s timeout without ever seeing the second leader.
        #
        # The application stays ALIVE while this runs: it is that simultaneity
        # that the defect made impossible.
        leader()
        machine.wait_until_succeeds(
            f"test $(journalctl -u footer --no-pager | grep -c 'leader ativa') -gt {prior}",
            timeout=20,
        )
        machine.succeed("pgrep -f '${sleepPattern}' >/dev/null")
        print("o rodape respondeu com o aplicativo ainda aberto")

    with subtest("o que subimos, encerramos"):
        machine.succeed("pkill -f '${sleepPattern}' || true")
  '';
}
