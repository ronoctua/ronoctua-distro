# Verification — section 7.12.
#
# "`nix flake check` is the single verification command."
#
# Every milestone is a named check (m1..m8), so "the command that proves the
# milestone passed" is uniform:  nix build .#checks.x86_64-linux.mN
#
# THERE IS NO STUB HERE, and that is deliberate.
#
# There was: a `mkStub` that printed "NOT ATTACKED YET" and did `touch $out`.
# The intention was good — the plumbing standing from day one. The effect was
# that `m7` stayed a stub after milestone 7 had been done, and
# `nix build .#checks.x86_64-linux.m7` answered OK without verifying anything.
# The four real checks existed and were loose.
#
# The lesson: a placeholder that PASSES is indistinguishable from a closed
# milestone to whoever reads the output. A new milestone writes a real check,
# or it does not go in.
#
# The second lesson, a costlier one: write the check against the CRITERION,
# never against what you have just built. `m3` verified that `nix run .#dev`
# "exists and is executable" — and the criterion asked for hot reload, which
# did not exist. See `reload-reloads`.
{
  pkgs,
  lib,
  self,
  inputs,
  system,
  fonts,
  fontconfigConf,
  glyphdb,
  unicodedb,
  lucidedb,
  iconSuperpack,
  rust,
  buffers,
  sbKeys,
  devApp,
  reloadApp,
  singleFooter,
  wit,
  tokensRs,
  niriActions,
}:

