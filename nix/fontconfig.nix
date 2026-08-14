# Font fallback chain — section 6.4.
#
# Two chains come out of THIS file, from a single list:
#
#   1. fontconfig's, for the foreign apps;
#   2. the renderer's, which does NOT use fontconfig.
#
# Section 6.4 is explicit: if the renderer depends on fontconfig, it breaks in
# the installer and in a minimal environment. So it resolves by absolute path,
# through fontdb. But the two chains have to agree, otherwise the same text
# renders differently inside and outside our buffers — hence the single source here.
#
# The parity test that proves it lives in tests/checks.nix (milestone 2).
{
  lib,
  writeText,
  writeTextDir,
  formats,
  fonts,
  fontconfig,
  # The chain's FALLBACK fonts. Empty works: whatever is missing simply
  # does not join the chain, and the text falls back to our font — which is the
  # first item of all three generics anyway.
  fallbackFonts ? [ ],
  noto ? null,
}:

let
  inherit (fonts) families;

  # ---------------------------------------------------------------------
  # The chain, in order. The first item wins.
  #
  # Section 6.4 demands the patched noctua-iosevka FIRST in all three
  # generics — serif, sansSerif and monospace. It is not a mistake: the whole
  # system is monospaced by design (section 5.1, terminal aesthetics), so an
  # app asking for "serif" should receive our font, not a real
  # serif that would break the visual coherence.
  # ---------------------------------------------------------------------
  chain = {
    monospace = [
      families.mono
      "Noto Sans Mono"
      "Noto Sans Mono CJK JP"
      "DejaVu Sans Mono"
      "Noto Color Emoji"
    ];
    sansSerif = [
      families.mono
      "Noto Sans"
      "Noto Sans CJK JP"
      "DejaVu Sans"
      "Noto Color Emoji"
    ];
    serif = [
      families.mono
      "Noto Serif"
      "Noto Serif CJK JP"
      "DejaVu Serif"
      "Noto Color Emoji"
    ];
    emoji = [
      "Noto Color Emoji"
    ];
    math = [
      "STIX Two Math"
      "Noto Sans Math"
    ];
  };

  # Name substitutions for legacy apps that hard-code families
  # (section 6.4). Liberation covers Arial/Times/Courier metrics; the
  # rest falls back to our font.
  legacyAliases = {
    "Arial" = "Liberation Sans";
    "Helvetica" = "Liberation Sans";
    "Times" = "Liberation Serif";
    "Times New Roman" = "Liberation Serif";
    "Courier" = "Liberation Mono";
    "Courier New" = "Liberation Mono";
    "Verdana" = "Liberation Sans";
    "Georgia" = "Liberation Serif";
    "Segoe UI" = "Liberation Sans";
    "Menlo" = families.mono;
    "Monaco" = families.mono;
    "Consolas" = families.mono;
    "SF Mono" = families.mono;
  };

  # The keys of `chain` are the names of the NixOS OPTIONS
  # (`fonts.fontconfig.defaultFonts.sansSerif`). fontconfig's generics
  # are spelt differently. Emitting `<family>sansSerif</family>` matches nothing:
  # the alias is accepted silently and simply never fires, so
  # `fc-match sans-serif` fell back to the defaults. monospace and serif hid the
  # mistake by having the same name in both worlds.
  fcGeneric = {
    monospace = "monospace";
    sansSerif = "sans-serif";
    serif = "serif";
    emoji = "emoji";
    math = "math";
  };

  familyList = xs: lib.concatMapStrings (f: "        <family>${f}</family>\n") xs;

  aliasBlock = from: to: ''
    <match target="pattern">
      <test qual="any" name="family"><string>${from}</string></test>
      <edit name="family" mode="assign" binding="same"><string>${to}</string></edit>
    </match>
  '';

  conf = ''
    <?xml version="1.0"?>
    <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
    <!--
      Gerado por nix/fontconfig.nix. Nao editar a mao.
      A cadeia aqui e a MESMA que o renderer usa (fallback-chain.json).
    -->
    <fontconfig>

      <!-- Genericos. noctua-iosevka em first nos tres, por desenho.

           SEM `binding="same"` nos alias, de proposito. Esse atributo promove
           as familias do <prefer> ao mesmo binding do pedido original; com
           several STRONG candidates, the <prefer> order stops applying and the
           result is a lottery. With `binding="same"` fc-match returned
           "Noto Color Emoji" for monospace AND for serif. <prefer> on its own
           gives a weak binding, which is the right behaviour for a generic. -->
    ${lib.concatStringsSep "\n" (
      lib.mapAttrsToList (generic: fams: ''
          <alias>
            <family>${fcGeneric.${generic}}</family>
            <prefer>
        ${familyList fams}    </prefer>
          </alias>
      '') chain
    )}

      <!-- Legacy apps that hard-code family names. -->
    ${lib.concatStrings (lib.mapAttrsToList aliasBlock legacyAliases)}

      <!-- Rendering: the font already ships hinting from ttfautohint 1.8.4,
           so we use the file's own and not the autohinter's. -->
      <match target="font">
        <edit name="autohint" mode="assign"><bool>false</bool></edit>
        <edit name="hinting" mode="assign"><bool>true</bool></edit>
        <edit name="hintstyle" mode="assign"><const>hintslight</const></edit>
        <edit name="antialias" mode="assign"><bool>true</bool></edit>
        <edit name="rgba" mode="assign"><const>rgb</const></edit>
        <edit name="lcdfilter" mode="assign"><const>lcddefault</const></edit>
      </match>

      <!-- The font is monospaced at 0.600 em. Do not let fontconfig
           synthesize spacing, which would ruin the cell arithmetic the
           footer depends on (section 4.1). -->
      <match target="pattern">
        <test name="family"><string>${families.mono}</string></test>
        <edit name="spacing" mode="assign"><const>mono</const></edit>
      </match>

      <!-- Sem negrito nem italico sintetizados: temos as 135 faces reais. -->
      <match target="font">
        <edit name="embolden" mode="assign"><bool>false</bool></edit>
        <edit name="matrix" mode="assign">
          <matrix><double>1</double><double>0</double><double>0</double><double>1</double></matrix>
        </edit>
      </match>

    </fontconfig>
  '';

  # The same chain, as data, for the renderer to consume. The
  # ronoctua-render crate reads this at build time and generates the path list.
  chainJson = (formats.json { }).generate "ronoctua-fallback-chain.json" {
    inherit chain legacyAliases;
    primary = families.mono;
    # A verifiable invariant: the cell is 0.600 em at normal width.
    # Measured in the inventory, not assumed. See docs/ARCHITECTURE.md.
    cellWidthEm = 0.6;
    unitsPerEm = 1000;
  };
  # Installable derivation: the snippet that goes into fonts.fontconfig.
  confDir = writeTextDir "etc/fonts/conf.d/50-ronoctua.conf" conf;
