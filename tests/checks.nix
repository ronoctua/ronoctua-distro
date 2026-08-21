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
#
# And the third, which cost the same lesson a second time: a check on the
# MECHANISM is not a check on the WIRING. `reload-reloads` proved the watcher
# fires, with a command of its own making, while the loop handed it the name of
# a function that had never existed — so hot reload did not fire once, for as
# long as the repository has existed. See `reload-command-is-defined`.
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
  gateApp,
  prefetchApp,
  singleFooter,
  wit,
  tokensRs,
  mimeDb,
  niriActions,
  # The C border in front of libtorrent, which carries the `sizeof` table the
  # layout check compares the Rust against.
  torrentShim,
  # The browser extension, already generated: the fingerprint check reads its
  # manifest, and the literal check reads the sources it was built from.
  extension,
  # The PINNED toolchain, for the buffers' formatting: `rustfmt` from nixpkgs
  # could be another version, and two versions of a formatter are two opinions
  # about the same file.
  rustToolchain,
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
    ronoctua-shell = rust.shell;
  };

  # The machine does not freeze when memory runs out, and the desktop is not the
  # victim. Two nodes with different RAM, because a percentage that is merely
  # present looks exactly like one that does not adapt.
  oomVm = import ./oom-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf;
      inherit (inputs) niri;
      shell = rust.shell;
      iconSuperpack = self.packages.${system}.noctua-icon-theme;
    };
  };

  # The modeline reflects the focused buffer, proved ON SCREEN (milestone 4).
  modelineVm = import ./modeline-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
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
      if ! niri validate --config "$cfg" 2>errors.txt; then
        echo "ERRO: o niri RECUSOU o config gerado." >&2
        echo "      Um config recusado nao e um atalho quebrado: e o" >&2
        echo "      compositor caindo no padrao dele, sem nada nosso." >&2
        cat errors.txt >&2
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

  # THE LOCALE AND THE TIMEZONE EXIST, and the check is where that gets asked.
  #
  # `ronoctua.locale` is a free string on purpose: the real list is glibc's
  # `SUPPORTED`, and reading it during evaluation would be
  # import-from-derivation — the flake would stop and wait for a build. Here,
  # inside a build, the file is free to read.
  #
  # What goes wrong without this is quiet in the way that costs most: a locale
  # glibc does not know produces no error at build time, and the installed system
  # falls back to `C` — dates in American order, no accents in `sort`, and
  # nothing anywhere saying why.
  # THE CONSOLE QUESTION EXISTS, AND ASKS ONCE.
  #
  # Three things that are cheap to get wrong and expensive to notice:
  #
  #   - it must be wanted by `multi-user.target`, because it exists for the boot
  #     that never reaches a graphical one. Under `graphical.target` it would run
  #     exactly when it is not needed and never when it is;
  #   - it must be `oneshot` with no `Restart`, or a machine out of range gets a
  #     password prompt on its console every minute, forever;
  #   - the binary must be a store path. A unit's PATH is minimal, and the
  #     failure of a bare name shows up as a service that exits with nothing to
  #     say about Wi-Fi.
  wifi-console-asks-once = mkCheck "wifi-console-asks-once" [ ] ''
    unit=${self.nixosConfigurations.vm.config.systemd.services.ronoctua-network-console.serviceConfig.Type}
    test "$unit" = "oneshot" || {
      echo "ERRO: o servico do console e \`$unit\`, e nao oneshot." >&2
      echo "      Com Restart ele perguntaria a senha em laco." >&2
      exit 1; }

    exec_start=${self.nixosConfigurations.vm.config.systemd.services.ronoctua-network-console.serviceConfig.ExecStart}
    case "$exec_start" in
      /nix/store/*) ;;
      *) echo "ERRO: ExecStart nao e caminho do store: $exec_start" >&2; exit 1 ;;
    esac

    wanted="${lib.concatStringsSep " " self.nixosConfigurations.vm.config.systemd.services.ronoctua-network-console.wantedBy}"
    case " $wanted " in
      *" multi-user.target "*) ;;
      *) echo "ERRO: o servico nao e puxado por multi-user.target: $wanted" >&2
         echo "      Ele existe para o boot que NAO chega ao grafico." >&2
         exit 1 ;;
    esac

    echo "OK: a pergunta de console e oneshot, por caminho, no multi-user"
  '';

  locale-exists = mkCheck "locale-exists" [ ] ''
    supported=${pkgs.glibcLocales}/share/i18n/SUPPORTED
    tab=${pkgs.tzdata}/share/zoneinfo/zone1970.tab
    locale=${self.nixosConfigurations.vm.config.i18n.defaultLocale}
    zone=${self.nixosConfigurations.vm.config.time.timeZone}

    grep -qE "^$locale/UTF-8" "$supported" || {
      echo "ERRO: a locale \`$locale\` nao esta no SUPPORTED do glibc." >&2
      echo "      O sistema cairia no \`C\`: data em ordem americana, sem" >&2
      echo "      acento na ordenacao, e nada dizendo por que." >&2
      exit 1; }

    # The measurement key is the one that must NOT follow the language: it is
    # what keeps the machine metric when someone installs in American English.
    measurement=${self.nixosConfigurations.vm.config.i18n.extraLocaleSettings.LC_MEASUREMENT or ""}
    case "$measurement" in
      en_US*|en_PH*|""|C|C.UTF-8|POSIX)
        echo "ERRO: LC_MEASUREMENT = \`$measurement\` nao e metrico." >&2
        echo "      Escolher ingles americano escolhe um idioma, nunca polegada." >&2
        exit 1 ;;
    esac

    awk -F'\t' '!/^#/ { print $3 }' "$tab" | grep -qx "$zone" || {
      echo "ERRO: o fuso \`$zone\` nao existe no tzdb." >&2
      exit 1; }

    echo "OK: locale \`$locale\`, medida metrica, fuso \`$zone\`"
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

    # ONE BY ONE, and there is more than one now.
    #
    # `options` is a comma-separated LIST, and this used to grep the whole
    # string as if it were a single name — which worked only for as long as
    # there was exactly one option. The day a second was added, the check would
    # have failed on a correct config, which is the other half of the same
    # defect: a check that cannot see is as bad as no check.
    for o in $(echo "$optionName" | tr ',' ' '); do
      grep -qE "^ +$o +" "$lst" || {
        echo "ERRO: a opcao \`$o\` nao existe no xkeyboard-config." >&2
        echo "      O xkb NAO recusa: ele IGNORA a opcao e segue em frente," >&2
        echo "      entao a acentuacao simplesmente nao acontece, sem erro." >&2
        exit 1; }
    done

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

  # AND THE OPTIONS DO WHAT THEIR NAMES PROMISE — the keymap, compiled.
  #
  # # Why existing is not enough
  #
  # The check above proves the names are in the database. It does not prove they
  # produce anything: an option xkb does not recognise is **ignored, and the
  # exit status is zero** — measured, with `compose:naoexiste`, which compiles
  # fine and merely writes "Unrecognized RMLVO option ... was ignored" to
  # stderr. The symptom is the accent key that types nothing, on a keyboard that
  # is otherwise perfect.
  #
  # So this compiles the real keymap out of the real database and reads the
  # symbols back:
  #
  #   - `<CAPS>` has to carry `Multi_key`, which IS Compose. Without it, the
  #     British layout reaches dead `^` and `` ` `` and nothing else — no `á`,
  #     no `ã`, no `ç`, which is half of Portuguese;
  #   - both Shifts have to carry `Caps_Lock` on their second level. That is
  #     where Caps Lock went when Compose took its place, and it is the only way
  #     back to it.
  #
  # `xkbcli` and not `setxkbmap`: the second one needs an X server, and this
  # is a check.
  keyboard-options-do-what-they-say =
    mkCheck "keyboard-options-do-what-they-say"
      [
        pkgs.libxkbcommon
        pkgs.gnugrep
      ]
      ''
        export XKB_CONFIG_ROOT=${pkgs.xkeyboard_config}/share/X11/xkb

        xkbcli compile-keymap \
          --layout ${keyboard.layout} \
          ${lib.optionalString (keyboard.variant != "") "--variant ${keyboard.variant} "}\
          --options "${keyboard.options}" > keymap.xkb 2> errors.txt || {
            echo "ERRO: o keymap nem compilou:" >&2; cat errors.txt >&2; exit 1; }

        # The ignored-option message, which is the silent failure by name.
        if grep -q "was ignored" errors.txt; then
          echo "ERRO: o xkb IGNOROU uma opcao e seguiu em frente:" >&2
          cat errors.txt >&2
          exit 1
        fi

        # Compose, wherever the answer put it — the symbol is what matters, and
        # not which key carries it.
        grep -q "Multi_key" keymap.xkb || {
          echo "ERRO: nenhuma tecla virou Compose (\`Multi_key\`) no keymap." >&2
          echo "      Sem ela nao existe \`a\`, \`a~\` nem \`c,\` neste teclado." >&2
          exit 1; }

        # And Caps Lock, which left the Caps Lock key and has to be somewhere.
        for side in LFSH RTSH; do
          grep -A 3 "key <$side>" keymap.xkb | grep -q "Caps_Lock" || {
            echo "ERRO: o Shift <$side> nao alterna o Caps Lock." >&2
            echo "      A tecla Caps virou Compose; sem os dois Shifts, o Caps" >&2
            echo "      Lock deixou de existir e ninguem foi avisado." >&2
            grep -A 3 "key <$side>" keymap.xkb >&2
            exit 1; }
        done

        echo "OK: Compose existe no keymap e os dois Shifts alternam o Caps Lock"
      '';

  # THE INSTALLER'S DEFAULTS ARE THE SYSTEM'S DEFAULTS.
  #
  # `ronoctua-installer` carries `DEFAULT_LAYOUT` and `DEFAULT_COMPOSE` as Rust
  # constants — it has to, because it runs from a TTY on the ISO with no flake
  # to evaluate. That makes them a SECOND copy of `nix/keyboard-xkb.nix`, and
  # the two agreeing has been a matter of whoever edited remembering the other.
  #
  # What divergence looks like: a machine installed by pressing Enter through
  # the questions and a machine built by `nix build .#noctua` type differently.
  # Nothing fails; the person just finds out the day they need an accent.
  installer-defaults-match-the-keyboard =
    mkCheck "installer-defaults-match-the-keyboard" [ pkgs.gnugrep ]
      ''
        src=${../crates/ronoctua-installer/src}/model.rs
        failed=0

        agree() {
          wanted="$2"
          found=$(grep -oP "pub const $1: &str = \"\K[^\"]*" "$src" | head -1)
          [ "$found" = "$wanted" ] || {
            echo "ERRO: $1 e \`$found\` no instalador e \`$wanted\` no Nix." >&2
            failed=1
          }
        }
        agree DEFAULT_LAYOUT "${keyboard.layout}"
        agree DEFAULT_COMPOSE "${keyboard.compose}"

        [ "$failed" = 0 ] || exit 1
        echo "OK: o instalador e o nix/keyboard-xkb.nix concordam"
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
    for language in ("pt-BR", "en-GB"):
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
    echo 'so-em-ingles = orphan' >> bad/en-GB.ftl

    if python3 ${../nix/scripts/gen-i18n.py} bad $TMPDIR/keys.rs 2>errors.txt; then
      echo "ERRO: o gerador ACEITOU uma chave que existe em so um idioma" >&2
      exit 1
    fi
    grep -q 'so-em-ingles' errors.txt || {
      echo "ERRO: falhou, mas a mensagem nao diz qual chave" >&2
      cat errors.txt >&2
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

    if python3 ${../nix/scripts/scan-secrets.py} tree_out 2>errors.txt; then
      echo "ERRO: a varredura NAO acusou os segredos plantados." >&2
      exit 1
    fi

    # Every form has to show up: one regex can break without the others.
    for forma in "chave privada" "hash de senha" "chave age" \
                 "token de servico" "PSK de wifi" "e-mail"; do
      grep -q "$forma" errors.txt || {
        echo "ERRO: a varredura nao acusou: $forma" >&2
        cat errors.txt >&2
        exit 1
      }
    done

    # And the secret must NOT show up in the output: the CI log would publish it.
    if grep -qE 'A1b2C3d4|QQPQZRHW4|SenhaDoWifi' errors.txt; then
      echo "ERRO: a varredura imprimiu o proprio segredo." >&2
      exit 1
    fi

    echo "OK: as seis formas derrubam o check, e nenhuma vaza na mensagem"
  '';

  # Notifications, history and the proof that there is no second surface.
  notifyVm = import ./notify-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
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

  # THE PANEL ANSWERS THE HOST — driven on the real component.
  #
  # The three sibling drivers exercise buffers that carry their own database.
  # This one is the first whose screen is EMPTY until the host speaks: the
  # survey crosses the WIT as a record with lists inside, lowered field by
  # field, and `wasmtime` compares the field NAMES only after zipping them
  # positionally. A field out of order fails here, with a message, instead of
  # on somebody's screen.
  #
  # It also holds the two rules that make an irreversible key safe: one press
  # arms, another key disarms, the same key fires — and closing points at the
  # ROW of the last survey, never at a pid, because a scope has none.
  memory-driven = mkCheck "memory-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-memory \
      ${buffers.memory}/lib/ronoctua_buffer_memory.wasm \
      ${../assets/i18n}
  '';

  # AND ON THE FOURTH, which is the lightest of them: it embeds no database at
  # all, because every number it draws is pushed in at run time. The ceiling
  # here is loose on purpose — what it guards is a regression in the RUNTIME,
  # which would show up first on the component that has the least to load.
  memory-instance-fast = mkCheck "memory-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.memory}/lib/ronoctua_buffer_memory.wasm 200
  '';

  # AND ON THE FIFTH — the queue in a window, which embeds nothing either.
  #
  # What only this driver can prove: the cursor is an ID, and the list REORDERS
  # underneath it. The queue sorts what is afoot above what has finished, so a
  # job ending moves every row below it — and the key in question deletes the
  # piece already written at the destination. The reorder is simulated between
  # the frame and the key, which is the one place both sides of the WIT are
  # visible at once.
  transfers-driven = mkCheck "transfers-driven" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-transfers \
      ${buffers.transfers}/lib/ronoctua_buffer_transfers.wasm \
      ${../assets/i18n}
  '';

  transfers-instance-fast = mkCheck "transfers-instance-fast" [ ] ''
    ${rust.workspace}/bin/ronoctua-bench-instance \
      ${buffers.transfers}/lib/ronoctua_buffer_transfers.wasm 200
  '';

  # ===================================================================
  # THE TORRENT BORDER, and the two things about it that cannot be read.
  # ===================================================================

  # THE LAYOUT, COMPARED AGAINST THE C++ COMPILER'S OWN MEASUREMENT.
  #
  # `#[repr(C)]` promises to ARRANGE what it was told and never to check it: a
  # wrong width is a register with the wrong contents and produces no
  # diagnostic at all. `CLAUDE.md` records that this cost three wrong struct
  # layouts on `libghostty-vt`, found by hand with a twenty-line C program.
  #
  # `nix/torrent-shim.nix` now compiles the REAL header with the real C++
  # compiler and writes every `sizeof` and `offsetof` down; this compares that
  # file against the constants in the Rust. Neither side can drift alone.
  torrent-layout-matches-cxx = mkCheck "torrent-layout-matches-cxx" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import re, sys, pathlib

    measured = {}
    for row in pathlib.Path("${torrentShim}/share/ronoctua/layout.txt").read_text().split("\n"):
        if not row.strip():
            continue
        name, value = row.split()
        measured[name] = int(value)

    rust = pathlib.Path("${../crates/ronoctua-torrent/src/ffi.rs}").read_text()
    declared = {}
    m = re.search(r"assert_eq!\(size_of::<Status>\(\), (\d+)", rust)
    if m:
        declared["size"] = int(m.group(1))
    m = re.search(r"assert_eq!\(align_of::<Status>\(\), (\d+)", rust)
    if m:
        declared["align"] = int(m.group(1))
    for field, value in re.findall(r"offset_of!\(Status, (\w+)\), (\d+)", rust):
        declared[field] = int(value)

    if not declared:
        sys.exit("ERRO: nao achei numero nenhum em ffi.rs — o teste de layout sumiu")

    failed = 0
    for name, value in sorted(measured.items()):
        if name not in declared:
            print(f"ERRO: o C++ mede `{name}` = {value} e o Rust nao o declara", file=sys.stderr)
            failed = 1
        elif declared[name] != value:
            print(
                f"ERRO: `{name}`: o C++ mede {value}, o Rust diz {declared[name]}."
                " Um FFI com layout errado nao da diagnostico — da registrador"
                " com conteudo errado.",
                file=sys.stderr,
            )
            failed = 1
    for name in declared:
        if name not in measured:
            print(f"ERRO: o Rust declara `{name}` e o C++ nao o mede", file=sys.stderr)
            failed = 1
    if failed:
        sys.exit(1)
    print(f"OK: {len(measured)} medidas do compilador C++ batem com as do Rust")
    PY
  '';

  # AND THAT v2 IS NOT A CLAIM.
  #
  # It makes a purely-v2 torrent, hands it to a real session, and demands a v2
  # info hash and NO v1 one — then does the same with a v1-only torrent, so a
  # flag that were simply always true fails here too. Entirely offline: the data
  # is already where the torrent says it is, so the session checks it against
  # the Merkle tree of SHA-256 hashes and goes straight to seeding.
  torrent-speaks-v2 = mkCheck "torrent-speaks-v2" [ ] ''
    ${rust.workspace}/bin/ronoctua-test-torrent "$TMPDIR/torrent"
  '';

  # ===================================================================
  # THE EXTENSION, and the fingerprint the owner asked about.
  # ===================================================================

  # THE FIVE WAYS A WEBSITE CAN SEE AN EXTENSION, refused in the manifest.
  #
  # The owner's question was exact: an extension nobody else has makes this
  # browser unique if every site can list it. The detection vectors are known
  # and every one of them is a manifest key — so the manifest is GENERATED with
  # none of them, and this fails the build the day one appears.
  extension-has-no-fingerprint = mkCheck "extension-has-no-fingerprint" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import json, pathlib, sys

    m = json.loads(pathlib.Path("${extension}/share/ronoctua-extension/manifest.json").read_text())
    forbidden = [
        ("web_accessible_resources", "qualquer pagina consegue `fetch` de chrome-extension://"),
        ("content_scripts", "o que ele deixa no DOM e um marcador"),
        ("externally_connectable", "entrega `chrome.runtime` a uma origem"),
        ("host_permissions", "acesso permanente e o que torna o resto possivel"),
        ("declarative_net_request", "a pagina infere pela propria requisicao"),
    ]
    failed = 0
    for key, why in forbidden:
        if key in m:
            print(f"ERRO: o manifesto ganhou `{key}` — {why}", file=sys.stderr)
            failed = 1
    # And the permissions: `activeTab` instead of standing access IS the whole
    # design. Without it the only alternative is `host_permissions`.
    if "activeTab" not in m.get("permissions", []):
        print("ERRO: sem `activeTab` a extensao precisaria de acesso permanente", file=sys.stderr)
        failed = 1
    if m.get("manifest_version") != 3:
        print("ERRO: MV2 nao tem `activeTab` com este significado", file=sys.stderr)
        failed = 1
    if failed:
        sys.exit(1)
    print(f"OK: nenhuma das cinco vias de deteccao, e {len(m['permissions'])} permissoes")
    PY
  '';

  # AND IT PARSES.
  #
  # A syntax error in a service worker is the quietest failure this extension
  # can have: Chrome loads the extension, the worker never starts, the button
  # does nothing, and the message is behind two clicks in a page nobody has
  # open. Eleven megabytes of esbuild at build time buys that away — and it
  # checks the GENERATED tree, so a broken `generated.js` fails here too.
  extension-parses = mkCheck "extension-parses" [ pkgs.esbuild ] ''
    failed=0
    for f in ${extension}/share/ronoctua-extension/*.js; do
      if ! esbuild --log-level=warning --format=esm --target=chrome116 "$f" > /dev/null; then
        echo "ERRO: $(basename "$f") nao compila — o service worker nunca subiria" >&2
        failed=1
      fi
    done
    [ "$failed" = 0 ] || exit 1
    echo "OK: os quatro arquivos da extensao compilam para chrome116"
  '';

  # AND NO TEXT AND NO COLOUR WRITTEN BY HAND.
  #
  # Invariants 8 and 9 do not stop at the browser's edge. Every word comes from
  # `assets/i18n/*.ftl` and every colour from `assets/tokens/colors.nix`, both
  # through the generated table — so a hex code or a sentence in the extension's
  # own sources is a value outside the design system.
  extension-has-no-literal = mkCheck "extension-has-no-literal" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    source = pathlib.Path("${../extension}")
    failed = 0
    for entry in sorted(source.iterdir()):
        if entry.name == "generated.js":
            continue
        text = entry.read_text()
        # A colour by hand, in any notation a browser would accept.
        for m in re.finditer(r"#[0-9a-fA-F]{3,8}\b|\brgba?\(\s*\d+", text):
            row = text[: m.start()].count("\n") + 1
            print(
                f"ERRO: {entry.name}:{row} tem cor a mao (`{m.group(0)}`)."
                " Toda cor vem de assets/tokens/colors.nix (invariante 9).",
                file=sys.stderr,
            )
            failed = 1
        # UI text. What a person reads arrives through `TEXT.*`; what is left
        # over are log messages, which nobody reads off a screen.
        for m in re.finditer(r"textContent\s*=\s*['\"`]([^'\"`]+)", text):
            if m.group(1).strip():
                row = text[: m.start()].count("\n") + 1
                print(
                    f"ERRO: {entry.name}:{row} escreve text direto na tela."
                    " Todo text vem de assets/i18n (invariante 8).",
                    file=sys.stderr,
                )
                failed = 1
    if failed:
        sys.exit(1)
    print("OK: a extensao nao carrega palavra nem cor propria")
    PY
  '';

  # ===================================================================
  # AND WHAT IS BORN ON THE MACHINE — invariant 13.
  # ===================================================================
  #
  # Two things this work needs that may never be in a public repository: the
  # extension's signing key, which decides its id, and aria2's RPC token. Both
  # are generated by units declared in the flake, on the machine, and
  # `nix/modules/secrets.nix` is explicit that "but it is encrypted" is not an
  # argument for either.
  secrets-are-not-in-the-repo = mkCheck "secrets-are-not-in-the-repo" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    root = pathlib.Path("${../.}")
    failed = 0
    # The extension's private key, in any PEM shape.
    private = re.compile(r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----")
    # And aria2's token WITH A VALUE: the line that generates it lives in the
    # module and ends in `$secret`, which is not a secret, and the line that
    # reads it lives in the Rust.
    token = re.compile(r"rpc-secret\s*=\s*[A-Za-z0-9+/=]{8,}")
    for entry in root.rglob("*"):
        if not entry.is_file():
            continue
        if any(part in {".git", "target", "result"} for part in entry.parts):
            continue
        try:
            text = entry.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        if private.search(text):
            print(f"ERRO: chave privada em {entry.relative_to(root)}", file=sys.stderr)
            failed = 1
        if token.search(text):
            print(f"ERRO: segredo do aria2 em {entry.relative_to(root)}", file=sys.stderr)
            failed = 1
    if failed:
        sys.exit(1)
    print("OK: nem chave privada nem token — os dois nascem NA maquina")
    PY
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
    timeout 300 ronoctua-dev > out_text.txt 2> errors.txt || true

    if grep -qE "cannot remove|Permission denied" errors.txt; then
      echo "ERRO: encerrar a sessao imprimiu erro de remocao:" >&2
      grep -E "cannot remove|Permission denied" errors.txt >&2
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
    n=$( { cat out_text.txt errors.txt | grep -c "logs desta sessao:"; } || true )
    [ "$n" = 1 ] || {
      echo "ERRO: o encerrar rodou $n vezes; esperava 1." >&2
      echo "      'trap … EXIT INT TERM' limpa pelo INT e de novo pelo EXIT." >&2
      echo "--- saida:" >&2; tail -20 out_text.txt >&2
      echo "--- erro:" >&2; tail -20 errors.txt >&2
      exit 1
    }

    echo "OK: encerrar a sessao sai limpo, e uma vez so"
  '';

  # THE NESTED LOOP CALLS A COMMAND THAT EXISTS.
  #
  # # The defect this exists for, and it lasted from the first commit
  #
  # `nix/dev.nix` handed the watcher `RONOCTUA_RELOAD_CMD='reconstruir'`, and
  # the function it defines and exports is called `rebuild`. The watcher `eval`s
  # the string, gets "command not found", and swallows it with `|| true` — which
  # is correct, because a compile error is the normal case in a loop. So HOT
  # RELOAD NEVER FIRED, not once, and nothing said so: the footer that stays on
  # screen is the store's, and it works perfectly.
  #
  # `reload-reloads`, just below, did not see it: it proves the MECHANISM with a
  # command of its own making. What was missing was a check on the WIRING.
  #
  # Both sides come from the built script, which is the artifact that runs.
  reload-command-is-defined =
    mkCheck "reload-command-is-defined"
      [
        pkgs.gnugrep
        pkgs.gnused
      ]
      ''
        script=${devApp}/bin/ronoctua-dev

        cmd=$(sed -n "s/.*RONOCTUA_RELOAD_CMD='\([a-z_]*\)'.*/\1/p" "$script" | head -1)
        [ -n "$cmd" ] || {
          echo "ERRO: nao achei o RONOCTUA_RELOAD_CMD no script do laco" >&2
          exit 1
        }
        echo "o laco manda rodar: $cmd"

        grep -q "^  $cmd() {" "$script" || {
          echo "ERRO: '$cmd' nao e uma funcao definida no script — o watcher vai" >&2
          echo "      receber 'command not found', engolir com '|| true', e o hot" >&2
          echo "      reload nao vai disparar NUNCA, sem uma linha de aviso." >&2
          exit 1
        }
        # And exported: the watcher is ANOTHER process, and a function that is
        # not exported does not cross into it — same symptom, same silence.
        grep -qE "^  export -f .*\b$cmd\b" "$script" || {
          echo "ERRO: '$cmd' nao e exportada; o watcher roda noutro processo" >&2
          exit 1
        }

        echo "OK: o laco chama '$cmd', que existe e atravessa para o watcher"
      '';

  # AND IT COMPILES THE SAME BINARY THE BUILD WOULD.
  #
  # # The defect this exists for
  #
  # Everything in `nix/rust.nix`'s `generated` reaches the Rust through
  # `option_env!`, which resolves at COMPILE time. Hot reload compiles the binary
  # HERE, so a variable this script does not export is a variable that binary
  # does not have — and the failure is always the same shape: a feature that
  # works in production and answers "could not open" in the loop.
  #
  # It had already happened twice, and both were found by accident: the icon
  # superpack (the list drew category glyphs and never the app's icon) and the
  # five wasm components. Twenty-five more were waiting, `btop` and the
  # clipboard and `ghostty-vt` among them; what hid them was that the reload was
  # wired to a command that did not exist, so no edit ever replaced the store's
  # binary.
  #
  # The list is not written here: it is read from the table the compilation gets.
  dev-exports-what-the-build-bakes = mkCheck "dev-exports-what-the-build-bakes" [ pkgs.gnugrep ] ''
    script=${devApp}/bin/ronoctua-dev
    failed=0

    for v in ${lib.concatStringsSep " " (lib.attrNames (rust.generated // rust.bufferComponents))}; do
      grep -q "^  export $v=" "$script" || {
        echo "ERRO: o build assa $v e o laco nao a exporta — o binario do hot" >&2
        echo "      reload vai sem ela, e a falha aparece longe daqui." >&2
        failed=1
      }
    done

    [ "$failed" = 0 ] || exit 1
    echo "OK: as $(grep -c '^  export RONOCTUA_' "$script") variaveis do laco cobrem o que o build assa"
  '';

  # ===================================================================
  # THE RESCUE CONSOLE — `Ctrl+Alt+Del`
  # ===================================================================

  # EVERY ROW OF THE CONSOLE HAS A SENTENCE, IN BOTH LANGUAGES.
  #
  # Invariant 8, on the one screen where breaking it would hurt most: a rescue
  # console that draws `[rescue-reboot]` is a console whose rows nobody can
  # read, opened by somebody whose machine is already broken.
  #
  # The ids come out of the Rust and not out of a list written here — a list
  # here would be a third copy, and the one that goes stale is always the copy.
  rescue-actions-have-text =
    mkCheck "rescue-actions-have-text"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        src=${../crates/ronoctua-rescue/src}
        failed=0

        keys=$(grep -ohE '"rescue-[a-z0-9-]+"' "$src"/lib.rs | tr -d '"' | sort -u)
        [ -n "$keys" ] || {
          echo "ERRO: nenhuma chave 'rescue-*' no registro — o grep casa o que?" >&2
          exit 1
        }

        for k in $keys; do
          for language in pt-BR en-GB; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERRO: falta '$k' em $language.ftl — a tela mostraria [$k]" >&2
              failed=1
            }
          done
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$keys" | wc -l) chaves do resgate, nos dois idiomas"
      '';

  # THE MIME TABLE CAME OUT WHOLE.
  #
  # The floors are in the generator and the compile-time assertions are in
  # `mime.rs`; what this adds is that the derivation BUILT, which is the thing
  # a reordered `mime-glyphs.nix` or a bumped `shared-mime-info` would break.
  mime-table-generated = mkCheck "mime-table-generated" [ pkgs.gnugrep pkgs.coreutils ] ''
    table=${mimeDb}/mimedb.rs
    test -s "$table" || { echo "ERRO: a tabela saiu vazia" >&2; exit 1; }
    for name in GLOBS MAGIC SUBCLASS ALIASES GENERIC ICON_OF_GENERIC ICON_OF_MIME; do
      grep -qE "pub static $name" "$table" || {
        echo "ERRO: falta a tabela $name" >&2
        exit 1
      }
    done
    # The counts, so a reader that quietly stopped understanding the format is
    # caught here as well as at compile time. A parser that breaks returns a
    # HANDFUL of rows, which reads as "the database is small".
    globs=$(grep -c 'Glob {' "$table")
    [ "$globs" -ge 900 ] || { echo "ERRO: so $globs globs" >&2; exit 1; }
    echo "OK: $globs padroes de nome, e as sete tabelas"
  '';

  # AND IT REFUSES A TABLE THAT DOES NOT ANSWER FOR EVERY GENERIC ICON.
  #
  # The negative fixture. Without it, a `shared-mime-info` release adding
  # `x-office-drawing` would draw the neutral square on a whole class of files
  # — and the neutral square is indistinguishable from "I did not resolve it",
  # so nothing would say so.
  mime-unknown-icon-fails = mkCheck "mime-unknown-icon-fails" [ ] ''
    test -s ${mimeDb.negativeFixture}/veredito
    cat ${mimeDb.negativeFixture}/veredito
  '';

  # EVERY ROW OF THE FILE MANAGER'S MENU HAS TEXT, IN BOTH LANGUAGES.
  #
  # The same shape as the rescue console's, and for the same reason: a row with
  # no translation draws `[files-open]` on screen, which is exactly the
  # silent-fallback class invariant 8 exists to refuse — `Texts::t` gives back
  # `[key]` rather than bringing the shell down, so somebody has to be looking
  # for it.
  files-actions-have-text =
    mkCheck "files-actions-have-text"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        src=${../crates/ronoctua-files/src}
        failed=0

        # FROM THE ID, and not from the label. The label is a CONSTANT
        # (`key::FILES_OPEN`), so grepping for a `"files-…"` literal here finds
        # only the prefix — which is what the first version of this check did,
        # and it would have gone green on a table with no translations at all.
        #
        # Deriving the key from the id is also the discipline itself: the
        # registry refuses a row whose label is not `files-<id>`, and this is
        # the same rule reaching the `.ftl`.
        ids=$(grep -oE '^ *id: "[a-z0-9-]+"' "$src"/menu.rs | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$ids" ] || {
          echo "ERRO: nenhuma linha na tabela de acoes — o grep casa o que?" >&2
          exit 1
        }
        keys=$(for i in $ids; do echo "files-$i"; done | sort -u)

        for k in $keys; do
          for language in pt-BR en-GB; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERRO: falta $k em $language.ftl — a tela mostraria a chave crua" >&2
              failed=1
            }
          done
        done

        # AND THE VIEW'S OWN KEYS, which live in a second table for the reason
        # `leader::OF_CLIPBOARD_VIEW` gives: a key that only exists inside a
        # `match` arm is a key nothing can enumerate.
        view=$(grep -ohE '"files-[a-z0-9-]+"' ${../crates/ronoctua-shell/src}/files.rs \
          | tr -d '"' | sort -u)
        for k in $view; do
          for language in pt-BR en-GB; do
            grep -qE "^$k = ." ${../assets/i18n}/$language.ftl || {
              echo "ERRO: falta $k em $language.ftl — a tecla nao teria nome" >&2
              failed=1
            }
          done
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$keys" "$view" | tr ' ' '\n' | sort -u | wc -l) chaves, nos dois idiomas"
      '';

  # NOTHING THE MOUSE DOES IS OUT OF THE KEYBOARD'S REACH.
  #
  # The owner's rule, mechanised. Two lists, compared in BOTH directions:
  #
  #   - every row of `menu.rs` has a key, and no two share one;
  #   - every `Hit` the drawing can produce is reachable by a key too.
  #
  # The second half is the one that would rot: adding a clickable field is one
  # line in `row_of`, and nothing about that line says a key has to exist for
  # it. A file manager where "change the owner" is a click and not a key is a
  # file manager somebody has to take their hand off the keyboard for.
  files-everything-has-a-key =
    mkCheck "files-everything-has-a-key"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        menu=${../crates/ronoctua-files/src}/menu.rs
        view=${../crates/ronoctua-shell/src}/files.rs
        failed=0

        # --- every menu row has a key, and the keys do not collide ----------
        keys=$(grep -oE '^ *key: "[^"]+"' "$menu" | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$keys" ] || { echo "ERRO: nenhuma tecla na tabela de acoes" >&2; exit 1; }
        repeated=$(echo "$keys" | sort | uniq -d)
        [ -z "$repeated" ] || {
          echo "ERRO: duas linhas do menu na mesma tecla: $repeated" >&2
          failed=1
        }

        # --- every clickable Hit is reachable by a key ----------------------
        #
        # The variants come from the enum itself, so a new one is caught the day
        # it is added rather than the day somebody notices it has no key.
        variants=$(sed -n '/^pub enum Hit {/,/^}/p' "$view" \
          | grep -oE '^ {4}[A-Z][A-Za-z]*' | tr -d ' ')
        [ -n "$variants" ] || { echo "ERRO: nao achei o enum Hit" >&2; exit 1; }

        for v in $variants; do
          case "$v" in
            # These three are not a promise of an action: `Nothing` is the
            # absence of one, and `Menu` and `Choice` are rows of something
            # already on screen that the arrows and `Enter` already reach.
            Nothing|Menu|Choice) continue ;;
          esac
          grep -qE "Hit::$v" "$view" || {
            echo "ERRO: a variante Hit::$v nao e produzida por nada" >&2
            failed=1
          }
        done

        # --- and the view's key table agrees with its `match` ---------------
        #
        # The unit test beside the table presses every key in it; this half
        # catches the other direction — a key handled and never declared, which
        # the manual would not list.
        declared=$(sed -n '/^pub const OF_THE_VIEW/,/^];/p' "$view" \
          | grep -oE '^ *\("[^"]+"' | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$declared" ] || { echo "ERRO: nao achei OF_THE_VIEW" >&2; exit 1; }

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$keys" | wc -l) teclas de menu, $(echo "$declared" | wc -l) da vista, e todo Hit tem dono"
      '';

  # NOTHING THE FILE MANAGER SPAWNS COMES BY NAME.
  #
  # `bsdtar`, `fstrim` and `pkexec`. Same two sides as the rescue console's:
  # the variables the crate reads, and the table `nix/rust.nix` sets.
  files-runs-by-path = mkCheck "files-runs-by-path" [ pkgs.gnugrep ] ''
    src=${../crates/ronoctua-files/src}
    failed=0

    read_it=$(grep -ohE 'option_env!\("RONOCTUA_[A-Z_]+"\)' "$src"/*.rs "$src"/bin/*.rs \
      | grep -oE 'RONOCTUA_[A-Z_]+' | sort -u)
    [ -n "$read_it" ] || {
      echo "ERRO: o crate nao le nenhuma variavel — o grep casa o que?" >&2
      exit 1
    }

    for v in $read_it; do
      grep -qE "^ *$v = " ${../nix/rust.nix} || {
        echo "ERRO: $v e lida pelo crate e nao esta em nix/rust.nix" >&2
        echo "      O programa falharia longe da causa, com PATH minimo." >&2
        failed=1
      }
    done

    # NO BARE NAME. A `Command::new("bsdtar")` would work in the devshell and
    # fail inside a systemd scope — the trap CLAUDE.md records.
    bare=$(grep -nE 'Command::new\("[a-z]' "$src"/*.rs "$src"/bin/*.rs || true)
    [ -z "$bare" ] || {
      echo "ERRO: programa por NOME, nao por caminho:" >&2
      echo "$bare" >&2
      failed=1
    }

    [ "$failed" = 0 ] || exit 1
    echo "OK: $(echo "$read_it" | wc -l) programas, todos por caminho de store"
  '';

  # THE PRIVILEGED DOOR IS AS NARROW AS THE CODE SAYS IT IS.
  #
  # Two directions, the same shape as `rescue-polkit-is-narrow`:
  #
  #   - the unit the crate names is the one the polkit rule allows;
  #   - the rule allows nothing else, and the verb is `start`.
  #
  # And one more, which is what this door has and the console's does not: the
  # list of VERBS. `Verb::ALL` is four, and none of them destroys anything —
  # a `remove` or a `move` slipped in there is exactly what nobody would notice
  # reviewing a diff.
  files-polkit-is-narrow =
    mkCheck "files-polkit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        crate=${../crates/ronoctua-files/src}/act.rs
        module=${../nix/modules/files.nix}
        failed=0

        unit=$(grep -oE 'pub const UNIT: &str = "[^"]+"' "$crate" | grep -oE '"[^"]+"' | tr -d '"')
        [ -n "$unit" ] || { echo "ERRO: o crate nao nomeia a unidade" >&2; exit 1; }

        grep -qE "unit = \"$unit\"" "$module" || {
          echo "ERRO: o crate pede '$unit' e o modulo nao a define" >&2
          failed=1
        }
        grep -qE 'action.lookup\("unit"\) == "\$\{unit\}"' "$module" || {
          echo "ERRO: a regra polkit nao compara a unidade com a do modulo" >&2
          failed=1
        }
        grep -qE 'action.lookup\("verb"\) == "start"' "$module" || {
          echo "ERRO: a regra polkit nao limita o verbo a 'start'" >&2
          echo "      Sem isso ela para, reinicia e recarrega qualquer coisa." >&2
          failed=1
        }
        grep -qE 'subject.user == "\$\{user\}"' "$module" || {
          echo "ERRO: a regra polkit nao limita o usuario" >&2
          failed=1
        }

        # NO SECOND UNIT. A rule that names one unit and a module that declares
        # two is privilege granted to something nobody audited.
        units=$(grep -cE '^ *systemd.services.ronoctua-files' "$module" || true)
        [ "$units" = 1 ] || {
          echo "ERRO: o modulo declara $units unidades privilegiadas; a regra nomeia uma" >&2
          failed=1
        }

        # AND THE FOUR VERBS, none of which destroys.
        for bad in '"rm"' '"remove"' '"mv"' '"move"' '"delete"' '"unlink"'; do
          grep -qE "Self::[A-Za-z]+ => $bad" "$crate" && {
            echo "ERRO: verbo destrutivo na porta privilegiada: $bad" >&2
            failed=1
          }
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: uma unidade ($unit), verbo start, um usuario, quatro verbos que nao apagam"
      '';

  # THE HOST'S LIBRARIES NEVER GO ON `LD_LIBRARY_PATH` — except for NVIDIA.
  #
  # `LD_LIBRARY_PATH` beats `DT_RUNPATH`, so a directory put there to reach ONE
  # library brings every other library in it. `/usr/lib64` was put there for the
  # host's `libEGL.so.1`, and `libc.so.6` came along — and the process then ran
  # the host's libc against Nix's `ld.so`. glibc's `GLIBC_PRIVATE` symbols only
  # match between a `libc.so.6` and the `ld.so` of the SAME build, so the day the
  # host's glibc moved, `nix run .#dev` died with:
  #
  #   niri: symbol lookup error: /usr/lib64/libc.so.6:
  #         undefined symbol: __pointer_chk_guard, version GLIBC_PRIVATE
  #
  # Measured then: host 2.44, Nix 2.42, the symbol absent from the host's. There
  # is no version to juggle — the fix is not to put the host's libraries there,
  # and for AMD and Intel that costs nothing, because what talks to the card is
  # the KERNEL driver and mesa is a userspace library Nix has a copy of.
  #
  # NVIDIA is the one real exception: its blob has to match its kernel module.
  # That branch is allowed, and it prints what it is doing.
  graphics-does-not-mix-libc =
    mkCheck "graphics-does-not-mix-libc"
      [
        pkgs.gnugrep
        pkgs.gawk
        pkgs.coreutils
      ]
      ''
        script=${devApp}/bin/ronoctua-dev
        failed=0

        # THE ORDINARY PATH EXISTS: a check that only forbids goes green on a
        # script that stopped setting up graphics at all.
        grep -q 'ronoctua-graphics/lib' "$script" || {
          echo "ERRO: o laco nao poe o mesa do Nix no LD_LIBRARY_PATH." >&2
          echo "      Sem ele o niri sobe sem EGL e morre com DisplayNotSupported." >&2
          failed=1
        }

        # AND EVERY MENTION OF THE HOST'S LIBRARIES IS INSIDE THE NVIDIA BRANCH.
        #
        # The block is cut out and what is left must not mention `/usr/lib` at
        # all. Grepping for the branch's presence would not do: the branch could
        # be there and a second export outside it.
        outside=$(awk '
          /if \[ "\$only_nvidia" = sim \]/ { inside = 1 }
          inside && /^      fi$/ { inside = 0; next }
          !inside { print }
        ' "$script" | grep -n '/usr/lib' || true)
        if [ -n "$outside" ]; then
          echo "ERRO: o laco poe biblioteca do HOST no caminho fora do ramo NVIDIA:" >&2
          echo "$outside" >&2
          echo "      Junto vai a libc do host, e ela nao combina com o ld.so do Nix." >&2
          failed=1
        fi

        [ "$failed" = 0 ] || exit 1
        echo "OK: o driver vem do Nix, e o do host so no ramo NVIDIA"
      '';

  # THE GENERATED MIME TABLE IS A BUILD INPUT, AND STAYS ONE.
  #
  # `nix/mimegen.nix` turns freedesktop's 125 KB of data into a Rust table at
  # BUILD time. What must never happen is that derivation becoming a runtime
  # reference — which is what an `option_env!` holding its path, or a crate
  # that decided to read `share/mime` directly, would do. The trap `CLAUDE.md`
  # records: a derivation references what its BYTES cite.
  #
  # WHAT THIS CHECK DOES NOT CLAIM, and the first version of it did.
  #
  # It used to forbid `shared-mime-info` from the closure outright, and that
  # was a check written against what its author hoped instead of against what
  # it can prove — the mistake this file's own header names. The package is in
  # the shell's closure and was there before the file manager existed:
  # `RONOCTUA_XDG_OPEN` points at `xdg-utils`, and `xdg-open` needs the
  # database to know what opens what. Measured with `nix why-depends`, not
  # assumed:
  #
  #   ronoctua-shell → ronoctua-0.1.0 → xdg-utils → shared-mime-info
  #
  # That is the TOOL arriving, and this check is about the TABLE. The two are
  # different things that share a name.
  files-closure-has-no-mime-db =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.shell ]; };
    in
    mkCheck "files-closure-has-no-mime-db" [ pkgs.gnugrep pkgs.coreutils ] ''
      if found=$(grep -e '-ronoctua-mimedb-' ${closure}/store-paths); then
        echo "ERRO: a tabela gerada esta no closure de EXECUCAO:" >&2
        echo "$found" >&2
        echo "      Ela e lida em tempo de COMPILACAO — o build.rs a copia para" >&2
        echo "      OUT_DIR e o binario carrega os DADOS. Uma referencia aqui" >&2
        echo "      significa que alguem passou a le-la em execucao, e com ela" >&2
        echo "      vem o shared-mime-info inteiro." >&2
        exit 1
      fi

      # AND THE MANAGER ESTA LA. A check that only forbids goes green on a
      # shell that lost the feature altogether — the lesson `iso-has-no-toolchain`
      # already learned. `bsdtar` is the file manager's one new runtime
      # reference, so its absence means the compressing half is gone.
      grep -q -e '-libarchive-' ${closure}/store-paths || {
        echo "ERRO: o libarchive nao esta no closure; o `bsdtar` do gerenciador" >&2
        echo "      sumiu, e comprimir e extrair falhariam por caminho ausente." >&2
        exit 1
      }
      echo "OK: $(wc -l < ${closure}/store-paths) caminhos; a tabela nao esta em execucao e o bsdtar esta"
    '';

  # THE COLUMNS GIVE WAY IN THE ORDER THE CODE STATES.
  #
  # An order that drifts with the last edit turns the narrow case into whatever
  # happened to survive. The unit test beside `GIVE_WAY` sweeps every width from
  # one cell to a hundred and twenty and asserts both halves — the order, and
  # that nothing leaks past the edge. This check exists so the constant cannot
  # be reordered without the test being re-read: it pins the FIRST to give way
  # and the one that never does.
  files-columns-degrade = mkCheck "files-columns-degrade" [ pkgs.gnugrep ] ''
    view=${../crates/ronoctua-shell/src}/files.rs
    order=$(sed -n '/^pub const GIVE_WAY/,/^];/p' "$view" | grep -oE 'Column::[A-Za-z]+' | tr -d ' ')
    first=$(echo "$order" | head -1)
    [ "$first" = "Column::Group" ] || {
      echo "ERRO: a primeira coluna a ceder virou $first, e era o grupo." >&2
      echo "      Se a mudanca e proposital, o teste ao lado de GIVE_WAY diz por que." >&2
      exit 1
    }
    echo "$order" | grep -q 'Column::Name' && {
      echo "ERRO: o nome esta em GIVE_WAY. Ele nunca cede: uma tabela sobre" >&2
      echo "      arquivos que nao diz quais e uma tabela sobre nada." >&2
      exit 1
    }
    echo "OK: cede na ordem $(echo "$order" | tr '\n' ' ')"
  '';

  # NOTHING THE CONSOLE SPAWNS COMES BY NAME.
  #
  # It runs from a unit on a VT, where `PATH` is minimal, and it is opened
  # BECAUSE something is already broken — a name that does not resolve fails on
  # the one screen that exists to explain failures.
  #
  # Two sides, both from real artifacts: the variables the crate reads, and the
  # table `nix/rust.nix` sets. It is `shell-finds-its-buffers` again, on the
  # other application.
  rescue-runs-by-path = mkCheck "rescue-runs-by-path" [ pkgs.gnugrep ] ''
    src=${../crates/ronoctua-rescue/src}
    failed=0

    read_it=$(grep -ohE 'option_env!\("RONOCTUA_[A-Z_]+"\)' "$src"/*.rs \
      | grep -oE 'RONOCTUA_[A-Z_]+' | sort -u)
    [ -n "$read_it" ] || {
      echo "ERRO: o console nao le variavel nenhuma — ele spawna por nome?" >&2
      exit 1
    }
    echo "o console le: $(echo "$read_it" | tr '\n' ' ')"

    baked="${lib.concatStringsSep " " (lib.attrNames rust.generated)}"
    for v in $read_it; do
      case " $baked " in
        *" $v "*) ;;
        *)
          echo "ERRO: o console le $v e o build nao a assa — ele cairia" >&2
          echo "      no 'PATH' minimo de uma unidade e falharia longe daqui." >&2
          failed=1
          ;;
      esac
    done

    [ "$failed" = 0 ] || exit 1
    echo "OK: todo programa do resgate vem por caminho de store"
  '';

  # THE POLKIT RULE ALLOWS EXACTLY WHAT THE CONSOLE ASKS FOR.
  #
  # Two directions, and the second is the one that rots: a row without its unit
  # fails in front of the person, which is loud; a unit left in the rule after
  # its row went away is privilege granted to nobody, which is silent and stays.
  #
  # The exemption list is spelled out with its reason. `keyd.service` is reached
  # by a FREE row and is not this rule's business — `nix/modules/keyboard.nix`
  # already allows exactly it, and `Ctrl+Shift+L` already fires it.
  rescue-polkit-is-narrow =
    let
      rule = self.nixosConfigurations.noctua.config.security.polkit.extraConfig;
    in
    mkCheck "rescue-polkit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        cat > rule.js <<'FIM'
        ${rule}
        FIM
        # THE WHOLE CRATE, and not just the registry. The unit that installs an
        # edited `host.nix` is named in `edit.rs` — this check found that by
        # refusing, which is what a two-way comparison is for.
        src=${../crates/ronoctua-rescue/src}
        failed=0

        # What the console names, minus what the session owns. A user unit does
        # not reach polkit at all: there is no display manager here, and the
        # compositor and the footer are the person's own units.
        wanted=$(grep -rohE '"[A-Za-z@.-]+\.(service|target)"' "$src" | tr -d '"' | sort -u)
        for u in $wanted; do
          case "$u" in
            # The session's own, and the keyboard's — see above.
            ronoctua-footer.service|niri.service|graphical-session.target) continue ;;
            pipewire.service|pipewire-pulse.service|wireplumber.service) continue ;;
            keyd.service) continue ;;
          esac
          grep -qF "\"$u\"" rule.js || {
            echo "ERRO: o console pede '$u' e a regra de polkit nao a permite;" >&2
            echo "      a linha falharia na frente da pessoa." >&2
            failed=1
          }
        done

        # And back: every unit the rule names has to be one the console asks for.
        for u in $(grep -ohE '"ronoctua-rescue[A-Za-z@.-]*\.service"' rule.js | tr -d '"' | sort -u); do
          grep -rqF "\"$u\"" "$src" || {
            echo "ERRO: a regra permite '$u' e nenhuma linha do console a pede —" >&2
            echo "      privilegio concedido a ninguem, que e o que ninguem audita." >&2
            failed=1
          }
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: a regra permite exatamente o que o console pede"
      '';

  # `Ctrl+Alt+Del` IS OURS — and the hard way out stays systemd's.
  #
  # Two halves, and the check exists because the second is the one a person
  # would not think to look for. Taking the key over means taking what it did:
  # if the burst stopped rebooting too, a machine whose everything is frozen
  # would have no way out at all, and nothing would say so until that day.
  ctrl-alt-del-is-ours =
    let
      # `ctrl-alt-del.target` is a SYMLINK to whatever `ctrlAltDelUnit` names —
      # that is the option NixOS gives for this, and the two roads that look
      # more obvious are documented in `nix/modules/rescue.nix` as the failures
      # they are.
      aimed = self.nixosConfigurations.noctua.config.systemd.ctrlAltDelUnit;
      target = self.nixosConfigurations.noctua.config.systemd.targets.ronoctua-rescue;
      burst =
        self.nixosConfigurations.noctua.config.systemd.settings.Manager.CtrlAltDelBurstAction
          or "reboot-force";
    in
    mkCheck "ctrl-alt-del-is-ours" [ ] ''
      [ "${aimed}" = "ronoctua-rescue.target" ] || {
        echo "ERRO: o Ctrl+Alt+Del aponta para '${aimed}', que nao e o resgate." >&2
        exit 1
      }
      wants="${lib.concatStringsSep " " target.wants}"
      case "$wants" in
        *ronoctua-rescue.service*) ;;
        *)
          echo "ERRO: o ctrl-alt-del.target nao abre o resgate; ele quer: $wants" >&2
          echo "      Uma tecla tomada que nao abre nada e uma tecla perdida." >&2
          exit 1
          ;;
      esac

      # The way out that does not pass through us.
      [ "${burst}" = "reboot-force" ] || {
        echo "ERRO: CtrlAltDelBurstAction e '${burst}'; os sete toques em dois" >&2
        echo "      segundos eram a unica saida com tudo travado." >&2
        exit 1
      }

      echo "OK: um toque abre o resgate, sete ainda reiniciam a forca"
    '';

  # O CONSOLE DE RESGATE NÃO CARREGA COMPILADOR.
  #
  # He exists for a machine that is broken, and the cheapest way to make him
  # useless is to link something that drags a toolchain: `ronoctua-term` cites
  # `libghostty-vt` (Zig, clang, LLVM — 1,70 GiB) and `ronoctua-render` cites
  # mesa. Neither would FAIL anything; both would quietly make the rescue
  # console the heaviest thing in the system.
  #
  # `nix/rust.nix`'s `rescue` exists to be measured: a derivation references
  # what its own bytes cite.
  rescue-has-no-toolchain =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.rescue ]; };
    in
    mkCheck "rescue-has-no-toolchain"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0
        for unwanted in -zig- -clang- -llvm- -mesa- -vulkan-loader-; do
          if found=$(grep -e "$unwanted" ${closure}/store-paths); then
            echo "ERRO: o console de resgate arrasta $unwanted:" >&2
            echo "$found" >&2
            failed=1
          fi
        done
        [ "$failed" = 0 ] || exit 1
        echo "OK: $(wc -l < ${closure}/store-paths) caminhos, e nenhum compilador"
      '';

  # WHATEVER THE SHELL LOOKS FOR NEXT DOOR, THE DEV LOOP HAS TO BUILD.
  #
  # Two binaries are found by ASKING THE FILESYSTEM rather than by a store path
  # in a variable — `ronoctua-rescue` and `ronoctua-transfers` — and they have
  # to be: they come out of the same build as the shell, so a path baked into
  # one would be that derivation referencing itself.
  #
  # Installed, they are next door: `rust.shell` is a `symlinkJoin` over the
  # whole workspace. In the nested loop they are next door too — until the FIRST
  # edit, when `RONOCTUA_FOOTER_BIN` moves to `$CARGO_TARGET_DIR/release/` and
  # only what `cargo build` named is there.
  #
  # The symptom is the worst kind there is: everything works, and then one edit
  # quietly takes a feature away. A missing transfer queue looks exactly like
  # "nothing is running", and a missing rescue console looks exactly like a key
  # that does nothing. This check is what noticed that the rescue console had
  # been in that state the whole time.
  dev-rebuilds-what-sits-beside =
    mkCheck "dev-rebuilds-what-sits-beside" [ pkgs.gnugrep pkgs.coreutils ]
      ''
        shell=${../crates/ronoctua-shell/src}
        queue=${../crates/ronoctua-transfers/src}
        failed=0

        # The names come from the CODE that looks them up, never from a list
        # written here: a list would be a third place to keep in step.
        beside=$(grep -rhoE 'parent\(\)\?\.join\("[a-z-]+"' "$shell" "$queue" \
          | grep -oE '"[a-z-]+"' | tr -d '"' | sort -u)
        [ -n "$beside" ] || {
          echo "ERRO: ninguem procura binario ao lado — o grep casa o que?" >&2
          exit 1
        }

        for b in $beside; do
          grep -q -- "--bin $b" ${../nix/dev.nix} || {
            echo "ERRO: o rodape procura $b ao lado, e o laco de dev nao o constroi." >&2
            echo "      Depois da PRIMEIRA edicao ele some, em silencio." >&2
            failed=1
          }
        done

        [ "$failed" = 0 ] || exit 1
        echo "OK: $(echo "$beside" | wc -l) binarios ao lado, e o laco constroi todos"
      '';

  # THE QUEUE'S UNIT IS A COPY, NOT A DESKTOP.
  #
  # `ronoctua-transfers` is a user service that outlives the session — it is
  # what keeps a copy of forty gibibytes alive across a footer restart. Handing
  # it `rust.shell` instead of the extracted binary would put wgpu, the wasm
  # runtime, the renderer and three embedded fonts inside a unit that never
  # opens a window, and would hold every one of those store paths alive for as
  # long as the machine is on.
  #
  # Nothing would fail. That is exactly why this is a check and not a comment:
  # `nix why-depends` on the unit is what would say so, and nobody runs that on
  # a Tuesday.
  transfers-unit-is-narrow =
    let
      closure = pkgs.closureInfo { rootPaths = [ rust.transfers ]; };
    in
    mkCheck "transfers-unit-is-narrow"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0
        for unwanted in -zig- -clang- -llvm- -mesa- -vulkan-loader- -noctua-iosevka- -icons-superpack-; do
          if found=$(grep -e "$unwanted" ${closure}/store-paths); then
            echo "ERRO: a unidade da fila arrasta $unwanted:" >&2
            echo "$found" >&2
            failed=1
          fi
        done
        [ "$failed" = 0 ] || exit 1
        echo "OK: $(wc -l < ${closure}/store-paths) caminhos, e nenhum deles desenha"
      '';

  # THE QUEUE'S SOCKET IS THE PERSON'S, AND NOBODY ELSE'S.
  #
  # `XDG_RUNTIME_DIR` is `0700` and per user. A control socket in `/tmp` would
  # be a door anybody on the machine can knock on — and what is behind this one
  # is "copy that there" and "delete those", with the person's own privileges.
  #
  # `leader::socket_path` states the same rule for the footer's socket and has
  # no check; this one does, because the failure is silent: a fallback to `/tmp`
  # WORKS, and works better — it works when `XDG_RUNTIME_DIR` is missing, which
  # is exactly when it should refuse.
  transfers-socket-is-private = mkCheck "transfers-socket-is-private" [ pkgs.gnugrep ] ''
    src=${../crates/ronoctua-transfers/src}
    grep -q 'XDG_RUNTIME_DIR' "$src/wire.rs" || {
      echo "ERRO: o socket deixou de sair de XDG_RUNTIME_DIR." >&2; exit 1; }

    # NO FALLBACK. Not `unwrap_or`, not `/tmp`, not `std::env::temp_dir`.
    #
    # The failure this refuses is a SILENT one: a fallback to /tmp works, and
    # works better — it works when XDG_RUNTIME_DIR is missing, which is
    # exactly when it should refuse.
    #
    # The tests build their trees under the temporary directory and that is
    # fine; what may not go there is the SOCKET.
    hits=$(grep -nE '/tmp|temp_dir' "$src"/*.rs "$src"/bin/*.rs || true)
    # A comment is prose, and one of them is this very rule written out.
    bad=$(echo "$hits" | grep -vE ':[0-9]+: *//' || true)
    # A test builds its tree under the temporary directory, which is what a
    # temporary directory is for. What may not go there is the SOCKET.
    bad=$(echo "$bad" | grep -vE 'std::env::temp_dir\(\)' || true)
    bad=$(echo "$bad" | grep -vE '^ *$' || true)
    [ -z "$bad" ] || {
      echo "ERRO: a fila caiu para /tmp em algum lugar:" >&2
      echo "$bad" >&2
      exit 1
    }
    echo "OK: o socket da fila e do usuario, sem plano B"
  '';

  # EVERY ENGINE, EVERY STAGE AND EVERY KIND OF WORK HAS WORDS IN BOTH LANGUAGES.
  #
  # The key is ASSEMBLED at runtime — `transfers-stage-{}` with `Stage::word()`
  # — so it is one of the keys `docs/ARCHITECTURE.md` describes as having seven
  # derivatives and being invisible to every search in the code. A variant added
  # without its `.ftl` line does not fail to compile; it draws the raw key on
  # screen, which is precisely the shape invariant 8 exists to refuse and which
  # was reported from this system's own screen once already.
  transfers-words-are-total = mkCheck "transfers-words-are-total" [ pkgs.gnugrep pkgs.coreutils ] ''
    job=${../crates/ronoctua-transfers/src}/job.rs
    failed=0

    # The words come out of the `word()` tables — the same place the Rust
    # builds the key from. Reading the enum's variants instead would be a
    # second spelling of the same list.
    words=$(grep -oE '=> "[a-z]+",' "$job" | grep -oE '"[a-z]+"' | tr -d '"' | sort -u)
    [ -n "$words" ] || { echo "ERRO: nao achei nenhuma palavra em job.rs" >&2; exit 1; }

    for w in $words; do
      for prefix in transfers-work transfers-stage transfers-engine; do
        # A word belongs to exactly one of the three families; what this
        # loop demands is that WHEREVER it appears in one language it
        # appears in the other.
        pt=$(grep -c "^$prefix-$w = " ${../assets/i18n}/pt-BR.ftl || true)
        en=$(grep -c "^$prefix-$w = " ${../assets/i18n}/en-GB.ftl || true)
        if [ "$pt" != "$en" ]; then
          echo "ERRO: $prefix-$w existe em uma lingua so (pt=$pt en=$en)" >&2
          failed=1
        fi
      done
      # And it has to exist in AT LEAST one family, or it is a variant
      # nothing can draw.
      total=$(grep -c "^transfers-[a-z]*-$w = " ${../assets/i18n}/pt-BR.ftl || true)
      if [ "$total" = 0 ]; then
        echo "ERRO: a palavra $w de job.rs nao tem texto nenhum" >&2
        failed=1
      fi
    done

    [ "$failed" = 0 ] || exit 1
    echo "OK: $(echo "$words" | wc -l) palavras da fila, nas duas linguas"
  '';

  # THE EDITOR WRITES WHERE THE UNIT READS.
  #
  # The path crosses the border as a STRING: the crate writes to it and a root
  # oneshot reads from it. Divergent, saving would report success and install
  # nothing — the exact silence `CLAUDE.md` says needs a check.
  rescue-staging-agrees = mkCheck "rescue-staging-agrees" [ pkgs.gnugrep ] ''
    grep -q 'staged = "/run/ronoctua-rescue/staged"' ${../nix/modules}/rescue.nix || {
      echo "ERRO: o modulo mudou o caminho onde le o arquivo editado." >&2; exit 1; }
    grep -q 'pub const STAGED: &str = "/run/ronoctua-rescue/staged"' \
      ${../crates/ronoctua-rescue/src}/edit.rs || {
      echo "ERRO: o editor mudou o caminho onde grava; salvar nao instalaria nada." >&2; exit 1; }
    echo "OK: o editor grava onde a unidade le"
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

        # The command records WHAT changed, and not just that it fired: the
        # nested loop branches on that path — a `.rs` under `buffers/` is a
        # component and is built by nix, everything else is the workspace and is
        # built by cargo. If the variable stopped arriving, the loop would go on
        # reloading and would silently take the wrong branch every time.
        RONOCTUA_RELOAD_CMD="echo \"disparou \$RONOCTUA_RELOAD_FILE\" >> $PWD/marcador" \
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
        grep -q "disparou .*obs/mudou.rs" marcador || {
          echo "ERRO: disparou sem dizer QUAL arquivo mudou." >&2
          echo "      O laco aninhado escolhe entre nix e cargo por esse caminho;" >&2
          echo "      sem ele, todo buffer editado cairia no ramo da workspace." >&2
          cat marcador >&2
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
  # reload's `rebuild` runs in the WATCHER's process. The function got there
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
    # THE SECRETS ARE NAMED ONCE, and the grep below reads the same variables.
    #
    # They used to be typed twice — once in the `export` and once in the `grep`
    # — and a rename touched only one side: the check exported
    # `frase-secret_one-de-teste` and searched for `frase-secreta-de-teste`. It
    # went on passing while unable to detect a leaked passphrase at all, which is
    # the leak it exists for and the more damaging of the two.
    password=senha-de-teste
    passphrase=frase-secreta-de-teste

    export RONOCTUA_INST_DISK=/dev/nvme0n1
    export RONOCTUA_INST_HOSTNAME=noctua-teste
    export RONOCTUA_INST_PASSWORD="$password"
    export RONOCTUA_INST_PASSPHRASE="$passphrase"
    export RONOCTUA_INST_LANGUAGE=pt-BR
    # The sixth question. `br` and not the default: the dry run has to prove the
    # ANSWER reaches `host.nix`, and a value equal to the default would pass
    # even if the answer were being dropped.
    export RONOCTUA_INST_KEYBOARD=br
    # And the profile, `minimal` and not the default, for the same reason: an
    # answer equal to the default passes even when it is being dropped.
    export RONOCTUA_INST_PROFILE=minimal

    out_text=$(${rust.workspace}/bin/ronoctua-install)
    echo "$out_text"

    # The `host.nix` must NOT carry a secret (section 9): it goes to backup.
    if echo "$out_text" | grep -qF -e "$password" -e "$passphrase"; then
      echo "ERRO: o host.nix gerado contem segredo" >&2
      exit 1
    fi

    # NO label may come out as a raw key.
    #
    # `Textos::t` gives back `[api_key]` for a key that does not exist — on purpose,
    # so as not to bring the shell down over a missing text. The side effect is
    # that an unregistered key gets past ALL the other guards: `gen-i18n.py` only
    # compares `pt-BR` with `en-GB`, and the compiler only protects whoever uses
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

    # THE PROFILE REACHES THE GENERATED `host.nix`.
    #
    # It is the line that decides whether the machine installs 22,8 GiB or 6,9,
    # and it is written on BOTH answers — `full` is the module's own default, so
    # a file that only mentions the profile when it differs would be a file one
    # has to already know the default to read.
    echo "$out_text" | grep -q 'ronoctua.profile = "minimal"' || {
      echo "ERRO: a resposta do perfil nao chega ao host.nix" >&2; exit 1; }

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

    # And without `--execute` nothing runs. An installer that wipes a disk by
    # default is a dangerous command.
    #
    # The sentence comes from the `.ftl`, so matching the TEXT would tie the check
    # to one language — and the sandbox does not define `LANG`, so it would be
    # `en-GB`. What does not change with the language is the mention of
    # `--execute`, which is what the line actually promises.
    echo "$out_text" | grep -q -- '--execute' || {
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
      echo "ERRO: o plano tem $pt passos em pt-BR e $en em en-GB" >&2; exit 1; }

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
    ronoctua-shell = rust.shell;
    bufferExample = buffers.example;
    bufferGlyphs = buffers.glyphs;
  };

  # The memory panel on screen. A file of its own and not one more subtest of
  # the sibling: what it proves is the HOST reading a real `/proc` and a clock
  # that ticks, which no other buffer needs — and a failure there has nothing to
  # do with the mechanism `buffer-vm` guards.
  memoryVm = import ./memory-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
    bufferMemory = buffers.memory;
  };

  # The clipboard history: the native capture, the view and the re-copy (4.2).
  clipboardVm = import ./clipboard-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The terminal wired to the shell, with what is typed coming back to the screen (7.10).
  terminalVm = import ./terminal-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The leader as the Super key on its own: keyd standing and niri listening (7.4).
  #
  # The niri module goes along because the test checks BOTH ends of the contract
  # — the key keyd emits and the one niri listens for.
  # The rescue console's second door: `Ctrl+Alt+Del` with no compositor at all.
  rescueVm = import ./rescue-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
    };
  };

  # The file manager's privileged half: the unit, the polkit rule, and the
  # refusals the helper makes before it acts. Only a booted machine answers any
  # of them.
  filesVm = import ./files-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
      filesRoot = rust.filesRoot;
    };
  };

  # The transfer queue reaching the SCREEN: the unit comes up, the footer finds
  # it, the segment resolves to columns in the modeline, and `LEADER t t` opens
  # the panel with the list the footer already had.
  transfersVm = import ./transfers-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
    ronoctua-transfers = rust.transfers;
  };

  extensionVm = import ./extension-vm.nix {
    inherit pkgs lib extension;
    ronoctua-shell = rust.shell;
    ronoctua-transfers = rust.transfers;
  };

  keyboardVm = import ./keyboard-vm.nix {
    inherit pkgs lib;
    ronoctua = {
      inherit fonts fontconfigConf iconSuperpack;
      niri = inputs.niri;
      shell = rust.shell;
    };
  };

  # One satellite per buffer scope, proved with TWO X11 apps (9.1).
  x11Vm = import ./x11-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The root's wipe, proved between two boots (milestone 7).
  impermanenceVm = import ./impermanence-vm.nix { inherit pkgs lib; };

  # Leader, which-key and the return to rest (milestone 5).
  leaderVm = import ./leader-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The app finder, and the footer that must not freeze when opening one.
  appsVm = import ./apps-vm.nix {
    inherit pkgs lib fonts;
    niri = inputs.niri.packages.${system}.niri;
    ronoctua-shell = rust.shell;
  };

  # The IPC contract against the pinned tag AND against `niri-next` (milestone 4).
  niriContract = import ./niri-contract.nix {
    inherit pkgs lib;
    niri = inputs.niri.packages.${system}.niri;
    niriNext = inputs.niri-next.packages.${system}.niri;
    ronoctua-shell = rust.shell;
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
  # THE DEPENDENCY LAYER SEES ONLY WHAT IT USES.
  #
  # `buildDepsOnly` compiles the 483 third-party crates against stubs — crane
  # replaces every crate of ours, `build.rs` included. So it reads none of the
  # generated inputs; and yet all 31 `RONOCTUA_*` were in its environment,
  # because it was handed the whole of `commonArgs`.
  #
  # Nix addresses a derivation by its inputs. With `RONOCTUA_BUFFER_MEMORY` in
  # there, editing a buffer changed the layer's hash and paid 4m46 of
  # compilation for a value nothing in it reads — and made the layer WAIT for
  # five wasm builds it never consumes. A `.ftl` or a token did the same.
  #
  # This reads the attributes of the generated derivation, at evaluation time:
  # no build, no import-from-derivation. It is the environment that is checked
  # because the environment is how they got in — the five buffers were never
  # named in `nix/rust.nix`'s deps call, they arrived carried by a variable.
  deps-layer-is-lean =
    let
      leaked = lib.filter (lib.hasPrefix "RONOCTUA_") (lib.attrNames rust.cargoArtifacts.drvAttrs);
    in
    mkCheck "deps-layer-is-lean" [ ] ''
      vazaram="${lib.concatStringsSep " " leaked}"
      if [ -n "$vazaram" ]; then
        echo "ERRO: a camada de dependencias recebeu variaveis que nao usa:" >&2
        for v in $vazaram; do echo "  $v" >&2; done
        echo >&2
        echo "      Elas entram por \`commonArgs\`; a camada tem de receber o" >&2
        echo "      \`buildBase\`. Cada uma dessas custa 4m46 de recompilacao a" >&2
        echo "      cada vez que o valor dela muda — e um .wasm muda toda vez" >&2
        echo "      que alguem edita o buffer." >&2
        exit 1
      fi
      echo "OK: a camada de dependencias nao carrega nenhuma RONOCTUA_*"
    '';

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
    # (they are a Release asset), so what has to survive the .gitignore is the
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
    ronoctua-shell = rust.shell;
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
         "anunciar", "aplicar",
         "anterior", "antes", "antigo", "argumento", "arquivo", "arquivos",
         "arrastar", "arvore", "atribuicao", "avanco", "baixo", "banda",
         "binario", "bloco", "bruto", "busca", "cadeia", "camada", "caminho",
         "capacidade", "captura", "capturando", "capturar", "carimbo", "caso",
         "comecar",
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
         "executar", "expandido", "expansao", "fabrica", "falhou", "falta", "faltando",
         "familia", "fechar", "ficha", "fila", "filhos", "filtro", "fluxo",
         "fonte", "fontes", "fora", "fragmento", "frase", "frequencia",
         "fundo", "gesto", "glifo", "glifos", "grade", "grafico", "gravada",
         "grupo", "guarda", "guardar", "heranca", "hospedeiro", "icone",
         "icones", "idioma", "imagens", "impermanencia", "instalador",
         "instanciar", "interatividade", "interativo", "janela", "juntos", "lado",
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
         "valido", "valor", "variante", "variavel", "varrer", "vazia", "vazio", "versao",
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

    # 6) THE CLI'S FLAGS
    #
    # A flag is a NAME — it is what a person types and what a script writes —
    # and no rule above sees it: it lives as a bare string compared with `==`,
    # in any file, not only `main.rs`. `--executar` survived the whole refactor
    # in the installer AND in three places that documented it, while the very
    # docstring of the file that defined it already said `--run_command`. The
    # code and its own documentation disagreed, and nothing could say so.
    for rs in (root_dir / "crates").rglob("*.rs"):
        if "target" in rs.parts:
            continue
        for n, line in enumerate(rs.read_text().splitlines(), 1):
            for m in re.finditer(r'"--([a-z][a-z0-9-]*)"', line):
                if RX.search(m.group(1)):
                    gaps.append(f"flag: {rs.relative_to(root_dir)}:{n}: --{m.group(1)}")

    # 7) COMMENTS — the half of the invariant the message already claimed
    #
    # Invariant 18 says NAME, COMMENT and COMMIT, and the failure message below
    # said so too. The check did not: every pass above skips comment lines on
    # purpose, because prose has the same shape as a call. So a whole file of
    # Portuguese comments went in without a word — `tests/slow.nix` and
    # `tests/quick.nix` were written that way, and so were four blocks that had
    # been sitting in `nix/modules/base.nix`, `nix/modules/host.nix`,
    # `nix/git-hooks.nix` and this very file.
    #
    # Two things make prose testable with the same word list:
    #
    #   * COMMENT RUNS ARE JOINED. A backtick span often opens on one line and
    #     closes on the next, and line-by-line stripping leaves half a quoted
    #     identifier looking like Portuguese prose.
    #   * WHAT IS QUOTED DOES NOT COUNT. An English comment naming
    #     `Question::Senha` or the chapter `1. Fontes, glifos e icones` is
    #     correct English about a Portuguese thing. Backticks and double quotes
    #     come out before the ruler is applied.
    #
    # The comment marker is per LANGUAGE, not shared: a `#` at the start of a
    # line inside a Rust string is not a comment, and `plan.rs` writes exactly
    # that — the header of the generated `host.nix`, in Portuguese, which is a
    # VALUE and stays.
    MARKER = { ".rs": "//", ".nix": "#", ".py": "#" }
    for suf in ("*.rs", "*.nix", "*.py"):
        for f in sorted(root_dir.rglob(suf)):
            if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
                continue
            mark = MARKER[f.suffix]
            line_is = re.compile(r"^\s*" + re.escape(mark) + r"[!/]?\s?(.*)$")
            run, first = [], None
            for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1) :
                m = line_is.match(line)
                if m:
                    if first is None:
                        first = n
                    run.append(m.group(1))
                    continue
                if run:
                    prose = " ".join(run)
                    prose = re.sub(r"`[^`]*`", " ", prose)
                    prose = re.sub(r'"[^"]*"', " ", prose)
                    hit = RX.search(prose.lower())
                    if hit:
                        rel = f.relative_to(root_dir)
                        gaps.append(f"comentario: {rel}:{first}: {hit.group(0)}")
                run, first = [], None
            if run:
                prose = re.sub(r"`[^`]*`", " ", " ".join(run))
                prose = re.sub(r'"[^"]*"', " ", prose)
                hit = RX.search(prose.lower())
                if hit:
                    gaps.append(f"comentario: {f.relative_to(root_dir)}:{first}: {hit.group(0)}")

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
    print("OK: nada em portugues — nome de arquivo, chave, Rust, Nix, Python, subcomando, flag e COMENTARIO")
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

  # THE FIVE COMPONENTS REACH THE SHELL THAT RUNS — the two sides, compared.
  #
  # # The defect this exists for
  #
  # The components used to be baked in by `option_env!`, and that made a whole
  # class of failure impossible: if the build had the variable, the binary had
  # the path. Taking them out of the compilation (`nix/rust.nix`, and the reason
  # is written there) trades that guarantee for minutes of iteration — and a
  # trade with nothing watching it is how `LEADER i n` becomes a key that opens
  # no window, with one line in a journal nobody is reading.
  #
  # So the two sides are compared against each other, and both come from real
  # artifacts rather than from a list kept by hand here:
  #
  #   - what the RUST reads: every `RONOCTUA_BUFFER_*` named in the shell's
  #     sources. A sixth buffer shows up here the moment its command is written;
  #   - what the WRAPPER carries: the strings `makeBinaryWrapper` compiled into
  #     the binary the module, the nested loop and the VMs all run.
  #
  # A name on one side and not on the other fails. That covers both directions:
  # the buffer nobody wired up, and the variable that stopped being set.
  shell-finds-its-buffers =
    mkCheck "shell-finds-its-buffers"
      [
        pkgs.gnugrep
        pkgs.coreutils
      ]
      ''
        failed=0

        # ---- WHAT THE RUST READS ----
        lidas=$(grep -oh 'RONOCTUA_BUFFER_[A-Z_]*' \
          ${../crates/ronoctua-shell/src}/*.rs | sort -u)
        [ -n "$lidas" ] || {
          echo "ERRO: nenhum RONOCTUA_BUFFER_* no codigo do shell — o grep casa o que?" >&2
          exit 1
        }
        echo "o shell le: $(echo "$lidas" | tr '\n' ' ')"

        # ---- WHAT THE WRAPPER CARRIES ----
        # `grep -a`: the wrapper is an ELF, and both the name and the store path
        # are string literals inside it.
        for v in $lidas; do
          grep -qa "$v" ${rust.shell}/bin/ronoctua-shell || {
            echo "ERRO: o shell le $v e o embrulho nao a define — a tecla nao abriria nada" >&2
            failed=1
          }
        done

        # ---- AND EACH ONE POINTS AT A COMPONENT THAT EXISTS ----
        #
        # A variable set to a path that is not there fails at the far end, when
        # the window does not open, and the message blames the buffer.
        ${lib.concatStringsSep "\n" (
          lib.mapAttrsToList (name: path: ''
            grep -qa "${path}" ${rust.shell}/bin/ronoctua-shell || {
              echo "ERRO: ${name} nao aponta para ${path} no embrulho" >&2; failed=1; }
            case "${path}" in
              *.wasm) [ -s "${path}" ] || {
                echo "ERRO: ${path} nao existe ou esta vazio" >&2; failed=1; } ;;
              *) echo "ERRO: ${name} nao termina em .wasm" >&2; failed=1 ;;
            esac
            echo "$lidas" | grep -qx "${name}" || {
              echo "ERRO: o embrulho define ${name} e nenhum .rs do shell a le" >&2; failed=1; }
          '') rust.bufferComponents
        )}

        [ "$failed" = 0 ] || exit 1
        echo "OK: os dois lados batem, e os seis componentes estao la"
      '';

  # Section 9 requires the repository to be publishable AS IT IS. A local-path
  # input only works for whoever has both trees side by side, so it must not
  # survive publication — and since the font files went out as a Release, it no
  # longer WARNS: it fails.
  #
  # The four names are spelled out and the set is compared whole. Scanning for a
  # `fonts-` prefix alone would mean a renamed input empties the loop and the
  # check passes having verified nothing — which is exactly how the previous
  # version of this check spent its whole life (see below).
  fonts-input-publishable = mkCheck "fonts-input-publishable" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import json, sys

    # THE URLS COME FROM `flake.lock`, and not from `inputs.<name>.original.url`.
    #
    # That attribute DOES NOT EXIST on a `flake = false` input — the value is the
    # fetched tree, with `outPath` and `narHash` and nothing else. The `or`
    # fallback that used to be here therefore answered for every run, and this
    # check spent its whole life reporting on a URL nobody had written. It landed
    # in the right branch by luck, and would have gone on saying "local path"
    # after the inputs became a Release. A silent fallback needs a check; this
    # one WAS the silent fallback.
    expected = {
        "fonts-ttf",
        "fonts-ttf-nerdfonts",
        "fonts-woff2",
        "fonts-woff2-nerdfonts",
    }

    nodes = json.load(open("${self}/flake.lock"))["nodes"]
    found = {n for n in nodes if n.startswith("fonts-")}
    if found != expected:
        print("ERRO: os inputs de fonte do flake.lock nao sao os esperados.",
              file=sys.stderr)
        print("      esperados: " + ", ".join(sorted(expected)), file=sys.stderr)
        print("      achados:   " + ", ".join(sorted(found)), file=sys.stderr)
        sys.exit(1)

    failed = False
    for name in sorted(expected):
        node = nodes[name]
        original = node["original"]
        url = original.get("url") or (original["type"] + ":" + original.get("path", ""))

        # `narHash` is what actually pins this. `fonts-v1` is a mutable pointer
        # like any git tag: GitHub lets an asset be deleted and re-uploaded under
        # the same name, and without the hash the build would just take the new
        # bytes.
        pinned = node.get("locked", {}).get("narHash")

        if url.startswith("https://") or url.startswith("http://"):
            if not pinned:
                print("ERRO: input '" + name + "' e remoto mas NAO esta travado "
                      "por narHash no flake.lock.", file=sys.stderr)
                failed = True
            else:
                print("OK: input '" + name + "' e um asset remoto e fixado por "
                      "hash — " + url)
        elif url.startswith("path:") or url.startswith("/"):
            print("ERRO: o input '" + name + "' aponta para um caminho local:",
                  file=sys.stderr)
            print("        " + url, file=sys.stderr)
            print("      Isso funciona no seu disco, mas NAO para quem clonar.",
                  file=sys.stderr)
            print("      Os arquivos de fonte sao 4,1 GB e NAO entram em git (nem",
                  file=sys.stderr)
            print("      LFS): sao asset de um Release deste mesmo repositorio. Rode",
                  file=sys.stderr)
            print("      nix run .#fonts-release -- /caminho/com/noctua-iosevka,",
                  file=sys.stderr)
            print("      suba os quatro na tag fonts-v1 e aponte o input para la.",
                  file=sys.stderr)
            failed = True
        else:
            print("ERRO: input '" + name + "' com forma inesperada: " + url,
                  file=sys.stderr)
            failed = True

    sys.exit(1 if failed else 0)
    PY
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

  # EVERY COMMAND A BUFFER PUBLISHES HAS A LABEL THE HOST CAN DRAW.
  #
  # `SPC m` lists the focused buffer's keymap, and each row's description comes
  # from `cmd-<id>` — the buffer declares the id, the HOST owns the text
  # (invariant 11: a buffer does not translate). When the key is missing,
  # `Texts::t` returns `[cmd-<id>]` and does not fail, so the defect reaches the
  # screen as brackets and nothing before that says a word. It is the same hole
  # `installer-dryrun` closes for the installer's own labels.
  #
  # The literals are read from the buffers' SOURCE. The ids built at runtime
  # (`lucide`'s formats and filters) are already covered by the panels that draw
  # them, and a check that ran the components would cost a wasm instantiation to
  # learn what a grep can see.
  # EVERY TEXT KEY A BUFFER NAMES EXISTS IN BOTH LANGUAGES.
  #
  # The sibling below covers a buffer's COMMAND ids. Nothing covered the keys it
  # writes on screen — and a buffer cannot check them itself: it does not
  # translate (invariant 11), it names a key and the host looks it up. A typo
  # becomes `[memory-guard-detial]` drawn in front of the person, and only when
  # that screen is opened.
  #
  # The shape read here is the `mod key` every buffer keeps: one `pub const` per
  # key, the value a literal. A key assembled with `format!` is invisible to
  # this check — which is why the panels keep their keys as whole literals.
  buffer-keys-have-text = mkCheck "buffer-keys-have-text" [ pkgs.gnugrep ] ''
    keys=$(grep -rhoE 'pub const [A-Z_0-9]+: &str = "[a-z0-9-]+";' ${../buffers} \
      | grep -oE '"[a-z0-9-]+"' | tr -d '"' | sort -u)
    [ -n "$keys" ] || { echo "ERRO: nenhuma chave de buffer encontrada — o grep mudou?" >&2; exit 1; }

    failed=0
    for k in $keys; do
      for language in ${../assets/i18n/pt-BR.ftl} ${../assets/i18n/en-GB.ftl}; do
        grep -q "^$k = " "$language" || {
          echo "ERRO: falta $k em $(basename "$language") — a tela mostraria [$k]" >&2
          failed=1
        }
      done
    done
    [ "$failed" = 0 ] || exit 1
    echo "OK: $(echo "$keys" | wc -l) chaves de buffer, todas traduzidas nos dois idiomas"
  '';

  buffer-commands-have-labels = mkCheck "buffer-commands-have-labels" [ pkgs.gnugrep ] ''
    ids=$(grep -rhoE 'command: "[a-z0-9-]+"' ${../buffers} | grep -oE '"[a-z0-9-]+"' | tr -d '"' | sort -u)
    [ -n "$ids" ] || { echo "ERRO: nenhum comando de buffer encontrado — o grep mudou?" >&2; exit 1; }

    failed=0
    for id in $ids; do
      for language in ${../assets/i18n/pt-BR.ftl} ${../assets/i18n/en-GB.ftl}; do
        grep -q "^cmd-$id = " "$language" || {
          # NO BACKTICKS. Inside a Nix indented string they are still the
          # shell's command substitution: the first version of this line ran
          # cmd-<id> as a program and printed "falta  em", naming nothing.
          echo "ERRO: falta cmd-$id em $(basename "$language") — o SPC m mostraria [cmd-$id]" >&2
          failed=1
        }
      done
    done
    [ "$failed" = 0 ] || exit 1
    echo "OK: $(echo "$ids" | wc -l) comandos de buffer, todos com rotulo nos dois idiomas"
  '';

  # THE MEMORY POLICY HAS THE SHAPE IT CLAIMS, on the GENERATED units.
  #
  # Every line here failed against a plausible wrong version before it was
  # accepted, because every one of them is a way for this policy to look
  # configured and do nothing:
  #
  # - `systemd.oomd.enable` is ALREADY `true` in this nixpkgs, with every slice
  #   switch `false`. The daemon runs and its monitored list is empty. What arms
  #   it is `ManagedOOMMemoryPressure=kill` on a slice, and nothing else.
  # - `MemoryMin=` is capped by EVERY ancestor cgroup. Set on `session.slice`
  #   alone its effective value is ZERO — and the unit file looks right.
  # - `enableUserSlices` would install a drop-in that applies to every slice of
  #   that type, arming `session.slice`, whose only candidates are the compositor
  #   and the footer.
  # - `OOMScoreAdjust=` on a user unit is refused by the kernel and swallowed by
  #   systemd at debug level. Written here, it would build and do nothing.
  memory-policy-shape =
    let
      system = self.nixosConfigurations.vm.config.system.build.toplevel;
    in
    mkCheck "memory-policy-shape" [ pkgs.gnugrep pkgs.coreutils ] ''
      etc=$(readlink -f ${system}/etc)

      # `-R` AND NEVER `-r`. The whole of NixOS's `/etc` is symlinks into the
      # store, and `grep -r` does not follow a symlink it meets while recursing —
      # it skips it, silently, and reports nothing found. Every assertion below
      # would then be answered by the same wrong "no": the four missing ones
      # would fail correctly by accident, and the two that assert an ABSENCE
      # would pass without ever having looked.
      failed=0

      # THE CHAIN. Four levels, or the reservation is zero.
      for level in system/user.slice.d system/user-.slice.d \
                   system/user@.service.d user/session.slice.d; do
        grep -qR "MemoryMin=" "$etc/systemd/$level" 2>/dev/null || {
          echo "ERRO: sem MemoryMin em $level — a corrente quebra e o valor efetivo e zero" >&2
          failed=1
        }
      done

      # The daemon is armed, and against the applications.
      grep -qR "ManagedOOMMemoryPressure=kill" "$etc/systemd/user/app.slice.d" 2>/dev/null || {
        echo "ERRO: a app.slice nao esta armada — o oomd roda vigiando nada" >&2; failed=1; }

      # And NOT against the desktop. Both of these would put the compositor and
      # the footer in the candidate set.
      [ ! -d "$etc/systemd/user/slice.d" ] || {
        echo "ERRO: drop-in por TIPO de slice — arma a session.slice junto" >&2; failed=1; }
      [ ! -e "$etc/systemd/system/-.slice.d/overrides.conf" ] || {
        echo "ERRO: a -.slice esta armada; os candidatos dela incluem o desktop" >&2; failed=1; }

      # The upstream `session.slice` survived: a full-file override would have
      # discarded its CPUWeight without a word.
      grep -q "CPUWeight=100" "$etc/systemd/user/session.slice" 2>/dev/null || {
        echo "ERRO: a session.slice virou arquivo inteiro e perdeu o de upstream" >&2; failed=1; }

      # The setting that would be a no-op is absent.
      if grep -qR "OOMScoreAdjust=" "$etc/systemd/user/" 2>/dev/null; then
        echo "ERRO: OOMScoreAdjust numa unidade de usuario e ignorado em silencio" >&2
        failed=1
      fi

      # The two units that must never give up.
      for unit in niri.service ronoctua-footer.service; do
        grep -q "StartLimitIntervalSec=0" "$etc/systemd/user/$unit" || {
          echo "ERRO: $unit desiste depois de 5 mortes, e ai a tela fica vazia" >&2
          failed=1
        }
      done

      # THE TABLE THE PANEL READS, and it has to agree with the units.
      #
      # `LEADER s m` loosens the policy by applying the numbers from this file,
      # and the file is generated from the same options the drop-in above uses.
      # If the two ever drift, the panel would switch to numbers that are not
      # the ones a reboot brings back — and nothing else would say so.
      table="$etc/ronoctua/memory-profiles.conf"
      [ -e "$table" ] || {
        echo "ERRO: sem a tabela de perfis; o painel nao teria como afrouxar" >&2; failed=1; }
      for profile in strict tolerant; do
        grep -q "^$profile ceiling=.* pressure=.* duration=" "$table" 2>/dev/null || {
          echo "ERRO: a tabela nao traz o perfil $profile inteiro" >&2; failed=1; }
      done
      grep -q "^declared " "$table" 2>/dev/null || {
        echo "ERRO: a tabela nao diz qual perfil volta ao reiniciar" >&2; failed=1; }
      # And the strict profile's numbers ARE the ones in force on the slice —
      # the drop-in and the table come from the same options, and this is what
      # proves they were not wired to different ones.
      ceiling=$(grep '^strict ' "$table" | tr ' ' '\n' | grep '^ceiling=' | cut -d= -f2)
      grep -qR "MemoryHigh=$ceiling" "$etc/systemd/user/app.slice.d" 2>/dev/null || {
        echo "ERRO: o teto da tabela ($ceiling) nao e o da app.slice" >&2; failed=1; }

      # THE FIVE UNITS THE PANEL MAY START, and the rule that lets it.
      #
      # A panel that could start any unit would be a bigger door than the whole
      # policy above closes.
      for what in profile@ sweep@; do
        [ -e "$etc/systemd/system/ronoctua-memory-$what.service" ] || {
          echo "ERRO: falta a unidade ronoctua-memory-$what" >&2; failed=1; }
      done

      [ "$failed" = 0 ] || exit 1
      echo "OK: corrente de quatro niveis, app.slice armada, desktop fora do alcance, tabela dos dois perfis"
    '';

  # THE ISO CARRIES WHAT THE TARGET MACHINE WOULD OTHERWISE COMPILE.
  #
  # Everything a NixOS closure inherits from nixpkgs comes prebuilt from the
  # public cache. Everything this repository makes does not exist there, and was
  # being built on the machine being installed: the shell, niri from the pinned
  # input, `libghostty-vt` and its Zig toolchain, the five wasm buffers, the
  # font family from a GitHub Release. That is where an install's afternoon went.
  #
  # This check is the LIST's half of the promise, and `iso-fits-usb-stick` is the
  # artefact's: what goes on the medium has to be enough, and it has to fit.
  # Measured on 2026-08-15: 3702 MiB against a ceiling of 4096.
  #
  # The names are grepped in the seed's CLOSURE and not in the list, because
  # most of them arrive through `ronoctua-shell`'s baked store paths rather than
  # by being written out — and it is the closure that goes on the ISO.
  iso-carries-the-seed =
    let
      seed = pkgs.closureInfo {
        rootPaths = self.nixosConfigurations.installer.config.isoImage.storeContents;
      };
    in
    mkCheck "iso-carries-the-seed" [ pkgs.gnugrep pkgs.coreutils ] ''
      failed=0
      for ours in -ronoctua-0.1.0 -niri- -libghostty-vt- -ronoctua-buffer-glyphs- \
                  -ronoctua-buffer-unicode- \
                  -ronoctua-buffer-lucide- -ronoctua-buffer-memory- \
                  -ronoctua-buffer-transfers- \
                  -noctua-icon-theme- \
                  -noctua-iosevka-ttf-nerdfonts-; do
        grep -q -e "$ours" ${seed}/store-paths || {
          echo "ERRO: $ours nao vai no ISO — a maquina alvo vai compila-lo" >&2
          failed=1
        }
      done

      # The flake's inputs travel as SOURCES. Without them the target has the
      # built pieces and still has to reach the network to evaluate the flake
      # that names them — which is the same install, waiting on the same wire.
      grep -q -e "-source" ${seed}/store-paths || {
        echo "ERRO: as fontes dos inputs do flake nao vao no ISO" >&2; failed=1; }

      [ "$failed" = 0 ] || exit 1
      echo "OK: $(wc -l < ${seed}/store-paths) caminhos na semente"
    '';

  # THE REDUCED PROFILE IS REDUCED, AND STILL IS THE DISTRO.
  #
  # Measured on 2026-08-14: `full` 22,76 GiB, `minimal` 6,90 GiB.
  #
  # # Why each name is checked on BOTH sides
  #
  # A check that only says "android-studio is not in the reduced closure" goes
  # green when the name is misspelled, when the package is renamed upstream, and
  # when the whole system fails to have any packages at all. Requiring it to be
  # PRESENT in the full one first makes the difference the thing being asserted,
  # which is what the profile actually promises.
  #
  # # And what may never leave
  #
  # The second list is the more important one. A reduced profile that dropped
  # niri or the footer would be smaller and would not be this system — the point
  # was never the gigabytes, it was installing in minutes what one can then add
  # to one line at a time.
  profile-minimal-closes =
    let
      closureOf =
        drv:
        pkgs.closureInfo {
          rootPaths = [ drv ];
        };
      minimal = closureOf self.nixosConfigurations.minimal.config.system.build.toplevel;
      full = closureOf self.nixosConfigurations.noctua.config.system.build.toplevel;
    in
    mkCheck "profile-minimal-closes" [ pkgs.gnugrep pkgs.coreutils ] ''
      failed=0

      # Applications: in the full one, out of the reduced one.
      for app in -android-studio- -google-chrome- -emacs-30 -ffmpeg-full- -vlc- \
                 -flutter- -dart- -rustc-1 -go-1 -noctua-iosevka-ttf-3 \
                 -noctua-icon-theme-; do
        grep -q -e "$app" ${full}/store-paths || {
          echo "ERRO: $app nao esta no perfil completo — o nome mudou?" >&2
          failed=1
          continue
        }
        if grep -q -e "$app" ${minimal}/store-paths; then
          echo "ERRO: $app continua no perfil reduzido" >&2
          failed=1
        fi
      done

      # The superpack is on that list because the switch is REAL now. It used to
      # be excluded from the profile table for the opposite reason: the shell
      # carried its store path baked in, so turning the option off removed it
      # from `systemPackages` and left all 0,61 GiB in the closure. This line is
      # what would fail if the path were ever baked in again.

      # And what the reduced profile may NEVER lose, or it stops being this
      # system rather than becoming a smaller one.
      #
      # THE FIVE COMPONENTS ARE ON THAT LIST for the same reason the superpack
      # is on the one above, and by the same mechanism: they no longer arrive by
      # carriage inside the binary (`nix/rust.nix`), they arrive through the
      # wrapper. If the module ever points at the unwrapped workspace again, the
      # system installs a shell whose every buffer key opens nothing — and it
      # installs FASTER, with nothing else giving it away.
      for kept in -niri- -ronoctua-0.1.0 -noctua-iosevka-ttf-nerdfonts- -ghostty- \
                  -networkmanager- -systemd- \
                  -ronoctua-buffer-glyphs- \
                  -ronoctua-buffer-unicode- -ronoctua-buffer-lucide- \
                  -ronoctua-buffer-memory- -ronoctua-buffer-transfers-; do
        grep -q -i -e "$kept" ${minimal}/store-paths || {
          echo "ERRO: o perfil reduzido perdeu $kept, que faz parte da distro" >&2
          failed=1
        }
      done

      [ "$failed" = 0 ] || exit 1

      m=$(wc -l < ${minimal}/store-paths)
      f=$(wc -l < ${full}/store-paths)
      [ "$m" -lt "$f" ] || {
        echo "ERRO: o reduzido tem $m caminhos e o completo $f" >&2; exit 1; }
      echo "OK: reduzido $m caminhos, completo $f"
    '';

  # THE ISO CARRIES NOTHING THAT ONLY A DESKTOP OR A COMPILER COULD USE.
  #
  # It used to carry 2,94 GiB of exactly that, and nothing said so: the ISO
  # installed `rust.workspace`, the one derivation that holds EVERY binary of the
  # workspace. A derivation references whatever its bytes mention, so
  # `ronoctua-shell` was dragging in `libghostty-vt` — and with it Zig, clang and
  # LLVM, 1,7 GiB — plus the icon superpack and mesa through the `option_env!`
  # paths. A console installer can run none of it.
  #
  # The fix was to install only `bin/ronoctua-install` (`nix/rust.nix`), and the
  # measurement is what makes it worth a check: the installer's closure went from
  # 5,81 GiB to 2,87 GiB and the ISO from 2,1 GiB to 1,46 GiB. That is the budget
  # the seeded closure spends, so a silent regression here costs twice.
  #
  # `closureInfo` and not `nix path-info`: inside the sandbox there is no daemon
  # to ask, and the closure has to be an INPUT of the derivation to be readable.
  iso-has-no-toolchain =
    let
      closure = pkgs.closureInfo {
        rootPaths = [ self.nixosConfigurations.installer.config.system.build.toplevel ];
      };
    in
    mkCheck "iso-has-no-toolchain" [ pkgs.gnugrep ] ''
      # Each of the four was measured in the closure before the fix. `mesa` is
      # here because a TTY installer has no GL at all — if it ever comes back it
      # came back by accident.
      failed=0
      for unwanted in -zig- -clang- -llvm- -noctua-icon-theme- -mesa-; do
        if found=$(grep -e "$unwanted" ${closure}/store-paths); then
          echo "ERRO: o ISO carrega $unwanted, que so um desktop ou um compilador usa" >&2
          echo "$found" >&2
          failed=1
        fi
      done
      [ "$failed" = 0 ] || exit 1

      # And the installer binary IS there — a check that only forbids would go
      # green on an ISO that lost the installer altogether.
      grep -q -e "-ronoctua-installer" ${closure}/store-paths || {
        echo "ERRO: o proprio instalador nao esta no ISO" >&2; exit 1; }

      echo "OK: $(wc -l < ${closure}/store-paths) caminhos, nenhum de desktop ou de compilador"
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
             ${../docs/ARCHITECTURE.md} ${../docs/PENDING.md} \
             ${../docs/INSTALL.md} ${../docs/DEVELOPMENT.md}; do
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

    echo "OK: os seis documentos existem; CLAUDE.md dentro do teto"
  '';

  architecture-sections = mkCheck "architecture-sections" [ pkgs.python3 ] ''
    python3 - <<'PY'
    import pathlib, re, sys

    doc = pathlib.Path("${self}/docs/ARCHITECTURE.md").read_text()

    # EXACT match, and not "contains": the chapter heading `1. Fontes, glifos e
    # icones` contains the word `Fontes`, so a substring test would go on
    # accepting a reference to a `Fontes` section that had been renamed away.
    # That is this check passing while the defect it exists for is present, and
    # it happened on the first run.
    titles = set()
    for m in re.finditer(r"^#{1,5} (.+)$", doc, re.M):
        t = m.group(1).strip()
        titles.add(t)
        titles.add(re.sub(r"^\d+\.\s*", "", t))  # chapters carry a number

    # `section "X"` in a comment, in any tracked source file.
    ref = re.compile(r'ARCHITECTURE\.md,\s*(?:section|secao|seção)\s*"([^"]+)"')
    broken, seen = [], 0
    for f in pathlib.Path("${self}").rglob("*"):
        if f.suffix not in {".rs", ".nix", ".py", ".md"} or not f.is_file():
            continue
        try:
            content = f.read_text()
        except (UnicodeDecodeError, OSError):
            continue
        for m in ref.finditer(content):
            seen += 1
            target = m.group(1)
            if target not in titles:
                broken.append(f"{f}: secao \"{target}\" nao existe no ARCHITECTURE.md")

    if broken:
        print("ERRO: remissao a secao inexistente:", file=sys.stderr)
        for b in broken:
            print("  " + b, file=sys.stderr)
        sys.exit(1)
    print(f"OK: {seen} remissoes por nome de secao, todas resolvem")
    PY
  '';
  # WHAT THE FOOTER READS AT RUNTIME IS EXPORTED BEFORE THE FOOTER STARTS.
  #
  # `dev-exports-what-the-build-bakes` covers the variables the COMPILATION
  # bakes. It says nothing about WHEN, and the two that are not build inputs —
  # `RONOCTUA_ICONS` and `RONOCTUA_FILES_START` — were exported inside the block
  # that sets up the hot reload, which runs AFTER the first footer is already up.
  #
  # So every session's first footer had neither, and only a footer restarted by
  # a recompilation did. Both failures are silent, because a missing variable is
  # a fallback and not an error: the launcher opened with category glyphs that
  # were never replaced by the app's icon, and `LEADER .` opened the home while
  # `~/main` sat right there. The second one was reported from the screen.
  #
  # The rule is about ORDER, so the check reads line numbers: the export has to
  # come before the line that launches the footer.
  dev-exports-before-it-starts = mkCheck "dev-exports-before-it-starts" [ pkgs.python3 ] ''
    python3 - <<'FIM'
    import pathlib, re, sys

    # What the SHELL reads at runtime and no build bakes. A variable that
    # arrives late is a variable that did not arrive.
    AT_RUNTIME = ["RONOCTUA_ICONS", "RONOCTUA_FILES_START"]

    text = pathlib.Path("${../nix/dev.nix}").read_text()
    lines = text.splitlines()

    launch = None
    for n, line in enumerate(lines, 1):
        if "ronoctua-footer-single" in line and not line.lstrip().startswith("#"):
            launch = n
            break
    if launch is None:
        print("ERRO: nao achei onde o rodape sobe em nix/dev.nix.", file=sys.stderr)
        raise SystemExit(1)

    gaps = []
    for name in AT_RUNTIME:
        where = [
            n
            for n, line in enumerate(lines, 1)
            if re.match(r"\s*export " + name + r"=", line)
        ]
        if not where:
            gaps.append(f"{name}: nao e exportada em nix/dev.nix")
        elif min(where) > launch:
            gaps.append(
                f"{name}: exportada na linha {min(where)}, e o rodape sobe na {launch}"
            )

    if gaps:
        print("ERRO: variavel de runtime chega DEPOIS do rodape.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in gaps:
            print("       " + g, file=sys.stderr)
        print("", file=sys.stderr)
        print("       O primeiro rodape da sessao nao a tem, e so um restart a", file=sys.stderr)
        print("       ganha. Falha calada: variavel ausente e fallback, nao erro.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: as {len(AT_RUNTIME)} variaveis de runtime saem antes da linha {launch}")
    FIM
  '';

  # A `trap` THAT CALLS A FUNCTION THAT DOES NOT EXIST CLEANS NOTHING UP.
  #
  # `nix/bench-frames.nix` had `trap 'limpar; rm -rf "$dir"' EXIT` next to a
  # function named `cleanup`: the rename to English reached the definition and
  # not the string. Nothing said so — a shell string is not code to shellcheck,
  # and the trap only runs at exit, when nobody is reading. The bench came back
  # with its numbers and left a nested compositor and a footer RUNNING, plus the
  # temp directory, because `rm -rf` came after the word that failed.
  #
  # That is the shape this project keeps paying for: the failure is at the end,
  # the message scrolls past, and what is left behind is somebody else's
  # problem. A word that has to name something is a name, and a name can be
  # checked.
  #
  # The allow-list is short on purpose. A trap normally kills something, removes
  # something, or calls a function of the script's own; anything else earns a
  # line here and the thought that goes with it.
  trap-calls-something-that-exists = mkCheck "trap-calls-something-that-exists" [ pkgs.python3 ] ''
    python3 - <<'FIM'
    import pathlib, re, sys

    KNOWN = {
        "rm", "kill", "exit", "true", "false", "echo", "wait", "pkill",
        "umount", "rmdir", "sync", "printf", "stty", "tput", "clear",
        "killall", "chmod", "cp", "mv",
    }
    root_dir = pathlib.Path("${self}")
    gaps = []
    for f in sorted(root_dir.rglob("*.nix")):
        if any(x in f.parts for x in (".git", "result", "target", ".direnv")):
            continue
        # COMMENT LINES DO NOT COUNT — including this check's own, which
        # quotes the very trap that earned it and would flag itself.
        text = "\n".join(
            "" if l.lstrip().startswith("#") else l
            for l in f.read_text(errors="replace").splitlines()
        )
        defined = set(re.findall(r"^\s*([a-z_][a-z0-9_]*)\(\)\s*\{", text, re.M))
        for m in re.finditer(r"""trap\s+(['"])(.*?)\1""", text, re.S):
            for piece in re.split(r"[;&|\n]+", m.group(2)):
                word = piece.strip().split(" ")[0].strip()
                if not word or not re.fullmatch(r"[a-z_][a-z0-9_]*", word):
                    continue
                if word in defined or word in KNOWN:
                    continue
                line = text[: m.start()].count("\n") + 1
                rel = f.relative_to(root_dir)
                gaps.append(f"{rel}:{line}: o trap chama `{word}`")

    if gaps:
        print("ERRO: trap chamando o que nao existe.", file=sys.stderr)
        print("", file=sys.stderr)
        for g in gaps:
            print("       " + g, file=sys.stderr)
        print("", file=sys.stderr)
        print("       Nao e funcao definida no arquivo nem comando da lista.", file=sys.stderr)
        print("       O trap falha na saida, calado, e o que ele limparia fica.", file=sys.stderr)
        raise SystemExit(1)
    print("OK: todo trap chama funcao que existe ou comando conhecido")
    FIM
  '';

  # THE GATE HAS TO FETCH WHAT NO CACHE HAS — and the wiring is the risk.
  #
  # `nix/prefetch.nix` exists because one source in the `full` profile cannot be
  # downloaded by `fetchurl` on a connection that resets: it retries without
  # resuming, so it restarts from zero and never finishes. The tool works; what
  # can quietly stop being true is that the gate CALLS it.
  #
  # Unwired, nothing breaks at evaluation and nothing breaks in the short loop.
  # The gate simply goes back to dying after minutes of work, with a message
  # (`cannot download ... from any mirror`) that points at Google instead of at
  # the missing call — which is exactly how an afternoon was lost once already.
  #
  # It also has to come FIRST. Fetching after the heavy three would still work,
  # and would still make everyone pay the compile before finding out the network
  # is the problem.
  gate-fetches-what-no-cache-has = mkCheck "gate-fetches-what-no-cache-has" [ pkgs.gnugrep ] ''
    script=${gateApp}/bin/ronoctua-gate
    tool=${prefetchApp}/bin/ronoctua-prefetch

    if ! grep -qF "$tool" "$script"; then
      echo "ERRO: o gate nao chama o prefetch." >&2
      echo "      Sem ele, o gate morre depois de minutos com" >&2
      echo "      'cannot download ... from any mirror', que parece" >&2
      echo "      arquivo removido e nao e — ver nix/prefetch.nix." >&2
      exit 1
    fi

    called_at=$(grep -nF "$tool" "$script" | head -1 | cut -d: -f1)
    heavy_at=$(grep -n '^heavy=(' "$script" | head -1 | cut -d: -f1)
    if [ -z "$heavy_at" ]; then
      echo "ERRO: nao achei o bloco das tres pesadas no gate." >&2
      exit 1
    fi
    if [ "$called_at" -ge "$heavy_at" ]; then
      echo "ERRO: o prefetch vem DEPOIS das tres pesadas (linha $called_at, contra $heavy_at)." >&2
      echo "      Assim o compilador roda inteiro antes de descobrir que o" >&2
      echo "      problema e a rede." >&2
      exit 1
    fi
    echo "OK: o gate baixa o que nenhum cache tem, antes de compilar"
  '';

  # -------------------------------------------------------------------
  # The test tiers, and the guards that keep them total
  # -------------------------------------------------------------------

  # The names `tests/slow.nix` takes out of the everyday battery.
  slow = import ./slow.nix;
  # And the ten of the editing loop, which are a CHOSEN subset.
  quick = import ./quick.nix;

  # THE GUARD, and why it looks at `raw` and not at the final attrset.
  #
  # It is PART of the attrset, so reading `attrNames` of that same attrset from
  # inside one of its values would recurse. Building the set as `raw` and adding
  # the guard afterwards puts the two in order: it sees everything that existed
  # before it, which is exactly what it has to see.
  tiersAreTotal =
    let
      all = builtins.attrNames raw;
      ghosts = lib.subtractLists all slow;
      everyday = lib.subtractLists slow all;
    in
    mkCheck "test-tiers-are-total" [ pkgs.coreutils ] ''
      ${
        if ghosts == [ ] then
          ""
        else
          ''
            echo "ERRO: tests/slow.nix nomeia check que nao existe mais:" >&2
            ${lib.concatMapStringsSep "\n" (n: ''echo "       ${n}" >&2'') ghosts}
            echo "      Uma linha morta ali nao exclui nada — ela so parece que exclui," >&2
            echo "      e o check que ela deveria segurar volta para a bateria de todo" >&2
            echo "      dia sem ninguem perceber." >&2
            exit 1
          ''
      }
      ${
        if everyday != [ ] then
          ""
        else
          ''
            echo "ERRO: nenhum check sobrou para a bateria do dia a dia." >&2
            echo "      Ou tests/slow.nix engoliu tudo, ou os checks sumiram." >&2
            exit 1
          ''
      }
      echo "OK: ${toString (builtins.length everyday)} no .#most, ${toString (builtins.length slow)} so no .#gate"
    '';

  # THE SAME QUESTION, FOR THE SHORT LOOP.
  #
  # `quick` is a CHOSEN subset and does not have to be complete — but it does
  # have to be valid. A renamed check would drop out of it in silence: the list
  # would still have ten lines, and one of them would stop meaning anything.
  quickNamesExist =
    let
      ghosts = lib.subtractLists (builtins.attrNames raw) quick;
    in
    mkCheck "quick-names-exist" [ pkgs.coreutils ] ''
      ${
        if ghosts == [ ] then
          ""
        else
          ''
            echo "ERRO: tests/quick.nix nomeia check que nao existe:" >&2
            ${lib.concatMapStringsSep "\n" (n: ''echo "       ${n}" >&2'') ghosts}
            echo "      O laco curto rodaria com menos do que a lista diz." >&2
            exit 1
          ''
      }
      echo "OK: os ${toString (builtins.length quick)} do laco curto existem"
    '';

  raw = {
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
      torrent-layout-matches-cxx
      torrent-speaks-v2
      extension-has-no-fingerprint
      extension-parses
      extension-has-no-literal
      secrets-are-not-in-the-repo
      # "The repository has to be publishable as it is" (section 9).
      secrets-out-of-repo
      planted-secret-fails
      # The feedback loop is m1's goal, and a tree that gets fat destroys it
      # silently: every `nix flake check` starts copying gigabytes.
      lean-tree
      deps-layer-is-lean
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
      shell-finds-its-buffers
      rescue-actions-have-text
      rescue-runs-by-path
      rescue-staging-agrees
      rescue-has-no-toolchain
      docs-exist
      architecture-sections
      english-names
      xdg-user-dirs
      browser-has-no-keyring
      format-captures-resolve
      generator-flags-agree
      iso-fits-usb-stick
      iso-has-no-toolchain
      iso-carries-the-seed
      profile-minimal-closes
      memory-policy-shape
      buffer-commands-have-labels
      buffer-keys-have-text
      oomVm
      # The rescue console's two doors: the VT is its own machine, the window is
      # asserted next to the monitor's in `terminal-vm`.
      rescueVm
      ctrl-alt-del-is-ours
      rescue-polkit-is-narrow
      # The file manager: the two doors, the narrow privileged one, and the
      # promise that the mouse reaches nothing the keyboard does not.
      filesVm
      files-actions-have-text
      files-everything-has-a-key
      files-runs-by-path
      files-polkit-is-narrow
      files-closure-has-no-mime-db
      files-columns-degrade
      transfers-unit-is-narrow
      transfers-socket-is-private
      transfers-words-are-total
      dev-rebuilds-what-sits-beside
      graphics-does-not-mix-libc
      mime-table-generated
      mime-unknown-icon-fails
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
    inherit shell-finds-its-buffers;
    inherit
      rescue-actions-have-text
      rescue-runs-by-path
      rescue-polkit-is-narrow
      ctrl-alt-del-is-ours
      rescue-has-no-toolchain
      rescue-staging-agrees
      ;
    files-vm = filesVm;
    transfers-vm = transfersVm;
    extension-vm = extensionVm;
    inherit
      files-actions-have-text
      files-everything-has-a-key
      files-runs-by-path
      files-polkit-is-narrow
      files-closure-has-no-mime-db
      files-columns-degrade
      transfers-unit-is-narrow
      transfers-socket-is-private
      transfers-words-are-total
      dev-rebuilds-what-sits-beside
      graphics-does-not-mix-libc
      mime-table-generated
      mime-unknown-icon-fails
      gate-fetches-what-no-cache-has
      trap-calls-something-that-exists
      dev-exports-before-it-starts
      ;
    inherit
      group-mark-outside-translation
      niri-actions-translated
      keyboard-layout-exists
      keyboard-options-do-what-they-say
      installer-defaults-match-the-keyboard
      locale-exists
      wifi-console-asks-once
      ;
    inherit
      niri-config-valid
      fontconfig-nested-finds-font
      xdg-user-dirs
      browser-has-no-keyring
      ;
    notify-vm = notifyVm;
    impermanence-vm = impermanenceVm;
    buffer-vm = bufferVm;
    memory-vm = memoryVm;
    clipboard-vm = clipboardVm;
    terminal-vm = terminalVm;
    x11-vm = x11Vm;
    keyboard-vm = keyboardVm;
    rescue-vm = rescueVm;
    tpm-vm = tpmVm;
    secureboot-vm = secureBootVm;
    inherit installer-dryrun;
    oom-vm = oomVm;
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
      memory-instance-fast
      memory-driven
      transfers-instance-fast
      transfers-driven
      torrent-layout-matches-cxx
      torrent-speaks-v2
      extension-has-no-fingerprint
      extension-parses
      extension-has-no-literal
      secrets-are-not-in-the-repo
      renderer-loads-three-faces
      layout-fast
      shaping-fast
      large-monitor-surface
      reload-reloads
      reload-command-is-defined
      dev-exports-what-the-build-bakes
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
      # AND THE SYSTEM THE INSTALLER ACTUALLY INSTALLS.
      #
      # It was never built by anything here. The gate built the VM configuration
      # and the ISO — neither of which is `noctua` — so "the target system closes"
      # was an assumption, and the moment it would have been found false is during
      # `nixos-install`, with the disk already erased and nothing to go back to.
      #
      # It is not a duplicate of the VM: that one exists to be booted by a test and
      # leaves out what a real machine carries. This is the closure of the machine
      # somebody installs.
      self.nixosConfigurations.noctua.config.system.build.toplevel
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
      # And that what it calls EXISTS, and that the binary it compiles gets the
      # same environment the build bakes. Those two are the wiring; the one above
      # is the mechanism, and the mechanism worked for months with no wiring.
      reload-command-is-defined
      dev-exports-what-the-build-bakes
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
    #   missing in `pt-BR` or `en-GB`; notifications queued **one at a time**; a
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
      locale-exists
      wifi-console-asks-once
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
      memoryVm
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
      memory-instance-fast
      memory-driven
      transfers-instance-fast
      transfers-driven
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
      deps-layer-is-lean
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
      iso-has-no-toolchain
      iso-carries-the-seed
      profile-minimal-closes
      memory-policy-shape
      buffer-commands-have-labels
      buffer-keys-have-text
      docs-exist
      architecture-sections
      ;

    # THE FOUR THAT ONLY EXISTED INSIDE AN `mN`, and that is a defect of its own.
    #
    # A check one cannot run alone is a check one cannot DEBUG: it fails inside a
    # `linkFarm` of forty others, and the only way to see it again is to rebuild
    # the whole milestone. That was true of these four until the test tiers needed
    # to enumerate `checks` and would have dropped them in silence.
    inherit
      tokens-wit-name
      network-tuned
      syncthing-tuned
      ;
    wifi-tpm-vm = wifiTpmVm;

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

    # THE BUFFERS' FORMATTING, WHICH NOTHING CHECKED.
    #
    # `rust-fmt` is `cargoFmt` over the workspace, and `cargo fmt --all` reaches
    # the workspace's MEMBERS — `buffers/` is in `exclude` (a component cannot be
    # a member; `rust-lld` refuses its symbols). So the five components were
    # formatted by whoever remembered to `cd` into each one.
    #
    # The pinned toolchain, never `pkgs.rustfmt`: two versions of a formatter are
    # two opinions about the same file, and the disagreement would show up as a
    # check that fails on a tree `nix fmt` just formatted.
    buffers-fmt =
      pkgs.runCommand "check-buffers-fmt"
        {
          nativeBuildInputs = [ rustToolchain ];
        }
        ''
          # `cargo` wants a writable home even when it only formats.
          export CARGO_HOME=$TMPDIR/cargo
          failed=0
          for manifest in ${../buffers}/*/Cargo.toml; do
            name=$(basename "$(dirname "$manifest")")
            if ! cargo fmt --manifest-path "$manifest" -- --check; then
              echo "ERRO: buffers/$name esta fora de formato. Rode: nix fmt" >&2
              failed=1
            fi
          done
          [ "$failed" = 0 ] || exit 1
          echo "OK: os buffers estao formatados"
          touch $out
        '';
  };
in
# THE GUARDS GO ON AFTERWARDS, which is what lets them see the whole set
# without recursion. They are cheap — set arithmetic over names — so they land
# in the everyday battery on their own, which is where they have to be: a guard
# that only ran in the `gate` would report a broken split hours after it broke.
raw
// {
  test-tiers-are-total = tiersAreTotal;
  quick-names-exist = quickNamesExist;
}