let
  # -------------------------------------------------------------------
  # Helpers
  # -------------------------------------------------------------------
  # The keyboard layout, from the same file the configs read.
  keyboard = import ../nix/keyboard-xkb.nix;

  # The software rasterizer, from the SAME place the VM tests use — otherwise
  # the two could point at different ICDs and only one of them would be the one
  # the CI actually exercises.
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };

  py = pkgs.python3.withPackages (ps: [
    ps.fonttools
    ps.brotli
  ]);

  # A check that runs a script and passes if it exits 0.
  mkCheck =
    name: deps: script:
    pkgs.runCommand "check-${name}" { nativeBuildInputs = deps; } ''
      set -euo pipefail
      ${script}
      touch $out
    '';

  # VM test of the footer: it proves that it DRAWS, not only that it compiles.
  # Under lavapipe (section 9.3 of the plan) — correctness, never performance.
  footerVm = import ./footer-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The modeline reflects the focused buffer, proved ON SCREEN (milestone 4).
  modelineVm = import ./modeline-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The default keymap closes: no collision and every shortcut pointing at a
  # command that exists. The binary is the same one a person runs by hand.
  keymap-no-collision = mkCheck "keymap-no-collision" [ ] ''
    ${rust.workspace}/bin/ronoctua-shell check-keymap
  '';

  # Both languages have exactly the same keys, none of them empty. It is the
  # generator that runs in the build; here it runs again as a named check.
  i18n-complete = mkCheck "i18n-complete" [ pkgs.python3 ] ''
    python3 ${../nix/scripts/gen-i18n.py} ${../assets/i18n} $TMPDIR/keys.rs
  '';

  # THE NESTED SESSION'S `FONTCONFIG_FILE` RESOLVES A REAL FONT.
  #
  # # What this catches, and how it showed up
  #
  # `FONTCONFIG_FILE` replaces the WHOLE fontconfig configuration. The
  # `nix/dev.nix` pointed at the FRAGMENT — only the alias rules, with no
  # `<dir>` at all —, and fontconfig was left with no font directory: zero
  # fonts, and everything that goes through it becomes TOFU.
  #
  # The symptom was an error message from niri in little boxes on the screen.
  # Precisely the message that says what is wrong — the cruelest failure mode
  # there is, because it wipes the information you need exactly when you need it.
  #
  # Our renderer does not go through here (section 6.4), so the footer looked
  # normal while the compositor's warning was illegible: two chains, and only
  # one broken. No check looked at the second.
  fontconfig-nested-finds-font = mkCheck "fontconfig-nested-finds-font" [ pkgs.fontconfig.bin ] ''
    export FONTCONFIG_FILE=${fontconfigConf.fullConf}
    export XDG_CACHE_HOME=$TMPDIR/cache

    # ZERO FONTS IS THE DEFECT. An empty `fc-list` means that every text that
    # goes through fontconfig comes out in little boxes.
    n=$(fc-list 2>/dev/null | wc -l)
    echo "o fontconfig enxerga $n fontes"
    [ "$n" -gt 0 ] || {
      echo "ERRO: NENHUMA fonte visivel pelo FONTCONFIG_FILE do aninhado." >&2
      echo "      Todo texto de app estrangeiro — e as mensagens de erro do" >&2
      echo "      compositor — sairia em tofu." >&2
      exit 1
    }

    # AND THE THREE GENERICS FALL ON OUR FONT (section 6.4). Finding "some" font
    # is not enough: the nested session exists to show what the distro shows, and
    # with the host's config in the middle it showed Liberation Sans.
    for g in sans-serif serif monospace; do
      matched=$(fc-match "$g" 2>/dev/null)
      echo "  $g -> $matched"
      case "$matched" in
        *NoctuaIosevka*) ;;
        *) echo "ERRO: \`$g\` caiu em '$matched', e nao na nossa fonte." >&2
           exit 1 ;;
      esac
    done
    echo "OK: o fontconfig do aninhado acha fonte, e e a nossa"
  '';

  # NIRI'S CONFIG IS VALID — BOTH of them, the production one and the nested one.
  #
  # # What this catches, and what not having it cost
  #
  # A `duplicate keybind` — `Alt+Ctrl+L` from niri's defaults against the
  # emergency `Ctrl+Alt+L`, the same key written in another order — went through
  # the WHOLE `nix flake check`. The symptom was a red box on the screen and,
  # worse, niri falling back to its DEFAULT config: none of our shortcuts, and
  # the leader not working.
  #
  # Nothing validated the generated files. The `niri-binds.nix` grepped for one
  # word, the module grepped for another — and neither of them asked the
  # compositor whether the whole file closes.
  #
  # `niri validate` is the right question, asked of whoever has the answer.
  niri-config-valid = mkCheck "niri-config-valid" [ inputs.niri.packages.${system}.niri ] ''
    for cfg in "${self.nixosConfigurations.vm.config.environment.etc."niri/config.kdl".source}" \
               "${devApp.config}"; do
      echo "validando $cfg"
      if ! niri validate --config "$cfg" 2>erro.txt; then
        echo "ERRO: o niri RECUSOU o config gerado." >&2
        echo "      Um config recusado nao e um atalho quebrado: e o" >&2
        echo "      compositor caindo no padrao dele, sem nada nosso." >&2
        cat erro.txt >&2
        exit 1
      fi
    done
    echo "OK: os dois configs do niri sao validos"
  '';

  # THE KEYBOARD LAYOUT REALLY EXISTS.
  #
  # # Why this needs a check
  #
  # xkb does NOT refuse a layout it does not know: it falls back to `us` and
  # carries on. The symptom is the whole keyboard typing something else, with
  # not one line of error anywhere — the same class as the silent fallback that
  # has already cost dearly four times in this project, and the dearest of them,
  # because the way to investigate is by typing.
  #
  # The source of truth is the `xkeyboard-config` registry the session is going
  # to use, and not a list of ours: bumping nixpkgs may retire a name.
  # Chrome does not ask for a password, and cannot start asking again.
  #
  # Two halves, and the check has to see both. The flag alone is not enough: a
  # Secret Service that exists in the closure will be found by SOMETHING, and
  # the prompt comes back through a dependency nobody chose — which is exactly
  # the failure this is here to prevent, because the symptom (a password box on
  # startup) looks like a Chrome setting and is not.
  browser-has-no-keyring = mkCheck "browser-has-no-keyring" [ pkgs.jq ] ''
    sistema=${self.nixosConfigurations.vm.config.system.build.toplevel}

    # 1) The flag reached the wrapper the user actually runs.
    chrome=$(readlink -f "$sistema/sw/bin/google-chrome-stable" 2>/dev/null \
             || readlink -f "$sistema/sw/bin/google-chrome" 2>/dev/null || true)
    [ -n "$chrome" ] || { echo "ERRO: o Chrome nao esta no closure do sistema" >&2; exit 1; }
    grep -q -- "--password-store=basic" "$chrome" || {
      echo "ERRO: o wrapper do Chrome nao tem --password-store=basic." >&2
      echo "      Sem ele o Chrome procura um Secret Service e pede senha." >&2
      exit 1; }
    echo "OK: --password-store=basic no wrapper"

    # 2) NO Secret Service anywhere in the closure.
    #
    # `nix-store -qR` and not a grep over the config: the point is what ends up
    # ON THE MACHINE, and a dependency that pulls the keyring in does not appear
    # in any option.
    found=$(nix-store -qR "$sistema" 2>/dev/null | grep -E "gnome-keyring|kwallet" || true)
    if [ -n "$found" ]; then
      echo "ERRO: um Secret Service entrou no closure:" >&2
      echo "$found" | sed 's/^/       /' >&2
      echo "" >&2
      echo "      Com ele instalado o Chrome volta a pedir senha ao abrir," >&2
      echo "      e o prompt e do keyring, nao do Chrome: nenhuma opcao do" >&2
      echo "      navegador o desliga." >&2
      exit 1
    fi
    echo "OK: nenhum gnome-keyring nem kwallet no closure"
  '';

  # The nine directories of section 2 reach `user-dirs.dirs`.
  #
  # Setting the environment variable alone moves NOTHING: a file dialog, a
  # browser download and `xdg-open` all read `~/.config/user-dirs.dirs`. So the
  # check reads the generated file, not the option — the option agreeing with
  # itself proves nothing.
  #
  # `XDG_PROJECTS_DIR` is in the list on purpose even though the spec has no such
  # key: it is ours, and if it silently vanished from the generated file nobody
  # would notice until a script went looking for it.
  xdg-user-dirs = mkCheck "xdg-user-dirs" [ ] ''
    dirs=${
      self.nixosConfigurations.vm.config.home-manager.users.noctua.xdg.configFile."user-dirs.dirs".source
    }
    echo "arquivo gerado: $dirs"

    expected="XDG_DESKTOP_DIR=/home/noctua/main/temp
    XDG_DOCUMENTS_DIR=/home/noctua/main/archive
    XDG_DOWNLOAD_DIR=/home/noctua/main/temp
    XDG_MUSIC_DIR=/home/noctua/main/media/audio-music
    XDG_PICTURES_DIR=/home/noctua/main/media/image
    XDG_PROJECTS_DIR=/home/noctua/main/prj
    XDG_PUBLICSHARE_DIR=/home/noctua/main/share/public
    XDG_TEMPLATES_DIR=/home/noctua/templates
    XDG_VIDEOS_DIR=/home/noctua/main/media/video"

    missing=0
    echo "$expected" | while read -r pair; do
      [ -n "$pair" ] || continue
      key=''${pair%%=*}
      value=''${pair#*=}
      line=$(grep -E "^$key=" "$dirs" || true)
      if [ -z "$line" ]; then
        echo "ERRO: $key nao esta no user-dirs.dirs gerado" >&2; exit 1
      fi
      # The generated value is quoted and uses `$HOME`.
      got=$(echo "$line" | sed -e 's/^[^=]*=//' -e 's/^"//' -e 's/"$//' -e "s|\\\$HOME|/home/noctua|")
      if [ "$got" != "$value" ]; then
        echo "ERRO: $key = $got, esperado $value" >&2; exit 1
      fi
      echo "  OK  $key = $got"
    done || missing=1
    [ "$missing" = 0 ] || exit 1

    echo "OK: os nove diretorios da secao 2 chegam ao user-dirs.dirs"
  '';

  keyboard-layout-exists = mkCheck "keyboard-layout-exists" [ ] ''
    lst=${pkgs.xkeyboard_config}/share/X11/xkb/rules/base.lst
    layout=${keyboard.layout}
    optionName=${keyboard.options}
    variantName="${keyboard.variant}"

    grep -qE "^ +$layout +" "$lst" || {
      echo "ERRO: o layout \`$layout\` nao existe no xkeyboard-config." >&2
      echo "      O xkb NAO recusa: ele cai no \`us\` e o keyboard inteiro" >&2
      echo "      passa a digitar outra coisa, sem erro nenhum." >&2
      exit 1; }

    grep -qE "^ +$optionName +" "$lst" || {
      echo "ERRO: a opcao \`$optionName\` nao existe no xkeyboard-config." >&2
      echo "      Sem ela o AltGr volta a ser nivel 3 e some a acentuacao." >&2
      exit 1; }

    if [ -n "$variantName" ]; then
      grep -qE "^ +$variantName +$layout:" "$lst" || {
        echo "ERRO: a variante \`$variantName\` nao existe para \`$layout\`." >&2
        exit 1; }
    fi

    # And the CONSOLE's keymap, which is another database (kbd, not xkb). The
    # installer runs on a TTY: a wrong name here gives a US keyboard at the moment
    # of typing the disk's passphrase.
    test -e ${pkgs.kbd}/share/keymaps/i386/qwerty/${keyboard.console}.map.gz || {
      echo "ERRO: o keymap de console \`${keyboard.console}\` nao existe no kbd." >&2
      exit 1; }

    echo "OK: layout $layout, opcao $optionName, console ${keyboard.console}"
  '';

  # EVERY COMPOSITOR ACTION HAS A TRANSLATION IN BOTH LANGUAGES.
  #
  # The action list is GENERATED from the pinned niri, and the translations are
  # written by hand. The two things move separately: bumping niri's tag brings
  # new actions, and without this check they would show up in the finder as
  # `[cmd-niri-...]` — the missing-key marker, in the middle of a list of 170
  # items, where nobody looks.
  #
  # It fails LOUDLY on purpose: a niri bump stops the build until what showed up
  # is translated. It is the price of mirroring the whole compositor, and it is
  # paid once per release instead of every day by whoever reads the list.
  niri-actions-translated = mkCheck "niri-actions-translated" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import re, sys, pathlib

    table_src = pathlib.Path("${niriActions}/niri-actions.rs").read_text()
    key_names = re.findall(r'key: "(cmd-niri-[a-z0-9-]+)"', table_src)
    assert len(key_names) > 100, f"so {len(key_names)} acoes na tabela gerada"

    absent = []
    for language in ("pt-BR", "en-US"):
        ftl = pathlib.Path("${../assets/i18n}/" + language + ".ftl").read_text()
        present = set(re.findall(r"^([a-z0-9-]+) *=", ftl, re.M))
        absent += [f"{language}: {c}" for c in key_names if c not in present]

    if absent:
        print("ERRO: acao do niri sem traducao:", file=sys.stderr)
        for f in absent:
            print(f"  {f}", file=sys.stderr)
        print(
            "\nA lista de acoes vem do niri PINADO. Se ele subiu de versao,\n"
            "as acoes novas precisam entrar nos dois .ftl — senao elas\n"
            "aparecem como `[chave]` no localizador.",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"OK: as {len(key_names)} acoes do niri estao nos dois idiomas")
    PY
  '';

  # THE SUBMENU MARK DOES NOT LIVE IN THE TRANSLATION.
  #
  # It used to: `group-window = +janela`, and the `+` was part of the text. That
  # is why three of the six groups ended up without it — nothing forced it, and
  # the omission showed up nowhere. Now whoever puts the `+` on is the drawing,
  # out of the tree's `e_grupo`, and no group has any way of escaping.
  #
  # This check stops it from coming back: a `+` in the .ftl would give `++janela`
  # on screen, which is the loud symptom of a mark in two places.
  group-mark-outside-translation = mkCheck "group-mark-outside-translation" [ ] ''
    if grep -nE '^grupo-[a-z-]+ *= *\+' ${../assets/i18n}/*.ftl; then
      echo "ERRO: ha rotulo de grupo com \`+\` na traducao." >&2
      echo "      A marca e posta no DESENHO (surface::MARCA_DE_GRUPO);" >&2
      echo "      escrita aqui, ela sai duplicada — e so em alguns grupos." >&2
      exit 1
    fi
    echo "OK: a marca de submenu nao esta em nenhuma traducao"
  '';

  # And the other way round: a key in only one language BRINGS THE BUILD DOWN.
  # Without this fixture, the check above would pass with the generator broken.
  i18n-missing-key-fails = mkCheck "i18n-missing-key-fails" [ pkgs.python3 ] ''
    cp -r ${../assets/i18n} bad && chmod -R +w bad
    echo 'so-em-ingles = orphan' >> bad/en-US.ftl

    if python3 ${../nix/scripts/gen-i18n.py} bad $TMPDIR/keys.rs 2>erro.txt; then
      echo "ERRO: o gerador ACEITOU uma chave que existe em so um idioma" >&2
      exit 1
    fi
    grep -q 'so-em-ingles' erro.txt || {
      echo "ERRO: falhou, mas a mensagem nao diz qual chave" >&2
      cat erro.txt >&2
      exit 1
    }
    echo "OK: chave em so um idioma derruba o build, dizendo qual"
  '';

  # A TOKEN THAT DOES NOT EXIST DOES NOT COMPILE IN THE BUFFER — section 7.6.
  #
  # "The build generates (a) the shell's Rust table and (b) the WIT enum of valid
  # names. A token that does not exist becomes a **compile error in the guest**."
  #
  # Before, `cor` was `record cor { token: string }`: a buffer cited
  # `color.inexistente`, compiled, and the host fell into a silent fallback —
  # the same class as invariant 8's `[api_key]`.
  #
  # # Why this check is STRUCTURAL, and not "compile and watch it fail"
  #
  # The guarantee does not come from a test: it comes from `cor` being an ENUM.
  # Citing a case that does not exist in an enum is a compile error in Rust —
  # that is a property of the language, and failing it here would be testing rustc.
  #
  # What CAN break, and is therefore what is asserted: `cor` going back to being
  # a record with a string, or the two generated artifacts diverging. Those are
  # the only two ways for the guarantee to be lost, and both are covered.
  invalid-token-fails-build = mkCheck "invalid-token-fails-build" [ pkgs.python3 ] ''
    wit=${wit}/deps/tokens/tokens.wit
    rs=${tokensRs}/tokens.rs

    # 1. `cor` is an enum. If it goes back to being a record with a string, the guarantee is gone.
    grep -qE '^\s*enum color \{' "$wit" || {
      echo "ERRO: \`color\` nao e mais um enum no WIT." >&2
      echo "      Com string livre, token inexistente compila no guest e" >&2
      echo "      falha em silencio na tela. Ver a secao 7.6." >&2
      exit 1
    }
    if grep -qE 'record color|token:\s*string' "$wit"; then
      echo "ERRO: \`color\` voltou a carregar string." >&2
      exit 1
    fi

    # 2. The two generated artifacts have exactly the same set of names.
    #    A divergence here shows up as a wrong color, never as an error.
    python3 - "$wit" "$rs" <<'PY'
    import re, sys
    wit, rs = open(sys.argv[1]).read(), open(sys.argv[2]).read()

    block = re.search(r"enum color \{(.*?)\n    \}", wit, re.S).group(1)
    in_wit = {c.strip().lstrip("%").rstrip(",")
              for c in block.splitlines()
              if c.strip() and not c.strip().startswith("//")}

    in_rs = set(re.findall(r'"([a-z0-9-]+)" => color::', rs))

    only_wit, only_rs = in_wit - in_rs, in_rs - in_wit
    if only_wit or only_rs:
        for n in sorted(only_wit):
            print(f"ERRO: `{n}` esta no WIT e nao na ponte Rust", file=sys.stderr)
        for n in sorted(only_rs):
            print(f"ERRO: `{n}` esta na ponte Rust e nao no WIT", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(in_wit)} cores, mesmas nos dois artefatos gerados")

    # 2b. THE SAME, FOR `icon`.
    #
    # Colour has a hand-written bridge in the host, so a divergence there is a
    # human mistake. Icons do NOT: the WIT enum and the Rust constants are both
    # emitted by `gen-tokens.py` from one dictionary, which is exactly why this
    # went unchecked — "same source, cannot diverge".
    #
    # A generator can still drop one side. And the symptom would be the `?`
    # glyph the host draws for an unknown token: visible only on the screen that
    # happens to use it, and looking like a font problem.
    block = re.search(r"enum icon \{(.*?)\n    \}", wit, re.S)
    if block is None:
        print("ERRO: `icon` deixou de ser enum no WIT", file=sys.stderr)
        raise SystemExit(1)
    icon_wit = {c.strip().lstrip("%").rstrip(",")
                for c in block.group(1).splitlines()
                if c.strip() and not c.strip().startswith("//")}

    # `pub const SOME_NAME: &str` inside `pub mod icon`.
    mod_icon = re.search(r"pub mod icon \{(.*?)\n\}", rs, re.S)
    if mod_icon is None:
        print("ERRO: `pub mod icon` sumiu do Rust gerado", file=sys.stderr)
        raise SystemExit(1)
    icon_rs = {n.lower().replace("_", "-")
               for n in re.findall(r"pub const ([A-Z0-9_]+):", mod_icon.group(1))}

    only_wit, only_rs = icon_wit - icon_rs, icon_rs - icon_wit
    if only_wit or only_rs:
        for n in sorted(only_wit):
            print(f"ERRO: icone `{n}` esta no WIT e nao no Rust", file=sys.stderr)
        for n in sorted(only_rs):
            print(f"ERRO: icone `{n}` esta no Rust e nao no WIT", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(icon_wit)} icones, mesmos nos dois artefatos gerados")
    PY

    # 3. The host's bridge must have no default arm: `_ =>` has to give back
    #    `None`, and not a color. A `_ => TEXT_DIM` would bring the fallback back.
    if grep -qE '_ => color::' "$rs"; then
      echo "ERRO: a ponte do host tem fallback para uma cor de verdade." >&2
      echo "      Nome invalido tem de virar \`None\`, nao uma cor bonita." >&2
      exit 1
    fi
  '';

  # NO SECRET IN THE TREE — section 9 of the prompt, invariant 13.
  #
  # "Add a check to `nix flake check` that fails if it detects secret patterns in
  # the repository." It is what holds up "the repository has to be publishable as
  # it is".
  #
  # It measures `self`, which already IS the source's path in the store —
  # measuring makes no new copy (unlike `${../.}`; see the CLAUDE.md).
  secrets-out-of-repo = mkCheck "secrets-out-of-repo" [ pkgs.python3 ] ''
    python3 ${../nix/scripts/scan-secrets.py} ${self}
  '';

  # And the other way round: a planted secret BRINGS THE CHECK DOWN.
  #
  # Without this fixture, "the sweep works" would be an untested claim — and a
  # sweep that has never reported anything is indistinguishable from a broken one.
  # It covers the six forms, because one regex can die on its own.
  planted-secret-fails = mkCheck "planted-secret-fails" [ pkgs.python3 ] ''
    mkdir -p tree_out/sub

    # Assembled in pieces for the same reason as the script: written whole, this
    # file would report itself in the real sweep.
    printf -- '-----%s OPENSSH %s-----\n' BEGIN 'PRIVATE KEY' > tree_out/sub/id_ed25519
    printf 'u:%sy%sj9T%sFq3kL9mZx2pQvR7sT1uWn0%saBcDeFgHiJkLmNoPqRsTuVwXyZ01234\n' \
      "\$" "\$" "\$" "\$" > tree_out/sub/shadow
    printf 'AGE-SECRET-%s\n' 'KEY-1QQPQZRHW4YSHVMKS4YFPXP93KVMHJ2LPXPD9Y6MPQ7TCFVWXHXAS8RXNMV' \
      > tree_out/sub/api_key.txt
    printf 'token = "%sA1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8"\n' 'ghp_' > tree_out/sub/cfg.toml
    printf 'psk%s"SenhaDoWifi12345"\n' '=' > tree_out/sub/wifi.conf
    printf 'maintainer = "fulano.real%sprovedor.com"\n' '@' > tree_out/sub/meta.nix

    if python3 ${../nix/scripts/scan-secrets.py} tree_out 2>erro.txt; then
      echo "ERRO: a varredura NAO acusou os segredos plantados." >&2
      exit 1
    fi

    # Every form has to show up: one regex can break without the others.
    for forma in "chave privada" "hash de senha" "chave age" \
                 "token de servico" "PSK de wifi" "e-mail"; do
      grep -q "$forma" erro.txt || {
        echo "ERRO: a varredura nao acusou: $forma" >&2
        cat erro.txt >&2
        exit 1
      }
    done

    # And the secret must NOT show up in the output: the CI log would publish it.
    if grep -qE 'A1b2C3d4|QQPQZRHW4|SenhaDoWifi' erro.txt; then
      echo "ERRO: a varredura imprimiu o proprio segredo." >&2
      exit 1
    fi

    echo "OK: as seis formas derrubam o check, e nenhuma vaza na mensagem"
  '';

  # Notifications, history and the proof that there is no second surface.
  notifyVm = import ./notify-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # THE 5 MS BUDGET of section 5.2, measured on the heaviest component.
  #
  # The glyph browser embeds the whole database (2.8 MB). If any component were
  # going to blow the ceiling, it would be that one — and that is why it is the
  # one the gate measures, not the example buffer.
  #
  # Section 9.3 of the plan is explicit: instantiation is one of the metrics
  # INDEPENDENT of the GPU that the CI gates. It does not fall under the ban on
  # measuring performance under lavapipe, which is about rasterization.
  instance-fast = mkCheck "instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm 200
  '';

  # THE FIVE FILTERS OF SECTION 6.5, exercised on the real component.
  #
  # It drives the buffer the way the host drives it — `evento` and `render` on
  # the SAME instance — and checks the result, not only that the call returned.
  # It was that distinction that was missing: the search "existed" and split the
  # whole database into ONE record, because the only check that touched it
  # measured instantiation.
  browser-filters = mkCheck "browser-filters" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-browser \
      ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm \
      ${../assets/i18n}
  '';

  # THE SYMBOL DATABASE IS GENERATED FROM THE REAL SOURCE FILE, like the glyph one.
  #
  # What it asserts beyond "the file exists": the total (the intersection of the
  # UCD with the `cmap`), and that the TWO fields only this buffer has arrived —
  # the `mostra` of the combining marks and the `largura` of the double-advance
  # ones. Without them the database looks whole and the screen lies on 325
  # characters.
  unicode-table-generated = mkCheck "unicode-table-generated" [ pkgs.jq ] ''
    db=${unicodedb.db}
    for f in unicode.json coverage.md; do
      [ -s "$db/$f" ] || { echo "ERRO: $f ausente ou vazio" >&2; exit 1; }
    done

    n=$(jq 'length' "$db/unicode.json")
    echo "caracteres na base: $n"
    [ "$n" -gt 7000 ] || { echo "ERRO: base pequena demais ($n)" >&2; exit 1; }

    # THE MANDATORY FIELDS, in every record.
    jq -e 'all(.cp and .name and .shows and .family)' "$db/unicode.json" >/dev/null \
      || { echo "ERRO: registro sem cp, nome, mostra ou familia" >&2; exit 1; }

    # `mostra` IS A LIST OF NUMBERS, and not text.
    #
    # It is what stops the character from travelling as a string: for `U+0022` the
    # JSON would write `"\""` and the component's parser — which finds the end of a
    # value by looking for the next quote — would read `\`. Two wrong characters,
    # silently, and only on those two.
    jq -e 'all(.shows | type == "array" and all(type == "number"))' \
      "$db/unicode.json" >/dev/null \
      || { echo "ERRO: \`shows\` deixou de ser lista de codepoints" >&2; exit 1; }

    # NO string field carries punctuation that would break the `split`.
    #
    # The punctuation that actually breaks it: a quote (it cuts the value in half),
    # a brace and a comma (they split a record and a list) and a bracket (it
    # delimits a list). The `:` does NOT count — it separates nothing in the
    # component's parser, and `bandeira: Brasil` is a legitimate emoji name coming
    # from the CLDR.
    bads=$(jq '[.[] | to_entries[] | select(.value | type == "string")
                 | select(.value | test("[\"{},\\[\\]]"))] | length' "$db/unicode.json")
    [ "$bads" = 0 ] || {
      echo "ERRO: $bads campos de string com pontuacao de JSON." >&2
      echo "      O componente divide os registros por \`},\` e nao tem parser." >&2
      exit 1; }

    combining=$(jq '[.[] | select(.width == 0)] | length' "$db/unicode.json")
    wide=$(jq '[.[] | select(.width == 2)] | length' "$db/unicode.json")
    echo "combinantes: $combining   largura dupla: $wide"
    [ "$combining" -gt 100 ] || { echo "ERRO: as marcas combinantes sumiram" >&2; exit 1; }
    [ "$wide" -gt 0 ] || { echo "ERRO: os de largura dupla sumiram" >&2; exit 1; }

    echo "OK: base de simbolos gerada da fonte real, com mostra e largura"
  '';

  # THE UCD VERSION GUARD SPEAKS — the negative fixture.
  #
  # Without it the guard would be a promise, and the glyph one has already shown
  # that an unverified promise is the same thing as an absent guard.
  unicode-version-mismatch-fails = mkCheck "unicode-version-mismatch-fails" [ ] ''
    grep -q "a versao do Unicode Character Database divergiu" \
      ${unicodedb.versionMismatchFails}/out_text.txt \
      || { echo "ERRO: a mensagem nao diz que as versoes divergem" >&2; exit 1; }
    echo "OK: UCD de outra versao derruba o build, dizendo qual"
  '';

  # THE 5 MS BUDGET, on the new component too.
  #
  # The glyph one is still the heaviest, so it is still the main gate. This one
  # exists for the opposite reason: a field regression in the new generator
  # (taking the character back to a string, leaving the width always present)
  # pushes the database towards 3 MB with nobody measuring.
  unicode-instance-fast = mkCheck "unicode-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.unicode}/lib/ronoctua_buffer_unicode.wasm 200
  '';

  # THE LUCIDE DATABASE IS GENERATED FROM THE REAL SET, and the font along with it.
  #
  # What it asserts beyond "the file exists": the exact total, that BOTH
  # codepoints of each icon arrived (the drawing one and the origin one), and
  # that the two live in SEPARATE ranges. That separation is this buffer's whole
  # subject — see `nix/lucidegen.nix`.
  lucide-table-generated = mkCheck "lucide-table-generated" [ pkgs.jq ] ''
    db=${lucidedb.db}
    for f in lucide.json coverage.md NoctuaLucide.ttf; do
      [ -s "$db/$f" ] || { echo "ERRO: $f ausente ou vazio" >&2; exit 1; }
    done

    n=$(jq 'length' "$db/lucide.json")
    echo "icones na base: $n"
    [ "$n" = 1668 ] || { echo "ERRO: a base tem $n icones e devia ter 1668" >&2; exit 1; }

    jq -e 'all(.name and .cp and .lcp and .cats and .nodes)' "$db/lucide.json" >/dev/null \
      || { echo "ERRO: registro sem nome, cp, lcp, cats ou nos" >&2; exit 1; }

    # THE TWO CODEPOINTS ARE FROM DIFFERENT RANGES.
    #
    # `cp` draws and `lcp` is copied. If they are ever equal, the remapping is gone
    # — and with it the reason for the buffer not drawing 634 wrong icons.
    not_in=$(jq '[.[] | select(.cp < 991232 or .cp > 993667)] | length' "$db/lucide.json")
    [ "$not_in" = 0 ] || {
      echo "ERRO: $not_in icones desenham fora de U+F2000..U+F2683." >&2
      echo "      A faixa de origem colide com 634 codepoints do Nerd Fonts." >&2
      exit 1; }
    merged=$(jq '[.[] | select(.cp == .lcp)] | length' "$db/lucide.json")
    [ "$merged" = 0 ] || { echo "ERRO: $merged icones com cp == lcp" >&2; exit 1; }

    # NO string field carries punctuation that would break the component's
    # `split`, which has no JSON parser (invariant 11). The `d` of the SVGs carries
    # a comma at the origin; the generator swaps it for a space, which the SVG
    # grammar treats the same.
    bads=$(jq '[.[] | to_entries[] | select(.value | type == "string")
                 | select(.value | test("[\"{},\\[\\]]"))] | length' "$db/lucide.json")
    [ "$bads" = 0 ] || {
      echo "ERRO: $bads campos de string com pontuacao de JSON." >&2
      exit 1; }

    cats=$(jq '[.[].cats[]] | unique | length' "$db/lucide.json")
    [ "$cats" = 43 ] || { echo "ERRO: $cats categorias, e a 0.563.0 tem 43" >&2; exit 1; }

    echo "OK: 1668 icones, 43 categorias, desenho e origem em faixas separadas"
  '';

  # THE COLLISION GUARD SPEAKS — the negative fixture.
  #
  # It is not hypothetical: it is the measured defect. Without the guard, 634 of
  # the 1,668 icons would draw the Nerd Fonts glyph that occupies the same codepoint.
  lucide-range-taken-fails = mkCheck "lucide-range-taken-fails" [ ] ''
    grep -q "colide com 634 codepoints" ${lucidedb.rangeTakenFails}/out_text.txt \
      || { echo "ERRO: a mensagem nao diz quantos codepoints colidem" >&2; exit 1; }
    echo "OK: a faixa de origem derruba o build, dizendo quantos colidem"
  '';

  # THE 5 MS BUDGET, on the third component too.
  lucide-instance-fast = mkCheck "lucide-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.lucide}/lib/ronoctua_buffer_lucide.wasm 200
  '';

  # THE LUCIDE BROWSER, driven the way the host drives it.
  #
  # The name ended up narrow: besides the search and the filters, it covers the
  # whole sheet — the 19 forms of copy, the scrolling of the 24 lines, and the
  # two that were born with the vector preview:
  #
  #   - the affordance MARK matches the table the key reads, line by line. A mark
  #     that promises a key that does nothing is worse than no mark at all, and
  #     that was exactly the case of the color line;
  #   - the PREVIEW is byte for byte the SVG the copy delivers, before and after
  #     touching the stroke. A second constructor for the screen would drift
  #     silently, and the preview would stop being the artifact with nothing
  #     giving it away.
  lucide-filters = mkCheck "lucide-filters" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-lucide \
      ${buffers.lucide}/lib/ronoctua_buffer_lucide.wasm \
      ${../assets/i18n}
  '';

  # WHICH FACE DRAWS WHAT — the promise the comment in `text.rs` had been making
  # since the second face and that had never been written down.
  #
  # It also witnesses the COLLISION that forced the remapping: as long as Iosevka
  # answers for `U+E0A0`, publishing Lucide in the origin range means drawing the
  # wrong symbol. The day that changes, the check fails and someone re-reads the
  # decision instead of rediscovering it through the screen.
  renderer-loads-three-faces = mkCheck "renderer-loads-three-faces" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-faces
  '';

  # THE UNICODE SYMBOL BROWSER, by the same path: the real component, driven
  # the way the host drives it.
  #
  # It asserts three things the glyph one does not have to assert, and each one
  # corresponds to a real difference of the Unicode database:
  #
  #   - the EXACT total (7,529). The component's parser splits the records on
  #     `"},"`, and `}` is INSIDE this database (`U+007D`) — what saves it is
  #     the generator not emitting a string field with an arbitrary character;
  #   - the combining mark drawn over the dotted circle, and COPIED without it:
  #     pasting `◌́` into the user's document is the worst possible defect, and
  #     the only one that survives any visual inspection;
  #   - the doubled budget of the 23 double-width ones, otherwise the host
  #     elides a single character into `…`.
  symbols-filters = mkCheck "symbols-filters" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-symbols \
      ${buffers.unicode}/lib/ronoctua_buffer_unicode.wasm \
      ${../assets/i18n}
  '';

  # The OTHER half of the GPU-independent metrics of section 9.3: the CPU work
  # that happens before any pixel.
  #
  # The gate is loose on purpose — today there is 5000x of headroom. It does not
  # exist to tighten the layout, it exists so that a regression of an ORDER OF
  # MAGNITUDE (measuring text instead of counting columns, allocating per
  # segment) shows up here and not in a bisection of stutter six months later.
  layout-fast = mkCheck "layout-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-layout 5000
  '';

  # Text shaping must not go back to the regime of SECONDS.
  #
  # It cost 4,447 ms on the first frame because cosmic-text's `Family::Monospace`
  # points at a font that is not in our database. The check compares the four
  # arrangements and fails if the production one regresses — the ceiling is loose
  # (300 ms against a floor of 0.1 ms) because what it catches is the return of
  # the regime, not variation between machines.
  shaping-fast = mkCheck "shaping-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-shaping
  '';

  # A SURFACE THE SIZE OF A MONITOR HAS TO FIT.
  #
  # `Limits::downlevel_defaults()` fixes `max_texture_dimension_2d` at 2048, and
  # the presentation surface IS a texture. The symptom had two faces, and neither
  # of them pointed at the limits:
  #
  #   - `Alt+=` on a 1920 monitor takes the column to 2112 px on the FIRST step,
  #     and the window CLOSES BY ITSELF. `Surface::configure` gives back no error:
  #     it kills the process, and niri sends the output of what it spawns to
  #     /dev/null;
  #   - any monitor wider than 2048 px never had a footer, because it takes the
  #     screen's width and died before the first pixel.
  #
  # The check asks for a device with the PRODUCTION limits and tries the texture.
  # Comparing numbers from `Limits` would be checking our arithmetic against
  # itself; here whoever answers is the driver.
  large-monitor-surface = mkCheck "large-monitor-surface" [ ] ''
    export VK_DRIVER_FILES=${xvfb.icd}
    export VK_ICD_FILENAMES=${xvfb.icd}
    export LIBGL_ALWAYS_SOFTWARE=1
    ${rust.workspace}/bin/ronoctua-test-large-surface
  '';

  # ENDING THE SESSION MUST NOT PRINT AN ERROR.
  #
  # # What this catches
  #
  # Two defects that lived together for months with nothing reporting them, and
  # that the owner saw every time they closed with Ctrl+C:
  #
  #   rm: cannot remove '/nix/store/…-ronoctua-niri-aninhado.kdl': Permission denied
  #   rm: cannot remove '/nix/store/…-ronoctua-niri-aninhado.kdl': Permission denied
  #
  # The first: `encerrar` tried to delete the CONFIG, which became a store path
  # when it started being generated by Nix — immutable by construction. The
  # `rm -f` does not save it, because it silences "does not exist" and not "is
  # not yours".
  #
  # The second, the duplicate: `trap … EXIT INT TERM` runs the cleanup on INT and
  # again on EXIT.
  #
  # # Why it runs the REAL app
  #
  # Because the criterion is "the output is clean", and a replica of `encerrar`
  # inside the check would prove nothing about the `encerrar` that runs on the
  # owner's machine — and the lesson at the top of this file is exactly that.
  # With no display niri does not come up, the script falls into the error path,
  # and it is the real `encerrar` that runs.
  dev-exits-clean = mkCheck "dev-exits-clean" [ devApp ] ''
    export HOME=$TMPDIR
    export XDG_RUNTIME_DIR=$TMPDIR
    # A DISPLAY THAT DOES NOT EXIST. With none at all, the script stops at the
    # graphical-session guard BEFORE installing the trap, and the path this check
    # wants to exercise never runs. With a fake name it carries on, niri dies at
    # startup, the waiting loop gives up — and `encerrar` really runs.
    export WAYLAND_DISPLAY=nao-existe-1
    # It exits non-zero on purpose (niri does not come up here); what is measured
    # is what it PRINTS on ending.
    timeout 300 ronoctua-dev > out_text.txt 2> erro.txt || true

    if grep -qE "cannot remove|Permission denied" erro.txt; then
      echo "ERRO: encerrar a sessao imprimiu erro de remocao:" >&2
      grep -E "cannot remove|Permission denied" erro.txt >&2
      echo "      Caminho do store nao se apaga, e o '-f' nao cala isso." >&2
      exit 1
    fi

    # AND ONLY ONCE. The log line comes out of `encerrar`; two means the cleanup
    # ran twice.
    #
    # HOW FAR THIS PROVES IT: here the script exits by the error path, so only the
    # EXIT trap fires. The duplicate the owner saw comes from Ctrl+C — INT cleans
    # up and EXIT cleans up again —, and reproducing it would require a compositor
    # standing to interrupt. This check catches `encerrar` running twice on a
    # normal exit; the other half is seen on screen, and its fix is the
    # `trap 'exit 130' INT` of dev.nix.
    #
    # The `|| true` is around the `grep -c`: with no match at all it exits 1, and
    # under `pipefail` that would bring the check down before it counted — which is
    # the same trap `pgrep` has already set twice in this file.
    n=$( { cat out_text.txt erro.txt | grep -c "logs desta sessao:"; } || true )
    [ "$n" = 1 ] || {
      echo "ERRO: o encerrar rodou $n vezes; esperava 1." >&2
      echo "      'trap … EXIT INT TERM' limpa pelo INT e de novo pelo EXIT." >&2
      echo "--- saida:" >&2; tail -20 out_text.txt >&2
      echo "--- erro:" >&2; tail -20 erro.txt >&2
      exit 1
    }

    echo "OK: encerrar a sessao sai limpo, e uma vez so"
  '';

  # HOT RELOAD RELOADS — behavior, not existence.
  #
  # The criterion of milestone 3 says "brings up niri + shell WITH HOT RELOAD".
  # The old check listed the `devApp` with the comment "it has to exist and be
  # executable", and an app that never reloads anything passes that easily. That
  # is how the milestone closed without the feature.
  #
  # This check exercises the real mechanism: it brings the watcher up over a
  # temporary directory, touches a `.rs`, and asserts that the command fired.
  # With no compositor and no GPU — that is why it fits in `nix flake check`,
  # which is where the milestone is proved.
  reload-reloads =
    mkCheck "reload-reloads"
      [
        reloadApp
        pkgs.procps
      ]
      ''
        export HOME=$TMPDIR
        mkdir -p obs
        : > obs/inicial.rs

        RONOCTUA_RELOAD_CMD="echo disparou >> $PWD/marcador" \
          ronoctua-reload "$PWD/obs" &
        watcher=$!
        trap 'kill $watcher 2>/dev/null || true' EXIT

        # The `inotifywait` has to be ARMED before the write: touching the file too
        # early and the event is lost and the check fails with no defect at all.
        for _ in $(seq 50); do
          [ -e /proc/$watcher ] || break
          sleep 0.2
          grep -q . /dev/null 2>/dev/null && break
        done
        sleep 3

        echo 'fn main() {}' > obs/mudou.rs

        ok=nao
        for _ in $(seq 50); do
          [ -s marcador ] && { ok=sim; break; }
          sleep 0.2
        done
        [ "$ok" = sim ] || {
          echo "ERRO: mudar um .rs NAO disparou o comando." >&2
          echo "O hot reload do marco 3 nao esta funcionando." >&2
          exit 1
        }

        # And the other way round: a change that is NOT code must not fire. Without
        # this the watcher would recompile on every `.swp` from the editor.
        : > marcador
        echo 'nao sou codigo' > obs/README.txt
        sleep 2
        [ ! -s marcador ] || {
          echo "ERRO: um arquivo que nao e .rs/.wit disparou a recompilacao." >&2
          exit 1
        }

        # IT DIES WHEN IT IS TOLD TO DIE.
        #
        # # The defect this catches, and that only shows up in use
        #
        # The watcher's last command was a pipeline in the FOREGROUND, and bash defers
        # a signal until the running command finishes — an `inotifywait -m` never
        # finishes. The `kill` from `nix/dev.nix` hung forever: the watcher survived
        # the Ctrl+C, was adopted by `systemd --user`, and every `nix run .#dev` left
        # one more of them recompiling the tree in the background.
        #
        # Nothing reported it. The two tests above passed with the defect present,
        # because firing and dying are different things — and the second is only found
        # out hours later, when the machine is full of them.
        kill "$watcher" 2>/dev/null || true
        died=nao
        for _ in $(seq 50); do
          [ -e /proc/$watcher ] || { died=sim; break; }
          sleep 0.2
        done
        [ "$died" = sim ] || {
          echo "ERRO: o watcher IGNOROU o SIGTERM e continua vivo." >&2
          echo "      Ele sera adotado pelo init e vai recompilar sozinho para" >&2
          echo "      sempre. Ver o \`wait\` no fim de nix/reload.nix." >&2
          exit 1
        }

        # AND IT TAKES THE CHILDREN WITH IT. Killing only the script would leave the
        # `inotifywait` alive, waiting for an event nobody reads any more — invisible,
        # because its name does not bring this project to mind.
        leftover=$(pgrep -c inotifywait 2>/dev/null || true)
        [ "''${leftover:-0}" = 0 ] || {
          echo "ERRO: sobraram $leftover inotifywait depois de matar o watcher." >&2
          exit 1
        }

        echo "OK: .rs dispara, o resto nao, e ele morre quando mandam"
      '';

  # ONE FOOTER PER SESSION — the second replaces the first, it does not add to it.
  #
  # # The defect this catches
  #
  # Restarting the footer was a FUNCTION inside `nix/dev.nix`, and the hot
  # reload's `reconstruir` runs in the WATCHER's process. The function got there
  # through `export -f`, but the `rodapepid` it read is a shell variable, and a
  # shell variable does not cross a process. Under the watcher's `set -u` that
  # does not become a false `if`: it becomes `unbound variable`, which brings the
  # whole shell down — and the loop's `|| true` does not save it.
  #
  # In order: the old footer did not die, the new one did not come up, and the
  # watcher died. On screen, "compiling…" and nothing else, forever.
  #
  # The `reload-reloads` above does not catch this, and that is not its fault: it
  # proves that the command FIRES. Firing and restarting are different things,
  # and the second is only found out when two footers fight over the keyboard —
  # the shortcut list opens and no key works.
  footer-is-single =
    mkCheck "footer-is-single"
      [
        singleFooter
        pkgs.procps
      ]
      ''
        export HOME=$TMPDIR
        pidfile="$PWD/rodape.pid"

        # A "footer" that only stands there. What is measured is the supervision, and a
        # real binary would bring a compositor, a GPU and fonts along with it.
        #
        # `printf` and not a heredoc: inside an indented Nix string the heredoc would
        # carry the indentation along, and a shebang with a space in front is not a
        # shebang — the error would show up as "file not executable", which does not
        # point at the cause.
        printf '#!/bin/sh\nexec sleep 300\n' > falso
        chmod +x falso

        ronoctua-footer-single "$pidfile" "$PWD/falso"
        first=$(cat "$pidfile")
        kill -0 "$first" || {
          echo "ERRO: o primeiro rodape nao subiu." >&2
          exit 1
        }

        # THE SECOND. This is where the defect was.
        ronoctua-footer-single "$pidfile" "$PWD/falso"
        second=$(cat "$pidfile")

        [ "$first" != "$second" ] || {
          echo "ERRO: o pidfile nao mudou; o segundo rodape nao subiu." >&2
          exit 1
        }

        if kill -0 "$first" 2>/dev/null; then
          echo "ERRO: o rodape anterior ($first) continua alive_flag." >&2
          echo "      Dois rodapes no mesmo display disputam o keyboard: a" >&2
          echo "      lista de atalhos abre num e as teclas vao para o outro." >&2
          exit 1
        fi

        kill -0 "$second" || {
          echo "ERRO: o rodape novo ($second) nao esta alive_flag." >&2
          exit 1
        }

        # AND THERE IS ONLY ONE. Counting is what separates "it replaced" from "it
        # killed the old one and brought up two".
        #
        # The `|| true` is INSIDE the braces: `pgrep` with no match at all exits 1,
        # and with `pipefail` that would bring the check down before counting.
        alive=$( { pgrep -f 'sleep 300' || true; } | wc -l)
        [ "$alive" = 1 ] || {
          echo "ERRO: ha $alive rodapes vivos; deveria haver exatamente 1." >&2
          exit 1
        }

        kill "$second" 2>/dev/null || true
        echo "OK: o segundo rodape substitui o primeiro, e sobra exatamente um"
      '';

  # THE FOOTER'S LOG OUTLIVES THE TERMINAL.
  #
  # # Why this is a check
  #
  # Because the shortcuts hung FOUR times and on all four there was no proof at
  # all: the log went to the terminal and nowhere else. When the owner writes "it
  # happened again", what exists is the description of the symptom — which does
  # not tell "the key did not arrive" from "it arrived with a stuck modifier",
  # and the two are fixed in opposite places.
  #
  # The check also asserts what has already broken once in this same file: the
  # pidfile keeps the FOOTER's PID. Resolving the log with a `tee` in the middle
  # would make `$!` be the tee's, and the pidfile would keep the wrong process —
  # which is exactly the defect `footer-is-single` exists in order not to have.
  footer-logs-to-file =
    mkCheck "footer-logs-to-file"
      [
        singleFooter
        pkgs.procps
      ]
      ''
        export HOME=$TMPDIR
        pidfile="$PWD/rodape.pid"
        export RONOCTUA_LOG="$PWD/rodape.log"

        # THE FAKE ONE SAYS ITS OWN PID in the log. It is what makes it possible to
        # check that the pidfile keeps IT, and not a wrapper — after the `exec` the
        # command line becomes `sleep 300` and is no good for identifying anyone.
        printf '#!/bin/sh\necho "footer-mark $$"\nexec sleep 300\n' > falso
        chmod +x falso

        ronoctua-footer-single "$pidfile" "$PWD/falso"
        pid=$(cat "$pidfile")

        for _ in $(seq 50); do
          [ -s "$RONOCTUA_LOG" ] && break
          sleep 0.1
        done

        grep -q "footer-mark" "$RONOCTUA_LOG" || {
          echo "ERRO: o log do rodape nao foi para \$RONOCTUA_LOG." >&2
          echo "      Sem arquivo, a proxima vez que os atalhos travarem vai" >&2
          echo "      ser diagnosticada do mesmo jeito que as quatro anteriores:" >&2
          echo "      sem nada." >&2
          exit 1
        }

        # THE PIDFILE KEEPS THE FOOTER, and not a wrapper.
        reported=$(awk '/footer-mark/ {print $2; exit}' "$RONOCTUA_LOG")
        [ "$reported" = "$pid" ] || {
          echo "ERRO: o rodape diz ser o pid $reported e o pidfile guarda $pid." >&2
          echo "      Um 'tee' no meio faria o \$! ser o dele — e o encerrar" >&2
          echo "      do dev.nix mataria o processo errado." >&2
          exit 1
        }

        # AND THE SECOND ONE APPENDS, it does not wipe: the hot reload restarts the
        # footer on every edit, and a log truncated per restart loses precisely the
        # piece before the defect.
        ronoctua-footer-single "$pidfile" "$PWD/falso"
        second=$(cat "$pidfile")
        for _ in $(seq 50); do
          [ "$(grep -c "footer-mark" "$RONOCTUA_LOG")" = 2 ] && break
          sleep 0.1
        done
        n=$(grep -c "footer-mark" "$RONOCTUA_LOG")
        [ "$n" = 2 ] || {
          echo "ERRO: o log tem $n marcas depois de dois rodapes; esperava 2." >&2
          echo "      O reinicio esta TRUNCANDO o log." >&2
          exit 1
        }

        kill "$second" 2>/dev/null || true
        echo "OK: o log do rodape vai para arquivo, acumula entre reinicios,"
        echo "    e o pidfile continua apontando para o rodape"
      '';

  # ZERO grid diff on the VT corpus (milestone 7, section 7.10).
  #
  # With a single engine, this is a golden file — it catches a regression, not a
  # divergence of interpretation. It is the degradation risk R1 prescribes, and
  # the binary itself says so in its output so nobody confuses the two.
  term-differential = mkCheck "term-differential" [ ] ''
    ${rust.workspace}/bin/ronoctua-term-diff \
      ${../tests/corpus-vt} ${../tests/golden-vt}
  '';

  installer-dryrun = mkCheck "installer-dryrun" [ ] ''
    export RONOCTUA_INST_DISK=/dev/nvme0n1
    export RONOCTUA_INST_HOSTNAME=noctua-teste
    export RONOCTUA_INST_PASSWORD=senha-de-teste
    export RONOCTUA_INST_PASSPHRASE=frase-secret_one-de-teste
    export RONOCTUA_INST_LANGUAGE=pt-BR
    # The sixth question. `br` and not the default: the dry run has to prove the
    # ANSWER reaches `host.nix`, and a value equal to the default would pass
    # even if the answer were being dropped.
    export RONOCTUA_INST_KEYBOARD=br

    out_text=$(${rust.workspace}/bin/ronoctua-install)
    echo "$out_text"

    # The `host.nix` must NOT carry a secret (section 9): it goes to backup.
    if echo "$out_text" | grep -q 'senha-de-teste\|frase-secreta-de-teste'; then
      echo "ERRO: o host.nix gerado contem segredo" >&2
      exit 1
    fi

    # NO label may come out as a raw key.
    #
    # `Textos::t` gives back `[api_key]` for a key that does not exist — on purpose,
    # so as not to bring the shell down over a missing text. The side effect is
    # that an unregistered key gets past ALL the other guards: `gen-i18n.py` only
    # compares `pt-BR` with `en-US`, and the compiler only protects whoever uses
    # `api_key::*`. The installer used a literal, and printed
    # `[inst-step-partition]` with nothing complaining.
    #
    # This line is what closes the hole at the flake's level.
    if echo "$out_text" | grep -q '\[inst-'; then
      echo "ERRO: rotulo saiu como chave crua — invariante 8" >&2
      echo "$out_text" | grep '\[inst-' >&2
      exit 1
    fi

    # The LUKS key is written BEFORE the first destructive step. `disko` reads it
    # in the first second; the file being missing after the disk has been wiped is
    # the worst possible moment to find out.
    order=$(echo "$out_text" | grep -n 'disko-luks.key\|disko --mode' | cut -d: -f1)
    first_key=$(echo "$order" | head -1)
    disko_at=$(echo "$order" | tail -1)
    [ "$first_key" -lt "$disko_at" ] || {
      echo "ERRO: a chave do LUKS e escrita depois de o disko rodar" >&2; exit 1; }

    # The password hash is written. Without it `base.nix` points
    # `hashedPasswordFile` at a file nobody creates, and the installed system comes
    # up with `mutableUsers = false` and no valid password — a machine nobody gets
    # into.
    echo "$out_text" | grep -q '/mnt/persist/etc/shadow/noctua' || {
      echo "ERRO: o plano nao grava o hash da senha" >&2; exit 1; }

    # The four PCRs, including 15. Without it the sealing does not protect against
    # a disk swap — see nix/modules/boot.nix.
    echo "$out_text" | grep -q 'tpm2-pcrs=7+11+12+15' || {
      echo "ERRO: o plano nao sela nos quatro PCRs" >&2; exit 1; }

    # And without `--executar` nothing runs. An installer that wipes a disk by
    # default is a dangerous command.
    #
    # The sentence comes from the `.ftl`, so matching the TEXT would tie the check
    # to one language — and the sandbox does not define `LANG`, so it would be
    # `en-US`. What does not change with the language is the mention of
    # `--executar`, which is what the line actually promises.
    echo "$out_text" | grep -q -- '--executar' || {
      echo "ERRO: o ensaio nao diz como executar de verdade" >&2; exit 1; }

    # And it proves it was a dry run: no step may have run. `nixos-facter` does not
    # even exist in the sandbox — if the installer tried, it would exit non-zero
    # and the check would already have died above at the `saida=$(...)`.
    [ ! -e /tmp/disko-luks.key ] || {
      echo "ERRO: o ensaio escreveu a chave do LUKS" >&2; exit 1; }

    # The SAME plan in both languages: same number of steps, same commands. A
    # `.ftl` that lost a key would come out with `[inst-` and would already have
    # died above; what this line catches is the subtler case of the language
    # changing what the installer DOES, and not only what it says.
    pt=$(LANG=pt_BR.UTF-8 ${rust.workspace}/bin/ronoctua-install | grep -c '^[! ] [0-9]')
    en=$(LANG=en_US.UTF-8 ${rust.workspace}/bin/ronoctua-install | grep -c '^[! ] [0-9]')
    [ "$pt" = "$en" ] || {
      echo "ERRO: o plano tem $pt passos em pt-BR e $en em en-US" >&2; exit 1; }

    echo "OK: $pt passos nos dois idiomas, quatro PCRs, ensaio por padrao"
  '';

  # The whole chain: signed UKI, Secure Boot and the proof of PCR 15 (milestone 8).
  secureBootVm = import ./secureboot-vm.nix {
    inherit pkgs lib sbKeys;
    lanzabooteModule = inputs.lanzaboote.nixosModules.lanzaboote;
  };

  # TPM2 sealing: it seals, unlocks, and refuses a volume with no token (milestone 8).
  tpmVm = import ./tpm-vm.nix { inherit pkgs lib; };

  # A native buffer in a window of its own, really drawing (milestone 7).
  bufferVm = import ./buffer-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
    bufferExample = buffers.example;
    bufferGlyphs = buffers.glyphs;
  };

  # The clipboard history: the native capture, the view and the re-copy (4.2).
  clipboardVm = import ./clipboard-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The terminal wired to the shell, with what is typed coming back to the screen (7.10).
  terminalVm = import ./terminal-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The leader as the Super key on its own: keyd standing and niri listening (7.4).
  #
  # The niri module goes along because the test checks BOTH ends of the contract
  # — the key keyd emits and the one niri listens for.
  keyboardVm = import ./keyboard-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.workspace;
    };
  };

  # One satellite per buffer scope, proved with TWO X11 apps (9.1).
  x11Vm = import ./x11-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The root's wipe, proved between two boots (milestone 7).
  impermanenceVm = import ./impermanence-vm.nix { inherit pkgs lib; };

  # Leader, which-key and the return to rest (milestone 5).
  leaderVm = import ./leader-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The app finder, and the footer that must not freeze when opening one.
  appsVm = import ./apps-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # The IPC contract against the pinned tag AND against `niri-next` (milestone 4).
  niriContract = import ./niri-contract.nix {
    inherit pkgs lib;
    niri = inputs.niri.packages.${system}.niri;
    niriNext = inputs.niri-next.packages.${system}.niri;
    ronoctua-shell = rust.workspace;
  };

  # -------------------------------------------------------------------
  # Granular checks of milestone 1
  # -------------------------------------------------------------------

  # THE FLAKE'S TREE MUST NOT GET FAT. It cost 793 GB to find out.
  #
  # This project is not a git repository (invariant 2), and it is git's index
  # that Nix uses to filter a flake's source. With no git, `nix flake check`
  # copies the WHOLE directory into the store — and since the path is
  # content-addressed, EVERY EDIT mints a new copy.
  #
  # With an 18 GB `target/` inside, that gave 44 copies and 793 GB: 94% of the
  # store and 42% of the disk, with nothing warning. The `nix/rust.nix` already
  # filtered the source `crane` compiles, but that filter runs LATER — the 19 GB
  # copy has already happened at evaluation time.
  #
  # The fix is keeping the build outside the tree (`CARGO_TARGET_DIR`, see
  # `nix/devshell.nix`). This check is what stops the fix from being lost: the
  # cost is invisible day to day and only shows up when the disk fills.
  #
  lean-tree = mkCheck "lean-tree" [ ] ''
    cap=20480  # KiB — the real tree is ~1.3 MB
    tree_size=$(du -s --exclude=.git ${self} | cut -f1)
    echo "fonte do flake: $tree_size KiB (teto $cap KiB)"
    if [ "$tree_size" -gt "$cap" ]; then
      echo >&2
      echo "ERRO: a arvore copiada para o store tem $tree_size KiB." >&2
      echo >&2
      echo "Cada edicao em QUALQUER arquivo cunha uma copia deste tamanho." >&2
      echo "Procure diretorio de build dentro da arvore:" >&2
      du -sh ${self}/* 2>/dev/null | sort -rh | head -5 >&2
      echo >&2
      echo "Build vai para \$CARGO_TARGET_DIR, fora da arvore." >&2
      exit 1
    fi
  '';

  # The mistake section 1 of the document explicitly says to avoid: the
  # .gitignore swallowing the font files that come to live in the repo.
  #
  # We cannot run `git check-ignore` against the real repository (there is no
  # git, by the decision of section 1), so we assemble a throwaway repo with
  # OUR .gitignore and the real tree of paths.
  gitignore-keeps-fonts = mkCheck "gitignore-keeps-fonts" [ pkgs.git ] ''
    export HOME=$TMPDIR
    mkdir -p repo && cd repo
    git init -q .
    cp ${../.gitignore} .gitignore

    # Recreates the real paths as empty files. The fonts are NOT here any more
    # (they live in ronoctua-fonts), so what has to survive the .gitignore is the
    # hand-kept content and the code.
    while IFS= read -r f; do
      mkdir -p "$(dirname "$f")"
      : > "$f"
    done < ${
      pkgs.writeText "tracked-paths.txt" (
        lib.concatStringsSep "\n" [
          "assets/lexicon/tags.en-pt.toml"
          "assets/lexicon/overlay.toml"
          "assets/icons/tray-glyphs.nix"
          "assets/tokens/.keep"
          "nix/fonts.nix"
          "nix/scripts/fix-font-names.py"
          "nix/scripts/make-console-psf.py"
          "docs/ARCHITECTURE.md"
          "tests/checks.nix"
          "flake.nix"
          "CLAUDE.md"
          "README.md"
        ]
      )
    }

    failed=0
    while IFS= read -r f; do
      if git check-ignore -q "$f"; then
        echo "ERRO: o .gitignore esta engolindo $f" >&2
        failed=1
      fi
    done < <(find assets nix docs tests -type f 2>/dev/null; ls flake.nix CLAUDE.md README.md)
    [ $failed -eq 0 ] || exit 1
    echo "OK: nenhuma fonte versionada e ignorada"

    # And the other way round: what MUST be ignored.
    for f in .TEMP/x.ttf result nix/hosts/host.nix foo.qcow2 a.key some.age; do
      mkdir -p "$(dirname "$f")"; : > "$f"
      git check-ignore -q "$f" || { echo "ERRO: $f nao esta sendo ignorado" >&2; exit 1; }
    done
    echo "OK: segredos, artefatos e host.nix ignorados"
  '';

  # Proof that the naming fix caught: ONE typographic family and 135 distinct
  # (family, subfamily) pairs, with no collision between SemiExtended
  # (usWidthClass 6) and Extended (7).
  #
  # The TTFs are fixed and MUST have a unique family. The WOFF2s pass through
  # untouched on purpose (see the note in nix/fonts.nix), so on them we only
  # take inventory — demanding a unique family there would assert something false.
  fonts-single-family = mkCheck "fonts-single-family" [ py ] ''
    echo "### corrigidos: familia unica e obrigatoria ###"
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/TTF-nerdfonts \
      --family "NoctuaIosevka Nerd Font Mono" --expect-count 135 --dry-run
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/TTF \
      --family "Noctua Iosevka" --expect-count 135 --dry-run

    echo
    echo "### intactos: so inventario ###"
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/WOFF2-nerdfonts \
      --family "NoctuaIosevka Nerd Font Mono" --expect-count 135 --inventory
    python3 ${../nix/scripts/fix-font-names.py} ${fonts.corrected}/WOFF2 \
      --family "Noctua Iosevka" --expect-count 135 --inventory
  '';

  # Invariants of the inventory, measured and not assumed. If the font is
  # swapped for another build, this fails and forces a review of the plan —
  # instead of the footer silently going out of alignment.
  fonts-metrics = mkCheck "fonts-metrics" [ py ] ''
    python3 - <<'PY'
    from fontTools.ttLib import TTFont
    import glob, sys

    failed = False
    expected_cp = 18053
    advances = {3: 456, 4: 548, 5: 600, 6: 658, 7: 720}

    for p in sorted(glob.glob("${fonts.corrected}/TTF-nerdfonts/*.ttf")):
        f = TTFont(p, lazy=True)
        cmap = f.getBestCmap()
        os2 = f["OS/2"]
        head = f["head"]

        if head.unitsPerEm != 1000:
            print(f"ERRO {p}: unitsPerEm {head.unitsPerEm} != 1000"); failed = True
        if len(cmap) != expected_cp:
            print(f"ERRO {p}: {len(cmap)} codepoints, esperava {expected_cp}"); failed = True

        # Exact monospacing: ONE advance per face, that of its own width.
        width = os2.usWidthClass
        adv = {f["hmtx"][g][0] for g in cmap.values()}
        if len(adv) != 1:
            print(f"ERRO {p}: {len(adv)} avancos distintos; nao e monoespacada"); failed = True
        elif adv.pop() != advances[width]:
            print(f"ERRO {p}: avanco errado para usWidthClass {width}"); failed = True
        f.close()

    if failed:
        sys.exit(1)
    print("OK: 135 faces, 18.053 codepoints cada, monoespacadas, upem 1000")
    print("OK: celula = 0,600 em na largura normal — a aritmetica do rodape vale")
    PY
  '';

  # The guard of section 6.3: the Nerd Fonts version comes from the FILE ITSELF
  # (nameID 5), not from a constant. If the font is updated without the
  # matching glyphnames.json, the PUA codepoints move silently.
  fonts-nerdfonts-version = mkCheck "fonts-nerdfonts-version" [ py ] ''
    python3 - <<'PY'
    from fontTools.ttLib import TTFont
    import re, sys

    p = "${fonts.corrected}/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf"
    v = TTFont(p, lazy=True)["name"].getDebugName(5)
    m = re.search(r"Nerd Fonts (\d+\.\d+\.\d+)", v or "")
    if not m:
        print(f"ERRO: nameID 5 nao declara versao do Nerd Fonts: {v!r}"); sys.exit(1)

    expected = "3.4.0"   # has to match the flake's `nerd-fonts` input
    if m.group(1) != expected:
        print(f"ERRO: fonte tem Nerd Fonts {m.group(1)}, o flake pina {expected}.")
        print("      Atualize o input nerd-fonts junto com a fonte, ou os")
        print("      codepoints da PUA andam sem ninguem notar.")
        sys.exit(1)
    print(f"OK: Nerd Fonts {m.group(1)} casa com o input do flake")
    PY
  '';

  # The icon table is generated from the REAL SOURCE FILE (section 6.3), never
  # hard-coded. This check proves that the generation happens and that the
  # result matches what the inventory measured.
  icons-table-generated = mkCheck "icons-table-generated" [ pkgs.jq ] ''
    db=${glyphdb.db}

    for f in icons.json glyphs.json coverage.md; do
      test -s "$db/$f" || { echo "ERRO: $f nao foi gerado" >&2; exit 1; }
    done

    n=$(jq 'length' "$db/icons.json")
    echo "tokens de icone: $n"
    [ "$n" -ge 40 ] || { echo "ERRO: tabela de icones suspeitamente pequena" >&2; exit 1; }

    # Every token has a glyph, a codepoint and a char — and the codepoint is in the PUA.
    jq -e 'to_entries | all(.value | has("glyph") and has("codepoint") and has("char"))' \
      "$db/icons.json" >/dev/null \
      || { echo "ERRO: entrada de icone incompleta" >&2; exit 1; }

    g=$(jq 'length' "$db/glyphs.json")
    echo "glifos indexados: $g"
    [ "$g" -eq 10473 ] || {
      echo "ERRO: esperava 10473 glifos Nerd Fonts, achei $g." >&2
      echo "      Se a fonte mudou, o inventario em docs/ARCHITECTURE.md" >&2
      echo "      precisa ser refeito antes de seguir." >&2
      exit 1
    }

    # The 126 without a name have to stay in the index: dropping them would be
    # lying about what the font has (section 6.5).
    unnamed=$(jq '[.[] | select(.name == null)] | length' "$db/glyphs.json")
    echo "glifos sem nome no glyphnames.json: $unnamed"
    [ "$unnamed" -eq 126 ] || { echo "ERRO: esperava 126 sem nome" >&2; exit 1; }

    echo "OK: tabela de icones e base de glifos geradas do arquivo real"
  '';

  # The guard of section 6.3 is only worth anything if it is tested. This check
  # builds the negative fixture, which ONLY passes if the generator refused a
  # token that does not exist.
  icons-unknown-token-fails = mkCheck "icons-unknown-token-fails" [ ] ''
    test -f ${glyphdb.negativeFixture}/out_text.txt \
      || { echo "ERRO: fixture negativo nao produziu saida" >&2; exit 1; }
    grep -q "tokens de icone que nao resolvem" ${glyphdb.negativeFixture}/out_text.txt \
      || { echo "ERRO: fixture negativo nao falhou pela razao certa" >&2; exit 1; }
    echo "OK: token de icone inexistente derruba o build (secao 6.3)"
  '';

  # fontconfig <-> renderer parity — section 6.4.
  #
  # "The renderer does NOT use fontconfig... The shell needs its own fallback
  # chain, COHERENT with fontconfig's."
  #
  # Both come out of nix/fontconfig.nix, from a single list. This check proves
  # that they do agree: if they diverge, the same text renders with different
  # faces inside and outside our buffers, and the symptom ("the accent looks
  # strange in the terminal") is expensive to trace back to here.
  #
  # The renderer's side is simulated by the declared order — it only exists in
  # milestone 3, but the behavior is already determined by the data. When the
  # renderer exists, this script becomes the oracle it is measured against.
  fonts-renderer-parity =
    let
      fontDirs = [
        "${fonts.noctua-iosevka-ttf-nerdfonts}/share/fonts"
        "${pkgs.noto-fonts}/share/fonts"
        "${pkgs.noto-fonts-cjk-sans}/share/fonts"
        "${pkgs.noto-fonts-color-emoji}/share/fonts"
      ];
      # OUR system's fontconfig, not the host's: with our rules and only our
      # fonts. Comparing against the host's fontconfig would prove nothing
      # about the distro.
      #
      # A REAL conf.d, ordered by name — not a sequence of <include> by hand.
      #
      # This matters: including our conf before or after the block of defaults
      # gives different results, and neither of the two reproduces the real
      # system. fontconfig reads `conf.d` in order of file NAME, and that is how
      # `50-ronoctua.conf` positions itself among the default rules. Testing with
      # another order measures a configuration that is never going to exist.
      confd = pkgs.runCommand "ronoctua-test-confd" { } ''
        mkdir -p $out
        ln -s ${pkgs.fontconfig.out}/etc/fonts/conf.d/*.conf $out/ 2>/dev/null || true
        ln -s ${fontconfigConf.confDir}/etc/fonts/conf.d/50-ronoctua.conf $out/50-ronoctua.conf
      '';

      fontsConf = pkgs.writeText "fonts.conf" ''
        <?xml version="1.0"?>
        <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
        <fontconfig>
          ${lib.concatMapStrings (d: "<dir>${d}</dir>\n  ") fontDirs}
          <cachedir>/tmp/fc-cache</cachedir>
          <include ignore_missing="yes">${confd}</include>
        </fontconfig>
      '';
    in
    mkCheck "fonts-renderer-parity"
      [
        py
        pkgs.fontconfig
      ]
      ''
        export FONTCONFIG_FILE=${fontsConf}
        mkdir -p /tmp/fc-cache
        fc-cache -f >/dev/null 2>&1 || true

        echo "### fc-match nos tres genericos ###"
        failed=0
        for g in monospace sans-serif serif; do
          m=$(fc-match -f '%{family}' "$g")
          echo "  $g -> $m"
          case "$m" in
            *"NoctuaIosevka Nerd Font Mono"*) ;;
            *) echo "ERRO: '$g' nao resolve para a nossa fonte (secao 6.4)" >&2; failed=1 ;;
          esac
        done
        [ $failed -eq 0 ] || exit 1
        echo

        python3 ${../nix/scripts/check-font-parity.py} \
          --chain-json ${fontconfigConf.chainJson} \
          --font-dir ${fonts.noctua-iosevka-ttf-nerdfonts}/share/fonts \
          --strict
      '';

  # Icon superpack — section 6.6. The manifest has to cover 100% of the names
  # that went in, and the origin licenses all have to be recorded: the
  # repository is publishable (section 9), so "where did this icon come from
  # and under what terms" needs an answer.
  icons-superpack = mkCheck "icons-superpack" [ pkgs.jq ] ''
    sp=${iconSuperpack}
    doc=$sp/share/doc/noctua-icons
    themeName=$sp/share/icons/noctua

    test -f $themeName/index.theme || { echo "ERRO: index.theme ausente" >&2; exit 1; }
    test -f $doc/manifest.json || { echo "ERRO: manifesto ausente" >&2; exit 1; }
    test -f $doc/coverage.md || { echo "ERRO: relatorio de cobertura ausente" >&2; exit 1; }

    # Inheritance: hicolor last, as the freedesktop spec requires.
    grep -q '^Inherits=hicolor$' $themeName/index.theme \
      || { echo "ERRO: index.theme nao herda hicolor por ultimo" >&2; exit 1; }

    # Every entry declares a winner and a license — without that the manifest
    # does not answer the question it exists for.
    n=$(jq '.entry_count' $doc/manifest.json)
    complete_ones=$(jq '[.entries[] | select(.winner != null and .license != null)] | length' $doc/manifest.json)
    echo "entradas no manifesto: $n"
    [ "$n" -eq "$complete_ones" ] || {
      echo "ERRO: $((n - completas)) entradas sem vencedor ou sem licenca" >&2; exit 1; }

    # Every directory cited in index.theme really exists.
    absent=0
    for d in $(grep '^Directories=' $themeName/index.theme | cut -d= -f2 | tr ',' ' '); do
      [ -d "$themeName/$d" ] || { echo "ERRO: diretorio declarado mas ausente: $d" >&2; absent=1; }
    done
    [ $absent -eq 0 ] || exit 1

    # Origin licenses recorded, one per theme that went into the merge.
    for t in $(jq -r '.sources[].name' $doc/manifest.json); do
      test -s "$doc/LICENSES/$t.txt" \
        || { echo "ERRO: licenca de origem ausente para $t" >&2; exit 1; }
    done

    echo "OK: manifesto cobre 100% das entradas, com vencedor e licenca"
    echo "OK: licencas de origem gravadas para todos os temas fundidos"
  '';

  # THE CHAIN'S PIXMAP STEP, against the REAL superpack.
  #
  # The `icons-superpack` above proves that the PACKAGE is well formed; this
  # one proves that the chain REACHES it. They are different things, and the
  # second fails silently: without it, `procurar` would give back `None` for
  # everything, the application list would be left with only the category
  # glyphs — and a whole list of category glyphs LOOKS DELIBERATE. It is the
  # most expensive defect of this path, because it has no symptom at all.
  #
  # It counts OPAQUE pixels: finding the file is not drawing it, and an SVG
  # that rasterizes empty gives an invisible smudge on the dark background —
  # which is indistinguishable from "the icon has not arrived yet".
  apps-icon-from-superpack = mkCheck "apps-icon-from-superpack" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-icons \
      ${iconSuperpack}/share/icons/noctua
  '';

  # ===================================================================
  # NETWORK AND SYNCTHING — does what was TUNED actually reach the system?
  #
  # These two checks evaluate the real NixOS configuration, and not the
  # module's text. The difference matters: a `grep` in the `.nix` would pass
  # with the value written and shadowed by an `mkIf` that never turns on, or
  # with an option renamed by nixpkgs — both cases give a system without the
  # adjustment and a green check. That is exactly how the tray table already
  # passed the build and never reached the Rust.
  # ===================================================================
  networkEval = (pkgs.nixos [ ../nix/modules/network.nix ]).config;
  syncthingEval =
    (pkgs.nixos [
      ../nix/modules/syncthing.nix
      # The Syncthing module reads `config.ronoctua.user`, which belongs to base.
      {
        options = { };
        config.ronoctua.user = "noctua";
      }
      { options.ronoctua.user = lib.mkOption { type = lib.types.str; }; }
    ]).config;

  # The Wi-Fi password sealed in the TPM, against a real TPM (swtpm).
  #
  # The defect it catches is THE PASSWORD STAYING ON DISK — not "the daemon
  # does not come up". A check that only looked at the unit would pass with
  # `psk=<password>` inside the keyfile, which is precisely what this work
  # eliminates.
  wifiTpmVm = import ./wifi-tpm-vm.nix {
    inherit pkgs lib;
    ronoctua-shell = rust.workspace;
  };

  network-tuned = mkCheck "network-tuned" [ ] ''
    expected() {
      if [ "$2" != "$3" ]; then
        echo "FALHOU: $1 = $2, esperado $3" >&2
        echo "" >&2
        echo "Este valor NAO e decorativo: ele saiu de medir uma" >&2
        echo "transferencia de 592 GB entre duas maquinas deste projeto." >&2
        exit 1
      fi
    }
    # The single biggest gain from the radio. With power saving on, the
    # throughput halves and nothing on screen says why.
    expected "wifi.powersave"       "${lib.boolToString networkEval.networking.networkmanager.wifi.powersave}" "false"
    # BBR without `fq` works WORSE than cubic: it is the classic mistake of
    # turning on only half of it, and that is why both are checked together.
    expected "tcp_congestion_control"       "${
      toString networkEval.boot.kernel.sysctl."net.ipv4.tcp_congestion_control"
    }" "bbr"
    expected "default_qdisc"       "${
      toString networkEval.boot.kernel.sysctl."net.core.default_qdisc"
    }" "fq"
    # The costliest trap in a synchronization: Syncthing pauses between files,
    # and without this every file starts over in slow start.
    expected "tcp_slow_start_after_idle"       "${
      toString networkEval.boot.kernel.sysctl."net.ipv4.tcp_slow_start_after_idle"
    }" "0"
    # Without the module loaded, the BBR sysctl fails at boot — and the failure
    # shows up in the journal, not on the screen.
    case " ${lib.concatStringsSep " " networkEval.boot.kernelModules} " in
      *" tcp_bbr "*) ;;
      *) echo "FALHOU: tcp_bbr nao esta em boot.kernelModules" >&2; exit 1 ;;
    esac
    echo "OK: powersave desligado, BBR+fq, partida lenta desarmada"
  '';

  syncthing-tuned = mkCheck "syncthing-tuned" [ ] ''
    # ================================================================
    # THE MOST IMPORTANT CHECK IN THIS FILE, and the least obvious.
    #
    # `overrideFolders` is `true` in nixpkgs, and `true` means "whatever is
    # not declared in Nix is DELETED on activation". This module declares no
    # folder at all, because a folder path is personal data (invariant 13) —
    # so the default would DELETE EVERY FOLDER on each `nixos-rebuild`,
    # including the ones the person has just added through the interface,
    # with no warning and no undo.
    #
    # There is no symptom before the damage. Hence the check.
    # ================================================================
    if [ "${lib.boolToString syncthingEval.services.syncthing.overrideFolders}" != "false" ]; then
      echo "FALHOU: overrideFolders ligado APAGA as pastas do usuario" >&2
      exit 1
    fi
    if [ "${lib.boolToString syncthingEval.services.syncthing.overrideDevices}" != "false" ]; then
      echo "FALHOU: overrideDevices ligado APAGA os dispositivos pareados" >&2
      exit 1
    fi
    # UPnP failed every 30 minutes on the measured machine, forever.
    if [ "${lib.boolToString syncthingEval.services.syncthing.settings.options.natEnabled}" != "false" ]; then
      echo "FALHOU: natEnabled ligado numa sincronizacao de LAN" >&2
      exit 1
    fi
    # The relay is a very slow fallback route: with it, a momentary discovery
    # failure drops the transfer to ~1 MB/s with nothing saying so.
    if [ "${lib.boolToString syncthingEval.services.syncthing.settings.options.relaysEnabled}" != "false" ]; then
      echo "FALHOU: relaysEnabled deixa a transferencia cair para o relay" >&2
      exit 1
    fi
    # The package comes from Nix; self-updating would try to write to /nix/store.
    if [ "${builtins.toString syncthingEval.services.syncthing.settings.options.autoUpgradeIntervalH}" != "0" ]; then
      echo "FALHOU: auto-atualizacao ligada num pacote do store" >&2
      exit 1
    fi
    echo "OK: override desligado, sem NAT, sem relay, sem auto-upgrade"
  '';

  # The token generator REFUSES a name that does not become a WIT identifier.
  #
  # The real defect: `wifi-0` was accepted here and broke the build three
  # derivations later, inside a WASM buffer, with the message "invalid
  # character in identifier '0'" pointing at the generated enum — which does
  # not say where the name came from. The check tests BOTH sides: the good
  # name passes, the bad one is refused.
  tokens-wit-name = mkCheck "tokens-wit-name" [ pkgs.python3 ] ''
    python3 - <<'FIM'
    import importlib.util
    spec = importlib.util.spec_from_file_location("gt", "${../nix/scripts/gen-tokens.py}")
    gt = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gt)

    for bom in ["wifi-fraco", "rede-protegida", "surface.base", "app-firefox"]:
        gt.wit_ident(bom)

    for bad in ["wifi-0", "sinal-4", "cor-2x"]:
        try:
            gt.wit_ident(bad)
        except SystemExit:
            continue
        raise SystemExit(f"FALHOU: `{bad}` passou, e o WIT o recusaria")
    print("OK: o gerador recusa nome que o WIT nao aceita")
    FIM
  '';

  # ===================================================================
  # INVARIANT 18 — every NAME is English; every COMMENT is English too.
  #
  # The `CLAUDE.md` already said "identifiers in English" and it was never met:
  # there were 54 ids, 90 checks, ~35 files and 358 keys in Portuguese. A rule
  # with no check is a recommendation, and a recommendation is what produced
  # that distance between what the repository asserts and what it does.
  #
  # WHY A DENY LIST, AND NOT "detect Portuguese"
  # --------------------------------------------
  # Detecting a language in general is impossible in a check. Refusing EXACTLY
  # the words that have already been here is possible, honest, and it is what
  # stops the regression — which is the real risk: someone adds `rede-abrir`
  # again out of habit.
  #
  # IT SWEEPS NAMES, NOT PROSE. It looks at: file paths, the `.ftl` key column,
  # and — since the identifier pass — every Rust declaration: `fn`, `struct`,
  # `enum`, `trait`, `const`, `static` and `mod`.
  #
  # It does NOT sweep comments, and that is not because they are exempt —
  # the owner moved them to English, so the second half of the invariant now
  # covers only the four documents, the key VALUES and the search tags.
  # Detecting "this comment is in Portuguese" is the same impossible
  # language detection the deny list exists to avoid; what guards the
  # comments is the sweep that translated them, not a check.
  #
  # THE DECLARATION IS READ ON ITS OWN LINE, and the sweep is deliberately
  # blind to a use: `let rede = …` is not caught here. Catching every local
  # would mean parsing Rust, and a check that needs a parser to be right is a
  # check that goes wrong on its own. What it does catch is what OTHER code
  # sees — and that is where a Portuguese name spreads from.
  english-names = mkCheck "english-names" [ pkgs.python3 ] ''
    python3 - <<'FIM'
    import pathlib, re, sys

    # The stems this refactor removed. Adding here is cheap; taking one out
    # requires explaining why the word became acceptable again.
    FORBIDDEN = [
         "aberto", "abrir", "acao", "achada", "achados", "achou", "acoes",
         "afinar", "agente", "agulha", "ajuda", "altura", "alvo", "anotacao",
         "anterior", "antes", "antigo", "argumento", "arquivo", "arquivos",
         "arrastar", "arvore", "atribuicao", "avanco", "baixo", "banda",
         "binario", "bloco", "bruto", "busca", "cadeia", "camada", "caminho",
         "capacidade", "captura", "capturando", "capturar", "carimbo", "caso",
         "casou", "catalogo", "categoria", "categorias", "celula", "chave",
         "chaves", "cima", "cliente", "cobertura", "cofre", "colado",
         "colisao", "colocado", "coluna", "colunas", "comandar", "comando",
         "comandos", "combinante", "completa", "conectar", "configuracao",
         "conjunto", "contexto", "contrato", "controlar", "copiar", "cor",
         "corridas", "declarada", "depois", "desconectar", "descritor",
         "desloc", "detalhe", "diagnostico", "diario", "dimensoes", "direita",
         "discos", "dispositivo", "dispositivos", "dito", "dominio", "eco",
         "efeito", "efeitos", "elemento", "elidir", "emergencia", "encaixe",
         "encerrar", "encolhe", "enrolar", "entalhes", "entrada", "escala",
         "escrita", "escutar", "esperada", "esperado", "esquecer", "esquerda",
         "estado", "estatico", "estrangeiro", "eventos", "executar",
         "expandido", "expansao", "fabrica", "falhou", "falta", "faltando",
         "familia", "fechar", "ficha", "fila", "filhos", "filtro", "fluxo",
         "fonte", "fontes", "fora", "fragmento", "frase", "frequencia",
         "fundo", "gesto", "glifo", "glifos", "grade", "grafico", "gravada",
         "grupo", "guarda", "guardar", "heranca", "hospedeiro", "icone",
         "icones", "idioma", "imagens", "impermanencia", "instalador",
         "instanciar", "interatividade", "janela", "juntos", "lado",
         "lancamento", "largo", "largura", "legivel", "levar", "lido",
         "ligacao", "limite", "limites", "limpar", "linha", "linhas",
         "localizador", "marca", "meio", "modelo", "modo", "morreu", "morto",
         "motor", "mover", "navegacao", "navegador", "nome", "nomes",
         "notificacao", "novo", "observador", "opcao", "orcamento", "ordem",
         "origem", "outra", "pacote", "padrao", "padroes", "painel", "partes",
         "passo", "passos", "pedaco", "pedacos", "pendencia", "perfil",
         "perfis", "pergunta", "pintura", "plano", "politica", "posicao",
         "prefixo", "preludio", "presente", "primeiro", "programa", "proibido",
         "proximo", "puxar", "quantos", "raiz", "ramo", "recarrega",
         "reconstruir", "rede", "redesenhar", "repeticao", "resolucao",
         "revarrer", "rodape", "rolagem", "rotulo", "ruim", "saida", "sair",
         "segmento", "segmentos", "segredo", "segundo", "selar", "senha",
         "sequencia", "servidor", "sigilo", "simbolo", "simbolos", "sobrando",
         "sobrou", "sono", "sorteia", "sorteio", "subida", "subir",
         "superficie", "surdo", "tabela", "tam", "tamanho", "teclado", "tela",
         "tema", "teto", "texto", "textos", "tipografia", "topo", "trecho",
         "trechos", "ultima", "ultimo", "unidade", "urgencia", "usuario",
         "valor", "variante", "variavel", "varrer", "vazia", "vazio", "versao",
         "vetorial", "vigia", "visao", "vista", "visto", "vivo", "volta",
         "voltou",
    ]
    RX = re.compile(r"(?<![a-z])(" + "|".join(FORBIDDEN) + r")s?(?![a-z])")

    # Every Rust declaration that becomes a name OTHER code can see.
    DECL = re.compile(
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+|unsafe\s+|const\s+|extern\s+\S+\s+)*"
        r"(fn|struct|enum|trait|type|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
    )

    root_dir = pathlib.Path("${self}")
    gaps = []

    # 1) FILE AND DIRECTORY NAMES
    for p in root_dir.rglob("*"):
        if any(x in p.parts for x in (".git", "result", "target", ".direnv")):
            continue
        if RX.search(p.name.lower().replace("_", "-")):
            gaps.append(f"arquivo: {p.relative_to(root_dir)}")

    # 2) THE .ftl KEY COLUMN — the value stays in pt-BR, only the name changes
    for ftl in (root_dir / "assets/i18n").glob("*.ftl"):
        for n, line in enumerate(ftl.read_text().splitlines(), 1):
            m = re.match(r"^([a-z][a-z0-9-]*) *=", line)
            if m and RX.search(m.group(1)):
                gaps.append(f"chave: {ftl.name}:{n}: {m.group(1)}")

    # 3) RUST DECLARATIONS — the half of the invariant the identifier pass closed
    #
    # A VARIANT IS A DECLARATION TOO, and reading only `enum Foo` missed it:
    # `Question::Senha`, `Question::Idioma` and `Question::WifiSenha` came through
    # the whole identifier refactor untouched, because the line that names them
    # has no keyword on it — just an indented capitalized word.
    VARIANT = re.compile(r"^\s+([A-Z][A-Za-z0-9_]*)\s*(?:[,({=]|$)")
    # A FIELD IS A DECLARATION TOO, and the same blind spot: the line that names
    # it carries no keyword, only an indented lowercase word and a colon. That is
    # how `Minibuffer.segredo` came through the whole identifier refactor.
    FIELD = re.compile(r"^\s+(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:")
    for base in ("crates", "buffers"):
        for rs in (root_dir / base).rglob("*.rs"):
            if "target" in rs.parts:
                continue
            in_enum = 0
            in_struct = 0
            for n, line in enumerate(rs.read_text().splitlines(), 1):
                m = DECL.match(line)
                if m:
                    # CamelCase and SNAKE_CASE become the same ruler as the file name.
                    item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", m.group(2)).lower().replace("_", "-")
                    if RX.search(item_name):
                        rel = rs.relative_to(root_dir)
                        gaps.append(f"{m.group(1)}: {rel}:{n}: {m.group(2)}")
                    in_enum = 1 if m.group(1) == "enum" else 0
                    in_struct = 1 if m.group(1) == "struct" else 0
                    continue
                if line.startswith("}"):
                    in_enum = 0
                    in_struct = 0
                    continue
                if in_struct:
                    f = FIELD.match(line)
                    if f:
                        item_name = f.group(1).replace("_", "-")
                        if RX.search(item_name):
                            gaps.append(f"campo: {rs.relative_to(root_dir)}:{n}: {f.group(1)}")
                    continue
                if not in_enum:
                    continue
                # The body ends at the closing brace in column 0.
                if line.startswith("}"):
                    in_enum = 0
                    continue
                v = VARIANT.match(line)
                if not v:
                    continue
                item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", v.group(1)).lower().replace("_", "-")
                if RX.search(item_name):
                    gaps.append(f"variante: {rs.relative_to(root_dir)}:{n}: {v.group(1)}")

    # 4) NIX AND PYTHON DEFINITIONS
    #
    # `english-names` used to stop at Rust, and the build layer drifted out of
    # sight behind it: `fonteEmoji`, `versaoDeclarada`, `hostGrafico`, plus every
    # variable inside the VM tests' embedded scripts. A `let` binding and a shell
    # assignment are names too, and this is the ruler that reaches them.
    DEF = re.compile(r"^\s*(?:def\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(|([A-Za-z_][A-Za-z0-9_]*)\s*=(?!=))")
    for suf in ("*.nix", "*.py"):
        for f in root_dir.rglob(suf):
            if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
                continue
            for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1):
                # Prose, not a definition: `fontes (passa de 1 GB)` inside a
                # comment matches the same shape as a call.
                if line.lstrip().startswith(("#", "//")):
                    continue
                m = DEF.match(line)
                if not m:
                    continue
                got = m.group(1) or m.group(2)
                item_name = re.sub(r"(?<!^)(?=[A-Z])", "-", got).lower().replace("_", "-")
                if RX.search(item_name):
                    gaps.append(f"def: {f.relative_to(root_dir)}:{n}: {got}")

    # 5) THE CLI'S SUBCOMMANDS
    #
    # These live as STRINGS in a `match` arm, so no declaration rule reaches
    # them: `comando`, `satelite`, `verificar-keymap` and `exportar-binds-niri`
    # went through the whole refactor untouched. They are commands, which
    # invariant 18 names explicitly.
    for rs in (root_dir / "crates").rglob("main.rs"):
        if "target" in rs.parts:
            continue
        for n, line in enumerate(rs.read_text().splitlines(), 1):
            for m in re.finditer(r'Some\("([a-z][a-z0-9-]*)"\)', line):
                if RX.search(m.group(1)):
                    gaps.append(f"subcomando: {rs.relative_to(root_dir)}:{n}: {m.group(1)}")

    if gaps:
        print("ERRO: nome em portugues (invariante 18).", file=sys.stderr)
        print("", file=sys.stderr)
        for f in gaps[:40]:
            print(f"       {f}", file=sys.stderr)
        if len(gaps) > 40:
            print(f"       … e mais {len(gaps) - 40}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Todo NOME e todo COMENTARIO sao ingles.", file=sys.stderr)
        print("       Em pt-BR so os 4 docs, o VALOR de chave e a tag de busca.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: nenhum nome em portugues — arquivo, chave, Rust, Nix, Python, subcomando")
    FIM
  '';

  # The flag a `.nix` PASSES has to be one the generator DECLARES.
  #
  # It is a name crossing the boundary as a string, exactly like the WIT: nothing
  # ties `add_argument("--teto-sem-pt")` to the `--teto-sem-pt` in the caller, so
  # renaming one side compiles, evaluates, and dies at build time with
  # `unrecognized arguments` — pointing at the script, not at the rename. It
  # happened twice in the same pass (`--teto-sem-pt`, `--categorias`), which is
  # what earned it a check.
  generator-flags-agree = mkCheck "generator-flags-agree" [ pkgs.python3 ] ''
    python3 - <<'FIM'
    import collections, pathlib, re, sys

    root_dir = pathlib.Path("${self}")
    declared = collections.defaultdict(set)
    for p in (root_dir / "nix/scripts").glob("*.py"):
        for m in re.finditer(r'add_argument\(\s*"(--[a-z0-9-]+)"', p.read_text()):
            declared[p.name].add(m.group(1))

    # `cp --reflink` and friends: a flag of a coreutils call, not of the script.
    NOT_OURS = {"--reflink"}
    gaps = []
    for p in root_dir.rglob("*.nix"):
        if any(x in p.parts for x in (".git", "result", "target", ".direnv")):
            continue
        t = p.read_text(errors="replace")
        for m in re.finditer(r"(gen-[a-z-]+\.py|scripts/[a-z-]+\.py)([^\n]*(?:\n\s+[^\n]*){0,14})", t):
            script = m.group(1).split("/")[-1]
            if script not in declared:
                continue
            for f in re.findall(r"(--[a-z][a-z0-9-]+)", m.group(2)):
                if f not in declared[script] and f not in NOT_OURS:
                    n = t.count("\n", 0, m.start()) + 1
                    gaps.append(f"{p.relative_to(root_dir)}:{n}: {script} recebe {f}")

    if gaps:
        print("ERRO: flag passada que o gerador nao declara.", file=sys.stderr)
        for g in sorted(set(gaps))[:20]:
            print(f"       {g}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       O nome da flag atravessa a fronteira como STRING: os dois", file=sys.stderr)
        print("       lados mudam na mesma passada, ou nada acusa ate o build.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: toda flag passada e declarada pelo gerador que a recebe")
    FIM
  '';

  # A `{name}` inside an f-string with no `name` in scope.
  #
  # It is what a rename leaves behind when it reaches the capture and not the
  # assignment, and NOTHING catches it: Python only raises on the line that runs,
  # so it hid on the "not measured" path of `nix run .#bench` and on the summary
  # line the bench prints every time. Three of them were live at once.
  #
  # In a `.nix` file the embedded script has to be isolated from the Nix around
  # it — otherwise a Nix attribute `name = …` counts as a Python definition and
  # hides exactly the orphan being hunted. And the fence does not end at the
  # first `''`: `'''`, `''$` and `''\` are escapes.
  format-captures-resolve = mkCheck "format-captures-resolve" [ pkgs.python3 ] ''
    python3 - <<'FIM'
    import builtins, pathlib, re, sys

    CAP = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)")
    FSTR = re.compile(r"""(?:\bf|\bfr|\brf)(\"\"\"|'''|\"|')""")
    DEFINE = [
        re.compile(r"^\s*\(?([A-Za-z_*][A-Za-z0-9_,\s*\[\]().]*?)\)?\s*(?::[^=\n]+)?=[^=]", re.M),
        re.compile(r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)", re.M),
        re.compile(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)", re.M),
        re.compile(r"\bfor\s+\(?([A-Za-z_][A-Za-z0-9_,\s()\[\]]*?)\)?\s+in\b", re.M),
        re.compile(r"\bas\s+\(?([A-Za-z_][A-Za-z0-9_,\s()]*)", re.M),
        re.compile(r"^\s*import\s+([A-Za-z_][A-Za-z0-9_.]*)", re.M),
        re.compile(r"\bdef\s+[A-Za-z_][A-Za-z0-9_]*\s*\(([^)]*)\)", re.M),
        re.compile(r"\bglobal\s+([A-Za-z_][A-Za-z0-9_,\s]*)", re.M),
        re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*:=", re.M),
        re.compile(r"\blambda\s+([A-Za-z_][A-Za-z0-9_,\s]*?):", re.M),
    ]
    DRIVER = {"machine", "subtest", "start_all", "nodes", "t", "polling_condition"}

    def defined(t):
        s = set(dir(builtins)) | DRIVER
        for p in DEFINE:
            for m in p.finditer(t):
                for part in re.split(r"[,\s()\[\]]+", m.group(1)):
                    part = part.split("=")[0].split(":")[0].split(".")[0].strip("*")
                    if part:
                        s.add(part)
        return s

    # The fence delimiter, built rather than written: a literal pair of quotes
    # in here would close the Nix string this script lives in.
    FENCE = chr(39) * 2

    def fences(t):
        """Every Nix fence. It does NOT end at the first delimiter: a tripled
        quote, and a delimiter followed by `$` or by a backslash, are escapes.
        Reading one of those as the terminator drops most of the script."""
        i = 0
        while True:
            a = t.find(FENCE, i)
            if a < 0:
                return
            j = a + 2
            while j < len(t):
                if not t.startswith(FENCE, j):
                    j += 1
                    continue
                if j + 2 < len(t) and t[j + 2] in chr(39) + "$" + chr(92):
                    j += 3
                    continue
                break
            yield a + 2, t[a + 2 : j]
            i = j + 2

    def fstrings(t):
        i = 0
        while True:
            m = FSTR.search(t, i)
            if not m:
                return
            q = m.group(1)
            j = m.end()
            while j < len(t):
                if t[j] == "\\":
                    j += 2
                    continue
                if t.startswith(q, j):
                    break
                j += 1
            yield m.end(), t[m.end() : j]
            i = j + len(q)

    root_dir = pathlib.Path("${self}")
    # Every VM test splices this prelude into its own script.
    pre = root_dir / "tests/niri-xvfb.nix"
    prelude = "\n".join(b for _, b in fences(pre.read_text())) if pre.exists() else ""

    orphans = []
    for p in sorted(root_dir.rglob("*")):
        if p.suffix not in (".nix", ".py") or not p.is_file():
            continue
        if any(x in p.parts for x in (".git", "result", "target", ".direnv")):
            continue
        t = p.read_text(errors="replace")
        regions = [(0, t)] if p.suffix == ".py" else list(fences(t))
        extra = prelude if p.suffix == ".nix" else ""
        known = defined("\n".join(b for _, b in regions) + "\n" + extra)
        for base, region in regions:
            for off, body in fstrings(region):
                for m in CAP.finditer(body):
                    if m.group(1) in known:
                        continue
                    if m.start() > 0 and body[m.start() - 1] == "{":
                        continue  # `{{` is a literal brace
                    if m.start() >= 1 and body[m.start() - 1] == "$":
                        continue  # Nix antiquotation, not a Python field
                    n = t.count("\n", 0, base + off + m.start()) + 1
                    orphans.append(f"{p.relative_to(root_dir)}:{n}: {m.group(1)}")

    if orphans:
        print("ERRO: captura de format sem o nome em escopo.", file=sys.stderr)
        for o in orphans[:30]:
            print(f"       {o}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       Python so acusa na linha que EXECUTA: um destes vive no", file=sys.stderr)
        print("       caminho de erro e so aparece quando ja ha outro problema.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: toda captura de format tem o nome em escopo")
    FIM
  '';

  # The crates: tests, clippy and formatting, each in its own derivation so it
  # says WHAT broke instead of "Rust failed".
  rust-tests = rust.tests;
  rust-clippy = rust.clippy;
  rust-fmt = rust.fmt;

  # The footer itself. The surface only comes up with a compositor, so what is
  # asserted here is what can be asserted without one: that the binary exists,
  # runs and loads the embedded face.
  shell-binary = mkCheck "shell-binary" [ ] ''
    bin=${rust.workspace}/bin/ronoctua-shell
    test -x "$bin" || { echo "ERRO: binario ausente" >&2; exit 1; }

    # With no WAYLAND_DISPLAY it must fail with a CLEAR message, not with a
    # panic. The message is the first thing anyone sees when the nested loop
    # does not come up.
    out_text=$("$bin" 2>&1 || true)
    echo "$out_text"
    echo "$out_text" | grep -q "compositor Wayland"       || { echo "ERRO: sem compositor, a mensagem de erro nao explica o porque" >&2; exit 1; }

    echo "OK: o rodape existe e falha de forma legivel sem compositor"
  '';

  # Section 9 requires the repository to be publishable AS IT IS. A local-path
  # input only works for whoever has both repositories side by side, so it must
  # NOT survive publication. This check does not fail today (the local path is
  # the correct transient state), but it shouts loudly so that carelessness does
  # not slip through.
  fonts-input-publishable = mkCheck "fonts-input-publishable" [ ] ''
    url=${lib.escapeShellArg (inputs.fonts.original.url or "path:../ronoctua-fonts")}
    case "$url" in
      path:*|/*)
        echo "AVISO: o input 'fonts' aponta para um caminho local:"
        echo "         $url"
        echo ""
        echo "       Isso funciona no seu disco, mas NAO para quem clonar."
        echo "       Antes de publicar (secao 9), troque em flake.nix por:"
        echo "         url = \"github:<user_name>/ronoctua-fonts/<rev>\";"
        echo "       e rode: nix flake lock --update-input fonts"
        ;;
      github:*|git+*)
        echo "OK: input 'fonts' e remoto e publicavel — $url"
        ;;
      *)
        echo "ERRO: input 'fonts' com forma inesperada: $url" >&2
        exit 1
        ;;
    esac
  '';

  # The OFL requires redistributing the license text along with the files.
  fonts-license = mkCheck "fonts-license" [ ] ''
    test -f ${fonts.noctua-iosevka-ttf-nerdfonts}/share/doc/noctua-iosevka/LICENSE-OFL.txt \
      || { echo "ERRO: LICENSE-OFL.txt ausente no pacote" >&2; exit 1; }
    grep -q "SIL OPEN FONT LICENSE" \
      ${fonts.noctua-iosevka-ttf-nerdfonts}/share/doc/noctua-iosevka/LICENSE-OFL.txt \
      || { echo "ERRO: LICENSE-OFL.txt nao contem o texto da OFL" >&2; exit 1; }
    echo "OK: texto da OFL redistribuido junto dos arquivos"
  '';

  # The ISO has to fit on a USB stick. The core subset exists for that.
  iso-fits-usb-stick = mkCheck "iso-fits-usb-stick" [ pkgs.coreutils ] ''
    iso=$(find ${self.packages.${system}.iso}/iso -name '*.iso' | head -1)
    bytes=$(stat -c %s "$iso")
    limit=$((4 * 1024 * 1024 * 1024))
    echo "ISO: $((bytes / 1024 / 1024)) MiB"
    if [ "$bytes" -gt "$limit" ]; then
      echo "ERRO: ISO passou de 4 GiB. O subconjunto core existe justamente" >&2
      echo "      para isso — conferir nix/installer/iso.nix." >&2
      exit 1
    fi
    echo "OK: ISO cabe em pendrive"
  '';

  # The FOUR documents exist, and the `CLAUDE.md` fits in its ceiling.
  #
  # The 150-line ceiling is not aesthetics: the `CLAUDE.md` is the only file
  # that goes into every agent's context on its own. Going past the ceiling
  # does not make it more informative — it makes it less read.
  #
  # `PENDING.md` is on the list because the "what is missing" half disappears
  # if it is not demanded: a project with no place for a pending item to live
  # goes back to hiding an overdue promise in a code comment.
  docs-exist = mkCheck "docs-exist" [ pkgs.coreutils ] ''
    for f in ${../CLAUDE.md} ${../README.md} \
             ${../docs/ARCHITECTURE.md} ${../docs/PENDING.md}; do
      test -s "$f" || { echo "ERRO: $f ausente ou vazio" >&2; exit 1; }
    done

    lines=$(wc -l < ${../CLAUDE.md})
    echo "CLAUDE.md: $lines linhas"
    if [ "$lines" -gt 150 ]; then
      echo "ERRO: CLAUDE.md passou do teto de 150 linhas." >&2
      echo "      'Se passar, corte antes de acrescentar.'" >&2
      exit 1
    fi

    # The PENDING.md must not become a changelog. Its rule is "on finishing,
    # remove it"; a struck-through or done-marked item is the start of the rot.
    # Anchored on a LIST ITEM: the rule at the top itself cites the markers in
    # order to forbid them, and a loose grep would report the text that forbids them.
    if grep -qE '^ *[-*|] *(\[x\]|~~|✅)' ${../docs/PENDING.md}; then
      echo "ERRO: PENDING.md tem item marcado como feito." >&2
      echo "      A regra e remover, nao riscar — ele nao guarda historico." >&2
      exit 1
    fi

    echo "OK: os quatro documentos existem; CLAUDE.md dentro do teto"
  '';
in
{
  # =================================================================
  # Milestones
  # =================================================================
  #
  # ===================================================================
  # MILESTONE 1 — Feedback loop before any feature.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   `nix flake check` green with empty tests; bootable ISO generated by the
  #   flake; the VM comes up to login; the three documents of §12 and the
  #   `.gitignore`/`.gitattributes` exist.
  # ===================================================================
  m1 = pkgs.linkFarmFromDrvs "check-m1" [
    # "The repository has to be publishable as it is" (section 9).
    secrets-out-of-repo
    planted-secret-fails
    # The feedback loop is m1's goal, and a tree that gets fat destroys it
    # silently: every `nix flake check` starts copying gigabytes.
    lean-tree
    gitignore-keeps-fonts
    fonts-single-family
    fonts-metrics
    fonts-nerdfonts-version
    fonts-input-publishable
    fonts-license
    icons-table-generated
    icons-unknown-token-fails
    # A token name the WIT would refuse. It stays in m1 along with the other two
    # because it is the same family of defect: the token exists, the build passes
    # here, and the failure shows up far away.
    tokens-wit-name
    icons-superpack
    fonts-renderer-parity
    rust-tests
    rust-clippy
    rust-fmt
    shell-binary
    docs-exist
    english-names
    xdg-user-dirs
    browser-has-no-keyring
    format-captures-resolve
    generator-flags-agree
    iso-fits-usb-stick
    self.packages.${system}.iso
    self.nixosConfigurations.vm.config.system.build.toplevel
  ];

  # Exposed so they can be run on their own during the work.
  footer-vm = footerVm;
  niri-contract = niriContract;
  modeline-vm = modelineVm;
  leader-vm = leaderVm;
  apps-vm = appsVm;
  inherit keymap-no-collision i18n-complete i18n-missing-key-fails;
  inherit group-mark-outside-translation niri-actions-translated keyboard-layout-exists;
  inherit
    niri-config-valid
    fontconfig-nested-finds-font
    xdg-user-dirs
    browser-has-no-keyring
    ;
  notify-vm = notifyVm;
  impermanence-vm = impermanenceVm;
  buffer-vm = bufferVm;
  clipboard-vm = clipboardVm;
  terminal-vm = terminalVm;
  x11-vm = x11Vm;
  keyboard-vm = keyboardVm;
  tpm-vm = tpmVm;
  secureboot-vm = secureBootVm;
  inherit installer-dryrun;
  inherit
    invalid-token-fails-build
    secrets-out-of-repo
    planted-secret-fails
    term-differential
    instance-fast
    browser-filters
    symbols-filters
    unicode-table-generated
    unicode-version-mismatch-fails
    unicode-instance-fast
    lucide-filters
    lucide-table-generated
    lucide-range-taken-fails
    lucide-instance-fast
    renderer-loads-three-faces
    layout-fast
    shaping-fast
    large-monitor-surface
    reload-reloads
    footer-is-single
    footer-logs-to-file
    dev-exits-clean
    english-names
    format-captures-resolve
    generator-flags-agree
    ;

  # VM test of the disk layout (section 7.2). Heavy: it brings up a VM with
  # KVM, partitions a real disk and mounts it. Exposed separately so it can be
  # run on its own during the work.
  disko-vm = import ./disko-vm.nix {
    inherit pkgs lib;
    inherit (inputs) disko;
  };

  # ===================================================================
  # MILESTONE 2 — NixOS base + the whole font and icon layer.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   The VM boots from a disk partitioned by disko with LUKS2 + the 7
  #   subvolumes of §7.2; `fc-match` gives back our font in serif/sans/mono;
  #   renderer and fontconfig agree on the probe codepoints; a negative fixture
  #   with an icon token that does not exist **fails the build**; the
  #   superpack's manifest covers 100% of the input names; Ghostty installed.
  # ===================================================================
  m2 = pkgs.linkFarmFromDrvs "check-m2" [
    # font layer
    fonts-renderer-parity
    # icon layer
    icons-table-generated
    icons-unknown-token-fails
    icons-superpack
    # system: the full VM evaluates and builds, with Ghostty and the superpack
    self.nixosConfigurations.vm.config.system.build.toplevel
  ];

  # ===================================================================
  # MILESTONE 3 — Two-line footer, in the right font, in Rust.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   A single layer-shell surface, bottom anchor, exclusive zone of 2 lines,
  #   writing a fixed state; the nested loop (`nix run .#dev`) brings up niri
  #   + shell with hot reload; sops-nix adopted.
  # ===================================================================
  m3 = pkgs.linkFarmFromDrvs "check-m3" [
    rust-tests
    rust-clippy
    rust-fmt
    shell-binary
    # The milestone's real proof: the surface is accepted by a real compositor,
    # wgpu assembles a context over it and a frame comes out. None of that shows
    # up in `cargo build` — all four are runtime.
    footerVm
    # The nested loop, and the proof that it RELOADS — not only that it exists.
    devApp
    reload-reloads
    # And that restarting RESTARTS: `reload-reloads` proves that the command
    # fires, this one proves that exactly one footer is left.
    footer-is-single
    footer-logs-to-file
    dev-exits-clean
    # The two GPU-independent gates of section 9.3, which protect this
    # milestone's frame budget.
    layout-fast
    shaping-fast
    # And the resolution ceiling: the footer takes the SCREEN's width, so a
    # texture limit below it kills the whole milestone on a big monitor.
    large-monitor-surface
  ];

  # ===================================================================
  # MILESTONE 4 — `ronoctua-niri` + contract, and a live modeline.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   The contract test passes against the pinned tag **and** against the
  #   `niri-next` input; the daemon consumes the event stream; the modeline
  #   reflects the focused buffer in a screenshot with OCR.
  # ===================================================================
  m4 = pkgs.linkFarmFromDrvs "check-m4" [
    # The contract against BOTH versions. It is the subtest against `niri-next`
    # that gives the drift warning before the release becomes mandatory (risk R2).
    niriContract
    # The daemon consuming the event stream, proved on screen by OCR.
    modelineVm
  ];

  # ===================================================================
  # MILESTONE 5 — Leader, keymap and which-key in §4's geometry.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   The leader captures the keyboard via `keyboard-interactivity:
  #   exclusive`; which-key expands **upwards** with the list **below** the
  #   minibuffer; `Esc` goes back to 2 lines; the keymap collision check
  #   passes; the §7 elision property test passes.
  # ===================================================================
  m5 = pkgs.linkFarmFromDrvs "check-m5" [
    # Leader, which-key on screen, Esc, and the command fired.
    leaderVm
    # The default keymap does not collide. `rust-tests` already runs the same
    # unit test; this is the command a PERSON runs after editing their own
    # keymap, and that is why it is worth existing separately.
    keymap-no-collision
    # The elision property test lives in `rust-tests` (5,000 cases drawn by
    # `layout::tests::property_*`).
    rust-tests
  ];

  # ===================================================================
  # MILESTONE 6 — Design system, i18n and notifications.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   Tokens generated from Nix; the `text-key` sweep fails if a key is
  #   missing in `pt-BR` or `en-US`; notifications queued **one at a time**; a
  #   critical one does not disappear on its own; a filterable tray; the
  #   3-stage icon chain exercised on all 3; the history opens **below the
  #   minibuffer**, with no second surface.
  # ===================================================================
  m6 = pkgs.linkFarmFromDrvs "check-m6" [
    # Both languages complete, and the proof that the generator refuses the
    # opposite.
    i18n-complete
    i18n-missing-key-fails
    group-mark-outside-translation
    niri-actions-translated
    keyboard-layout-exists
    niri-config-valid
    fontconfig-nested-finds-font
    # A queue one at a time, a critical one that does not disappear, the history
    # below the minibuffer, and ONE surface.
    notifyVm
    # The 3-stage icon chain and the filterable tray are unit tests of
    # `ronoctua-notify`, which run here.
    rust-tests
  ];

  # Milestone 7 WAS ATTACKED — and this check stayed a `mkStub` by carelessness.
  #
  # A `mkStub` prints "NOT ATTACKED YET" and does `touch $out`: it ALWAYS
  # passes. The four real checks already existed and were loose, so
  # `nix build .#checks.x86_64-linux.m7` said OK without verifying anything.
  #
  # The lesson is not "I forgot to swap it": it is that a stub that PASSES is
  # indistinguishable from a closed milestone to whoever reads the output. See
  # the note about milestones in docs/ARCHITECTURE.md.
  # ===================================================================
  # MILESTONE 7 — Native buffers: protocol, runtime, glyph browser, terminal. Impermanence switchable.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   The first native buffer end to end with an `xdg_toplevel` of its own;
  #   instantiation < 5 ms measured with `memory_init_cow` on; a glyph browser
  #   with search, filters and assignment to an app id with hot reload; a
  #   differential test libghostty-vt × alacritty with zero grid diff on the
  #   corpus; a switchable root wipe.
  # ===================================================================
  m7 = pkgs.linkFarmFromDrvs "check-m7" [
    # A native buffer end to end, with an `xdg_toplevel` of its own.
    bufferVm
    clipboardVm
    # Instantiation < 5 ms with `memory_init_cow` on.
    instance-fast
    # Zero grid diff on the VT corpus.
    term-differential
    # The terminal wired to the shell: its own window, and what is typed in it
    # shows up on the screen. `term-differential` proves the ENGINE; this one
    # proves that there is a path from the keyboard to the pixel.
    terminalVm
    # The leader as the Super key on its own (7.4): keyd standing, the right
    # remapping, and niri listening for the key that comes out of it.
    keyboardVm
    # Switchable impermanence, with the recovery window.
    impermanenceVm
    # Search, filters and assignment to an app id — IN THE COMPONENT, driven by
    # `evento` and checked by `render`. This used to point at `rust-tests`, which
    # exercises the database's GENERATOR: the database could be perfect and the
    # browser find nothing, which is what was actually happening.
    browser-filters
    symbols-filters
    unicode-table-generated
    unicode-version-mismatch-fails
    unicode-instance-fast
    lucide-filters
    lucide-table-generated
    lucide-range-taken-fails
    lucide-instance-fast
    renderer-loads-three-faces
    # THE APP FINDER, and what only the VM proves: the sweep at startup, the name
    # in the pixel, and the footer still responding with the application open —
    # the `--scope` is synchronous, and waiting for it froze everything.
    appsVm
    # And the PIXMAP step reaching the real superpack. Without it the list is
    # left with only the category glyphs, which look deliberate.
    apps-icon-from-superpack
    rust-tests
  ];

  # The installer's questions, and the plan they generate.
  #
  # It runs the binary as a DRY RUN: it prints the `host.nix` and the steps
  # without touching a disk. It is the same path `nix run .#install` takes — a
  # check that exercised another path would not prove the installer.

  # ===================================================================
  # MILESTONE 8 — Installer and secure boot end to end.
  #
  # ACCEPTANCE CRITERION, copied from the plan. It is against THIS that the
  # check below has to be written — never against what has already been built:
  #
  #   The TUI collects **only** disk, hostname, password, passphrase, language
  #   and `resume_offset`; everything else detected by `nixos-facter` +
  #   `nixos-hardware`; Secure Boot with lanzaboote and a signed UKI; TPM2
  #   unlocks with PCR 7/11/12/15; **a forged LUKS volume does not unlock**
  #   (the PCR 15 proof).
  #
  # THE NETWORK WENT ONTO THE LIST, and the criterion changed with it —
  # deliberately, and with the reason written in `model.rs` and in section 7.1
  # of the ARCHITECTURE. In short: of the things the criterion said NOT to ask
  # about, the network is the only one that (a) `nixos-facter` has no way of
  # detecting, because a Wi-Fi password is a per-person secret, and (b)
  # without which the `nixos-install` step does not run on a machine with no
  # Ethernet port. The original criterion still stands: one asks what only the
  # person knows, and nothing beyond that.
  # ===================================================================
  m8 = pkgs.linkFarmFromDrvs "check-m8" [
    # The questions, the plan with the four PCRs, and the dry run by default.
    installer-dryrun
    # THE NETWORK AND SYNCTHING of the installed system. They stay here because
    # it is at m8 that the system starts really existing: before that there is no
    # machine on which "is powersave off?" means anything.
    network-tuned
    syncthing-tuned
    # The Wi-Fi password out of the file, sealed in the TPM.
    wifiTpmVm
    # The TUI. The milestone's criterion speaks of it — "the TUI collects ONLY
    # disk, hostname, password, passphrase, language and resume_offset" — and its
    # `Estado` is a function from (state, key) to state, with no terminal at all.
    # The tests walk through the questions, check that the `resume_offset` is NOT
    # shown, that an invalid answer does not advance, and that the confirmation
    # requires the disk's path typed out in full.
    rust-tests
    # TPM2 seals and unlocks with no passphrase; a volume with no token is refused.
    tpmVm
    # The whole chain: a UKI signed by lanzaboote, Secure Boot on, the firmware
    # measuring, and PCR 15 tying the state to the opened volume.
    secureBootVm
    # The ISO all of that is installed from.
    self.packages.${system}.iso
  ];

  # =================================================================
  # Granular checks — exposed so they can be run in isolation during the work.
  # =================================================================
  inherit
    lean-tree
    gitignore-keeps-fonts
    fonts-single-family
    fonts-metrics
    fonts-nerdfonts-version
    fonts-input-publishable
    fonts-license
    icons-table-generated
    icons-unknown-token-fails
    icons-superpack
    apps-icon-from-superpack
    fonts-renderer-parity
    rust-tests
    rust-clippy
    rust-fmt
    shell-binary
    iso-fits-usb-stick
    docs-exist
    ;

  # Nix formatting.
  #
  # The source is FILTERED to contain only the .nix files. Without the filter,
  # `${../.}` would copy the 4.1 GB of assets/fonts into the store on every
  # evaluation of this check — 4.1 GB to run a formatter.
  fmt =
    let
      nixOnly = lib.cleanSourceWith {
        src = ../.;
        name = "ronoctua-nix-sources";
        filter =
          path: type:
          let
            base = baseNameOf path;
          in
          (
            type == "directory"
            && !lib.elem base [
              "assets"
              ".git"
              ".TEMP"
              "target"
              ".direnv"
            ]
          )
          || (type == "regular" && lib.hasSuffix ".nix" base);
      };
    in
    pkgs.runCommand "check-fmt" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
      cd ${nixOnly}
      if ! nixfmt --check $(find . -name '*.nix'); then
        echo "ERRO: rode 'nix fmt' antes de commitar" >&2
        exit 1
      fi
      touch $out
    '';
}