in
{
  inherit confDir;

  # THE COMPLETE FILE, for whoever points `FONTCONFIG_FILE` at it.
  #
  # # Why it exists, and what its absence caused
  #
  # `FONTCONFIG_FILE` replaces fontconfig's ENTIRE configuration with the
  # file it points to. The `confDir` above is a FRAGMENT — only the alias
  # rules, with no `<dir>` at all — and pointing `FONTCONFIG_FILE` at it left
  # fontconfig with no font directory: no font found, and everything
  # that goes through it turns into TOFU.
  #
  # The symptom was an unreadable niri error message on screen — little boxes in
  # place of every letter. Precisely the message that says what is wrong.
  #
  # Our renderer does not go through here (section 6.4: it resolves by
  # absolute path), and that is why the footer looked normal while the
  # compositor's warning was unreadable. Two chains, and only one broken.
  #
  # Here are the three things a `fonts.conf` needs to have and the fragment does
  # not: the directories, the cache, and the system's base.
  fullConf = writeText "ronoctua-fonts.conf" ''
    <?xml version="1.0"?>
    <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
    <!--
      GENERATED by nix/fontconfig.nix. Do not edit by hand.

      THE COMPLETE FILE, for `FONTCONFIG_FILE`. On NixOS what is used is the
      FRAGMENT (`confDir`), through `fonts.fontconfig.localConf`.
    -->
    <fontconfig>
      <!-- SELF-CONTAINED, and not an `<include>` of the system's base.
           Including nixpkgs' `fonts.conf` drags the HOST's `/etc/fonts/conf.d`
           along, and the rules there beat ours: in the test,
           `fc-match sans-serif` returned "Liberation Sans" instead of our
           font. The nested loop exists to show what the distro will show, and
           with the host's config in the middle it shows something else. -->

      <!-- THE CACHE. Without it fontconfig rescans everything on every
           start and warns on stderr; with it, the second start is instant. -->
      <cachedir prefix="xdg">fontconfig</cachedir>

      <!-- THE DIRECTORIES. Without them there is no font at all, and every
           glyph turns into a little box. -->
      <dir>${fonts.noctua-iosevka-ttf-nerdfonts}/share/fonts</dir>
      <dir>${fonts.noctua-iosevka-core}/share/fonts</dir>
    ${lib.concatMapStrings (p: "  <dir>${p}/share/fonts</dir>\n") fallbackFonts}
      <!-- E as NOSSAS regras, as mesmas do fragmento: o `conf` inteiro entra
           por include, e nao copiado — duas copias das mesmas regras seriam
           duas verdades. -->
      <include>${confDir}/etc/fonts/conf.d/50-ronoctua.conf</include>
    </fontconfig>
  '';

  # The same chain as data, for the renderer (which does not use fontconfig).
  inherit chainJson;

  # Exposed for the checks: it is on these that the fontconfig <-> renderer
  # parity test runs (milestone 2).
  inherit chain legacyAliases conf;
}
