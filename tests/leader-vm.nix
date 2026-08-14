# Leader, which-key and the return to rest — acceptance criterion of milestone 5.
#
# "The leader captures the keyboard via `keyboard-interactivity: exclusive`;
# which-key expands upwards with the list BELOW the minibuffer; `Esc` goes
# back to 2 lines."
#
# WHAT IS VERIFIED ON SCREEN, AND NOT ONLY IN THE LOG
# ---------------------------------------------------
# The surface's HEIGHT. It is the only external sign that the expansion
# happened, and the log does not prove it: the footer may think it expanded
# and the compositor may have refused. We measure it by the footer's band of
# pixels before and after — at rest it is two lines, with the leader active it
# is two plus one per which-key entry.
#
# The keyboard grab is verified through niri's IPC, which exposes each layer's
# `keyboard_interactivity`. That is better than inferring from behavior: it
# says literally what the compositor understood of our request.
#
# HOW THE KEY IS SENT
# -------------------
# By `xdotool`, on Xvfb. niri runs on the winit backend over X (see
# `niri-xvfb.nix`), so a key injected into the X window reaches the compositor
# as a real key and follows the normal path to our surface. There is no
# shortcut: it is the same route as the physical keyboard.
{
  pkgs,
  lib,
  niri,
  ronoctua-shell,
  fonts,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  # THE HEIGHT COMES FROM THE TOKENS, not from a number nailed down here.
  #
  # It used to be `35`, from when the font was 14 px and the line height 1.25.
  # Both changed, and the test started measuring against a value that no longer
  # existed — a test that fails for being out of date teaches people to update
  # the number without reading what it asserts.
  typography = import ../assets/tokens/typography.nix;
  linePx = builtins.ceil (typography.size.base * typography.lineHeight);
  footerPx = builtins.ceil (typography.size.base * typography.lineHeight * 2);
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-leader";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      ronoctua-shell
      pkgs.imagemagick
      pkgs.tesseract
    ]
    ++ xvfb.packages;

    fonts.enableDefaultPackages = false;
    fonts.packages = [ fonts.noctua-iosevka-core ];

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    # The footer's height at rest: two lines (section 4.1), computed from the
    # tokens — see this file's `let`.
    FOOTER = ${toString footerPx}


    def footer_height():
        """How many px from the bottom up differ from niri's background.

        The compositor's background is a flat color (`#404040` by default);
        our surface is another. Counting the lines that differ from it
        measures the surface's REAL height on screen, which is the only proof
        that the expansion was accepted — the log only proves it was asked for.
        """
        w, h, x, y = niri_geometry()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        # A column of pixels well to the left, from the top to the end of the window.
        raw = machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop 1x{h}+2+0 +repage -depth 8 txt:-"
        )
        colors = []
        for line in raw.splitlines()[1:]:
            if "#" in line:
                colors.append(line.split("#")[1][:6])
        background = colors[0] if colors else ""
        height = 0
        for color in reversed(colors):
            if color == background:
                break
            height += 1
        return height

    def wait_for_height(expected, tentativas=20):
        """Wait for the surface to reach the given height, and report the value.

        Two lessons in one helper: `assert x == y` with no message hides the
        value that actually came, and measuring right after the paint log runs
        ahead of the COMPOSITOR — the footer has drawn, niri has not composed yet.
        """
        seen_now = None
        for _ in range(tentativas):
            seen_now = footer_height()
            if seen_now == expected:
                return seen_now
            machine.sleep(1)
        raise AssertionError(
            f"o rodape tem {seen_now} px, esperava {expected} "
            f"(depois de {tentativas}s)"
        )



    def press_key(item_name):
        machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {item_name}")
        machine.sleep(1)


    def layers():
        """What the COMPOSITOR says about our layer surfaces.

        Asking for `exclusive` and getting `exclusive` are different things;
        only its side answers the second one.
        """
        out_text = machine.succeed(f"${ronoctua-shell}/bin/niri-contract {sock}")
        for line in out_text.splitlines():
            if line.startswith("camadas:"):
                return line
        raise AssertionError(f"niri-contract nao listou camadas:\n{out_text}")


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()
    start_niri("niri", "${niri}")
    sock = socket_of("niri")

    with subtest("o rodape sobe em repouso, com duas linhas"):
        machine.succeed(
            "systemd-run --unit=footer ${xvfb.env} "
            "--setenv=WAYLAND_DISPLAY=wayland-1 "
            # pt-BR on purpose, and not the default: that way the list on screen
            # proves the whole chain of invariant 8 — `.ftl` -> generator ->
            # constant -> Fluent -> pixel — in the language that is NOT the fallback.
            "--setenv=LANG=pt_BR.UTF-8 "
            f"--setenv=NIRI_SOCKET={sock} "
            "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_render=debug "
            # THE LEADER'S LABEL comes from the environment, put there by whoever
            # defines the bind. Here it is the test that defines it, and that is why
            # it can assert the exact text — without the variable it would fall back
            # to `SPC` and the test would pass without proving the variable is read.
            "--setenv=RONOCTUA_LEADER=LEADER "
            "${ronoctua-shell}/bin/ronoctua-shell"
        )
        # Wait for the PAINTING, not for the control socket.
        #
        # Both happen at startup, but in an order that depends on how long the
        # process takes to come up — and it changed when the binary gained the
        # WASM runtime. Measuring the height right after the socket measured a
        # surface that had not received a `configure` yet: zero pixels, with no error.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'layout do rodape resolvido'",
            timeout=90,
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'socket de controle aberto'",
            timeout=30,
        )
        at_rest = wait_for_height(FOOTER)
        print(f"altura em repouso: {at_rest} px")

    with subtest("a leader captura o teclado e expande"):
        # Without this, `xdotool` injects keys that reach nowhere at all.
        focus_niri()

        # THE POINTER TO A CORNER THAT DOES NOT GET IN THE WAY — and there are TWO nuisances.
        #
        # 1. The message line started showing the description of the item UNDER THE
        #    CURSOR. The Xvfb pointer is born in the middle of the screen and the
        #    expansion grows upwards close to it: with a long list, the footer would
        #    reach the cursor and the minibuffer would show a description in place of
        #    the pending sequence.
        # 2. The `footer_height` samples COLUMN x=2 and takes the pixel at the top
        #    as "background". With the cursor drawn there, the background becomes the
        #    cursor's color and the arithmetic gives back the whole window — which is
        #    what happened: 775 px of footer on a surface of 288.
        #
        # The upper RIGHT corner, inside the window: far from the sampled column and
        # far from the footer.
        w0, h0, x0, y0 = niri_geometry()
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{x0 + w0 - 5} {y0 + 5}"
        )

        # `ronoctua-shell leader` does NOT bring a footer up: it notifies the one
        # already running. It is the path niri's bind uses (`nix/modules/niri.nix`).
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )

        expanded = footer_height()
        print(f"altura com a leader: {expanded} px")
        # Four groups in the default keymap (f, b, g, q), one line each, below the
        # footer's two.
        assert expanded > FOOTER, (
            f"a leader nao expandiu a superficie: {expanded} px, igual ao repouso"
        )

        journal = machine.succeed("journalctl -u footer --no-pager")
        # THE INVARIANT, and not the count: a nailed-down number breaks on every new
        # command and teaches people to update the test without reading what it says.
        # What matters is that the leader opened with entries ON SCREEN.
        import re as _re
        n = [int(m) for m in _re.findall(r"inputs=(\d+)", journal)]
        assert n and n[0] > 0, f"a leader abriu sem entradas: {n}"
        top_px = n[0]

        # The milestone's literal criterion: the COMPOSITOR reports `exclusive`.
        c = layers()
        print(c)
        assert "ronoctua-footer@Top/Exclusive" in c, c

    with subtest("a lista do which-key aparece na tela"):
        w, h, x, y = niri_geometry()
        alt = footer_height()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            # Only the part BELOW the footer's two lines.
            #
            # THE ORDER CHANGED (the owner's decision): modeline and minibuffer stay
            # GLUED at the top of the footer, and the expansion grows below them —
            # before it grew between the two. Cropping from the old place would read
            # the minibuffer and pass by mistake.
            f"-crop {w}x{alt - FOOTER}+0+{h - alt + FOOTER} +repage "
            "-colorspace gray -negate -resize 400% /tmp/whichkey.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/whichkey.png stdout --psm 6 2>/dev/null"
        ).strip()
        print(f"OCR da lista: {read_back!r}")
        # TRANSLATED TEXT, not a key. If `group-arquivo` shows up here, the label
        # reached the screen raw and invariant 8 is broken at the last link — the
        # only one no unit test covers.
        lowered = read_back.lower()
        assert "buffer" in lowered, (
            f"a lista do which-key nao esta na tela em pt-BR; OCR leu {read_back!r}"
        )
        assert "grupo-" not in lowered, (
            f"o rotulo chegou como CHAVE, nao como texto: {read_back!r}"
        )

    with subtest("uma tecla de prefixo estreita a lista"):
        # `i` and not `b`: `b` today has as many bindings as the root, and
        # "it narrowed" would become a coincidence. What is asserted is that the
        # list became the PREFIX's, and `i` has only one.
        press_key("i")
        journal = machine.succeed("journalctl -u footer --no-pager")
        # What is asserted is that it NARROWED, not a number: how many commands
        # exist at the top changes with the keymap.
        import re as _re
        n = [int(m) for m in _re.findall(r"inputs=(\d+)", journal)]
        assert len(n) >= 2, f"faltou o segundo `entradas=`: {n}"
        assert n[-1] < top_px, f"a lista nao estreitou: {n[-1]} vs {top_px} no topo"

    with subtest("o minibuffer mostra a SEQUENCIA PENDENTE, como no Doom"):
        # The defect this catches: the prefix only showed up in the modeline's mode
        # indicator — a narrow segment that ELIDES. Whoever types a chord looks at
        # the line below, and there was nothing there: pressing the leader and seeing
        # the same screen is indistinguishable from "it did not register the key".
        journal = machine.succeed("journalctl -u footer --no-pager")
        # `LEADER i-`: the label from the environment, and Doom's notation (`-`, and
        # not ` …`). `SPC` here would be the fallback, and would prove the opposite.
        assert "LEADER i-" in journal, journal

        w, h, x, y = niri_geometry()
        alt = footer_height()
        machine.succeed("${pkgs.xwd}/bin/xwd -display :99 -root -out /tmp/tela.xwd")
        # THE FOOTER'S SECOND LINE, counting from its top — and not the last one
        # on the screen.
        #
        # The minibuffer became GLUED to the modeline (the owner's decision), and
        # the expansion moved below the two. With the list open, the screen's last
        # line is the LIST's last line; reading there would find a shortcut label
        # and the test would pass for the wrong reason.
        machine.succeed(
            f"${pkgs.imagemagick}/bin/magick /tmp/tela.xwd -crop {w}x{h}+{x}+{y} +repage "
            f"-crop {w}x${toString linePx}+0+{h - alt + ${toString linePx}} +repage "
            "-colorspace gray -negate -resize 400% /tmp/mini.png"
        )
        read_back = machine.succeed(
            "${pkgs.tesseract}/bin/tesseract /tmp/mini.png stdout --psm 7 2>/dev/null"
        ).strip()
        print(f"OCR do minibuffer: {read_back!r}")
        assert "leader" in read_back.lower(), (
            f"a sequencia pendente nao esta na linha do minibuffer; OCR leu {read_back!r}"
        )

    with subtest("tecla que nao leva a lugar nenhum DIZ isso"):
        # Before: it ended silently. The user had no way of telling "that
        # combination does not exist" from "the key did not arrive".
        press_key("z")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'area de eco'", timeout=30
        )
        journal = machine.succeed("journalctl -u footer --no-pager")
        echo_line = [l for l in journal.splitlines() if "area de eco" in l][-1]
        print(f"eco: {echo_line}")
        # THE MESSAGE cites the whole sequence, not only the last key:
        # `LEADER i z` is what the user typed, and `z` on its own would not help.
        assert "LEADER i z" in echo_line, echo_line
        # And in pt-BR, translated — not the raw key.
        assert "eco-" not in echo_line, echo_line

    with subtest("Esc volta ao repouso, de duas linhas"):
        press_key("Escape")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader encerrada'", timeout=30
        )
        # And it gives the keyboard back: without this no buffer would receive a key
        # again, and the system would go mute after the first leader.
        c = layers()
        print(c)
        assert "ronoctua-footer@Top/None" in c, c

        came_back = wait_for_height(FOOTER)
        print(f"altura depois do Esc: {came_back} px")

    with subtest("a expansao PARA de crescer, e o resto vira pagina"):
        # INVARIANT 14 IN PIXELS. The `M-x` list has more commands than fit, and with
        # no ceiling it would grow with the registry: every new command would push the
        # footer further up, until it stopped being a footer.
        #
        # What is asserted is the CRITERION — "the footer does not take the screen" —,
        # and not the token's number: repeating the formula here would give a test
        # that agrees with the code by construction and verifies nothing.
        press_key("Escape")
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )
        # `colon` is the name of the `:` keysym in xdotool. The key changed from `x`
        # to `:` — Doom's `M-x` one —, and `x` there is the scratch buffer.
        press_key("colon")  # the finder: the longest list in the system
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'command=\"mx-open\"'",
            timeout=30,
        )
        machine.sleep(2)
        _, h, _, _ = niri_geometry()
        with_mx = footer_height()
        print(f"altura com o M-x aberto: {with_mx} px de {h} px de tela")
        assert with_mx > FOOTER, "o M-x nao expandiu"
        assert with_mx <= h / 2, (
            f"a expansao pegou {with_mx} px de {h} px — mais de metade da tela"
        )
        # THE MOUSE WHEEL SCROLLS THE REGION — with the `M-x` open, which is the
        # scrollbar's case (a long list, with no page counter).
        #
        # `click 4`/`click 5` are the wheel-up and wheel-down buttons on X. What is
        # asserted is the OFFSET, which is the number the bar and the counter come
        # out of: a test of the handler would say that the event arrived, and not
        # that the screen moved.
        def offset():
            import re as _re

            journal = machine.succeed("journalctl -u footer --no-pager")
            seen = _re.findall(r"offset=(\d+)", journal)
            assert seen, "o rodape nunca registrou deslocamento"
            return int(seen[-1])

        # THE POINTER HAS TO BE OVER THE FOOTER.
        #
        # On Wayland the axis event goes to the surface UNDER the cursor, and it is
        # parked in the upper right corner (see the comment up there, about
        # `footer_height`). Without moving it, the wheel would scroll the buffer
        # behind and the test would start measuring something else — or nothing.
        wv, hv, xv, yv = niri_geometry()
        footer_band = footer_height()
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{xv + wv // 2} {yv + hv - footer_band // 2}"
        )
        machine.sleep(1)

        prior = offset()
        for _ in range(3):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool click 5")
            machine.sleep(1)
        machine.sleep(1)
        after = offset()
        assert after > prior, (
            f"a roda para baixo nao rolou a regiao: deslocamento {prior} -> {after}"
        )

        # AND UP GOES BACK. Without this, a swapped sign would pass: scrolling both
        # ways would give the same error in half the cases.
        for _ in range(3):
            machine.succeed("DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool click 4")
            machine.sleep(1)
        machine.sleep(1)
        came_back = offset()
        assert came_back < after, (
            f"a roda para cima nao voltou: deslocamento {after} -> {came_back}"
        )

        # IT PUTS THE CURSOR BACK IN THE CORNER. Leaving it over the footer would
        # spoil the `footer_height` of the following subtests, for the reason the
        # parking comment explains.
        machine.succeed(
            "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool mousemove "
            f"{xv + wv - 5} {yv + 5}"
        )
        machine.sleep(1)

        press_key("Escape")
        wait_for_height(FOOTER)

    with subtest("uma sequencia completa dispara o COMANDO"):
        # Section 13 wants the key to reach a named command, not an anonymous
        # function. The id in the log is the proof that the whole path — key, tree,
        # registry — closed.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )
        press_key("i")
        press_key("n")
        # The QUOTES are part of it: `tracing` formats a `Debug` value between
        # quotes, and `command=glyphs-nerd-fonts` matches nothing.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'command=\"glyphs-nerd-fonts\"'",
            timeout=30,
        )
        # And the keyboard goes back to the buffers on its own after running.
        #
        # Through the COMPOSITOR, and not through `wait_for_height`: that helper
        # measures by counting pixels that differ from the background, and from here
        # on there is a WINDOW covering the screen — the "background" it samples
        # becomes the buffer, and the arithmetic gives zero. The question that
        # matters is another one: did the keyboard come back? Only niri answers that.
        c = layers()
        print(c)
        assert "ronoctua-footer@Top/None" in c, c

    with subtest("o comando ABRE o navegador de glifos de verdade"):
        # THE DEFECT THIS CATCHES: five commands showed up in which-key, "ran", and
        # fell into a log `warn`. On screen, nothing. Firing the command proves
        # nothing — what proves it is the WINDOW existing.
        try:
            machine.wait_until_succeeds(
                f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'janelas: 1'",
                timeout=90,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -30")[1])
            print(machine.execute("systemctl list-units 'ronoctua-buffer-*' --no-pager")[1])
            raise
        # And the `warn` for a command with no action must NOT have shown up: it is
        # the exact symptom of the defect.
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert "comando sem acao registrada" not in journal, journal

    with subtest("`i u` abre o navegador de SIMBOLOS, e e outra janela"):
        # THE DEFECT THIS CATCHES is the same as the subtest above, and that is why
        # it exists again for the second buffer: a command can show up in which-key,
        # "run", and fall into a log `warn`. On screen, nothing.
        #
        # The assertion is about the COUNT: `i n` has already opened a window, so
        # after `i u` there have to be TWO. Waiting for "a window exists" would pass
        # with the second command doing nothing.
        machine.succeed(
            "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
            "${ronoctua-shell}/bin/ronoctua-shell leader"
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
        )
        press_key("i")
        press_key("u")
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep -q 'command=\"unicode-symbols\"'",
            timeout=30,
        )
        try:
            machine.wait_until_succeeds(
                f"${ronoctua-shell}/bin/niri-contract {sock} | grep -q 'janelas: 2'",
                timeout=90,
            )
        except Exception:
            print(machine.execute("journalctl -u footer --no-pager | tail -30")[1])
            raise
        journal = machine.succeed("journalctl -u footer --no-pager")
        assert "comando sem acao registrada" not in journal, journal

    with subtest("com o Alt SEGURADO a sequencia continua funcionando"):
        # THE DEFECT THIS CATCHES, and that only shows up in real use.
        #
        # In the nested session the leader IS `Ctrl+Alt+Space`: when the sequence starts,
        # Ctrl and Alt are pressed because the hand has just pressed them. Every key came out
        # with the modifier along, nothing matched, and the screen said
        # `CTRL ALT SPC M-n is not bound` — which names the consequence and hides the
        # cause. The same state happens WITHOUT the hand: leaving for a host shortcut
        # and coming back makes the nested niri miss the Alt release and announce it
        # forever.
        #
        # The normal `tecla()` does not catch this: `xdotool key` releases the key,
        # and between one and the next the Alt has already gone up. Here it stays
        # down the whole time, which is the only way of reproducing what the owner saw.
        def xdo(*args):
            machine.succeed(
                "DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool " + " ".join(args)
            )

        machine.succeed("journalctl --rotate && journalctl --vacuum-time=1s")
        xdo("keydown", "alt")
        try:
            machine.succeed(
                "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
                "${ronoctua-shell}/bin/ronoctua-shell leader"
            )
            machine.wait_until_succeeds(
                "journalctl -u footer --no-pager | grep -q 'leader ativa'", timeout=30
            )
            xdo("key", "i")
            machine.sleep(1)
        finally:
            # In the `finally` on purpose: an Alt stuck in Xvfb would spoil any
            # subtest that came afterwards, including in a future where this is
            # no longer the last one.
            xdo("keyup", "alt")

        journal = machine.succeed("journalctl -u footer --no-pager")
        # THE PROOF: `i` opened the group. With the defect, the log carries
        # `nao esta ligado` and the pending sequence never reaches `LEADER i-`.
        assert "LEADER i-" in journal, (
            "com o Alt segurado, `LEADER i` nao abriu o grupo de icones.\n"
            "A tecla saiu como `M-i` e nao casou com nada.\n\n" + journal
        )
  '';
}
