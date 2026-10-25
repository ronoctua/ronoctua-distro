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

  # ---------------------------------------------------------------------
  # THE PER-SCRIPT CHAIN — the renderer's half, and the reason this file
  # says "two chains from a single list" and can now mean it.
  #
  # `chain` above answers fontconfig's question, which is "what does the
  # generic `sans-serif` mean". It is the wrong shape for the renderer: the
  # renderer resolves by absolute path through fontdb (section 6.4 forbids it
  # from consulting fontconfig), and what it needs to know is which FAMILY
  # answers for a script it just met in the middle of a line.
  #
  # cosmic-text has a table of exactly this shape built in, and it is
  # deliberately not used: it names the PROPORTIONAL CJK faces, and this
  # system is a cell grid. Ours asks for `Noto Sans Mono CJK`, whose Latin
  # advance is half its ideograph's — which is the only shape a grid can
  # place. Everything else here follows the upstream table, so a script it
  # knows about and we forgot is a difference somebody can see.
  #
  # `probe` is a codepoint that ONLY this family should answer for. It is not
  # decoration: `ronoctua-test-faces` asks the built database who draws it,
  # and that is what turns this table from a wish into a promise. Written
  # `U+XXXX` because a bare character here would be a file that renders
  # differently in every editor that opens it.
  # ---------------------------------------------------------------------
  scripts = {
    Adlam = {
      family = "Noto Sans Adlam";
      probe = "U+1E900";
    };
    Arabic = {
      family = "Noto Sans Arabic";
      probe = "U+0627";
    };
    # THE LIGATURE and not a letter: the Iosevka draws 91 of the 96 Armenian
    # codepoints itself, and a face asked for by NAME beats every fallback. A
    # probe it answers for would prove nothing at all — which is what the
    # generator now refuses, having been given this one to refuse.
    Armenian = {
      family = "Noto Sans Armenian";
      probe = "U+FB13";
    };
    Bengali = {
      family = "Noto Sans Bengali";
      probe = "U+0985";
    };
    Buhid = {
      family = "Noto Sans Buhid";
      probe = "U+1740";
    };
    Chakma = {
      family = "Noto Sans Chakma";
      probe = "U+11103";
    };
    Cherokee = {
      family = "Noto Sans Cherokee";
      probe = "U+13A0";
    };
    Deseret = {
      family = "Noto Sans Deseret";
      probe = "U+10400";
    };
    Devanagari = {
      family = "Noto Sans Devanagari";
      probe = "U+0915";
    };
    Ethiopic = {
      family = "Noto Sans Ethiopic";
      probe = "U+1200";
    };
    Georgian = {
      family = "Noto Sans Georgian";
      probe = "U+10D0";
    };
    Gothic = {
      family = "Noto Sans Gothic";
      probe = "U+10330";
    };
    Grantha = {
      family = "Noto Sans Grantha";
      probe = "U+11315";
    };
    Gujarati = {
      family = "Noto Sans Gujarati";
      probe = "U+0A85";
    };
    Gurmukhi = {
      family = "Noto Sans Gurmukhi";
      probe = "U+0A05";
    };
    Hanunoo = {
      family = "Noto Sans Hanunoo";
      probe = "U+1720";
    };
    Hebrew = {
      family = "Noto Sans Hebrew";
      probe = "U+05D0";
    };
    Javanese = {
      family = "Noto Sans Javanese";
      probe = "U+A984";
    };
    Kannada = {
      family = "Noto Sans Kannada";
      probe = "U+0C85";
    };
    Khmer = {
      family = "Noto Sans Khmer";
      probe = "U+1780";
    };
    Lao = {
      family = "Noto Sans Lao";
      probe = "U+0E81";
    };
    Malayalam = {
      family = "Noto Sans Malayalam";
      probe = "U+0D05";
    };
    Mongolian = {
      family = "Noto Sans Mongolian";
      probe = "U+1820";
    };
    Myanmar = {
      family = "Noto Sans Myanmar";
      probe = "U+1000";
    };
    Oriya = {
      family = "Noto Sans Oriya";
      probe = "U+0B05";
    };
    Runic = {
      family = "Noto Sans Runic";
      probe = "U+16A0";
    };
    Sinhala = {
      family = "Noto Sans Sinhala";
      probe = "U+0D85";
    };
    Syriac = {
      family = "Noto Sans Syriac";
      probe = "U+0710";
    };
    Tagalog = {
      family = "Noto Sans Tagalog";
      probe = "U+1700";
    };
    Tagbanwa = {
      family = "Noto Sans Tagbanwa";
      probe = "U+1760";
    };
    Tai_Le = {
      family = "Noto Sans Tai Le";
      probe = "U+1950";
    };
    Tai_Tham = {
      family = "Noto Sans Tai Tham";
      probe = "U+1A20";
    };
    Tai_Viet = {
      family = "Noto Sans Tai Viet";
      probe = "U+AA80";
    };
    Tamil = {
      family = "Noto Sans Tamil";
      probe = "U+0B85";
    };
    Telugu = {
      family = "Noto Sans Telugu";
      probe = "U+0C05";
    };
    Thaana = {
      family = "Noto Sans Thaana";
      probe = "U+0780";
    };
    Thai = {
      family = "Noto Sans Thai";
      probe = "U+0E01";
    };
  };

  # THE HAN UNIFICATION, and why a locale decides a DRAWING.
  #
  # 直 is one codepoint and four drawings: the Japanese, the Korean and the two
  # Chinese ones differ in strokes that a reader of that language reads as
  # right or as foreign. Unicode unified the codepoint and left the choice to
  # whoever renders, and the only thing that knows the answer is the language
  # the interface is in — which is why this is keyed by locale and why
  # `Text::set_locale` exists at all.
  #
  # The DEFAULT is Simplified Chinese, and it is upstream's, kept on purpose:
  # neither of the two languages this system speaks is a CJK one, so any
  # default here is a guess, and a guess that matches the rest of the world's
  # is the one a reader can predict.
  #
  # `hanScripts` says which scripts are answered this way, and WHOSE locale
  # decides. Three of the five do not follow the interface at all, and that is
  # not a shortcut: Hiragana and Katakana are written by exactly one language,
  # and Hangul by one more, so routing them through the reader's locale would
  # draw あ with a Simplified Chinese face for a Brazilian — a face that has
  # the codepoint, so nothing would look broken. Only Han itself and Bopomofo
  # are genuinely ambiguous, and those are the two that `follow`.
  hanScripts = {
    Bopomofo = "follow";
    Han = "follow";
    Hangul = "ko";
    Hiragana = "ja";
    Katakana = "ja";
  };

  han = {
    ja = "Noto Sans Mono CJK JP";
    ko = "Noto Sans Mono CJK KR";
    "zh-HK" = "Noto Sans Mono CJK HK";
    "zh-TW" = "Noto Sans Mono CJK TC";
    other = "Noto Sans Mono CJK SC";
  };

  # WHAT IS TRIED WHEN THE SCRIPT SAYS NOTHING — symbols, dingbats, arrows,
  # anything `unicode-script` calls Common. Ordered, and the first that has the
  # codepoint wins, exactly as everywhere else in this file.
  #
  # Our own face is NOT here: it is asked for by NAME and has already been
  # tried by the time a fallback is consulted (see `Text::prepare`).
  #
  # THE EMOJI FACE IS FIRST, and it was not, and `ronoctua-test-faces` refused
  # the build until it was. `nix/primary-font.nix` CUTS the 75
  # `Emoji_Presentation=Yes` codepoints out of the Iosevka precisely so the
  # colour face answers for them; the moment `Noto Sans Symbols 2` joined the
  # chain ahead of it, it answered first — U+1F550 came back monochrome, which
  # is the exact defect that cut exists to prevent, arriving from a new
  # direction. Anything reaching here that the emoji face HAS is by
  # construction an emoji-presentation codepoint, so first is where it belongs;
  # the symbol faces keep everything it does not have.
  common = [
    "Noto Color Emoji"
    "Noto Sans Mono"
    "Noto Sans"
    "Noto Sans Symbols"
    "Noto Sans Symbols 2"
    "Noto Sans Math"
  ];

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
      Generated by nix/fontconfig.nix. Do not edit by hand.
      The chain here is the SAME one the renderer uses (fallback-chain.json).
    -->
    <fontconfig>

      <!-- THE PARTS' FACES (`nix/modules/parts.nix`): what a part brings —
           the full family's variants, `fonts-full` — reaches the session
           through the parts' view, here. A folder that is not there yet is
           skipped, and fontconfig sees the faces once they arrived. -->
      <dir>${(import ./parts-names.nix).view}/share/fonts</dir>

      <!-- Generics. noctua-iosevka first in all three, by design.

           NO `binding="same"` on the aliases, on purpose. That attribute promotes
           the <prefer> families to the same binding as the original request; with
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

      <!-- No synthesised bold or italic: we have the 135 real faces. -->
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
  # EVERY FAMILY THIS FILE PROMISES, deduplicated — what `nix/coverage-fonts.nix`
  # has to find a file for, and it REFUSES to build if it cannot find one.
  #
  # Derived and never written out: a hand-kept second list is a list that goes
  # one entry stale and turns a promised script back into tofu, silently.
  coverage = lib.unique (
    (lib.mapAttrsToList (_: v: v.family) scripts) ++ (lib.attrValues han) ++ common
  );

  chainJson = (formats.json { }).generate "ronoctua-fallback-chain.json" {
    inherit
      chain
      legacyAliases
      scripts
      han
      hanScripts
      common
      coverage
      ;
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
  # The symptom was an unreadable compositor error message on screen — little boxes in
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
      <!-- And OUR rules, the same ones as the fragment: the whole `conf` comes in
           by include, and is not copied — two copies of the same rules would be
           duas verdades. -->
      <include>${confDir}/etc/fonts/conf.d/50-ronoctua.conf</include>
    </fontconfig>
  '';

  # The same chain as data, for the renderer (which does not use fontconfig).
  inherit chainJson;

  # Exposed for the checks: it is on these that the fontconfig <-> renderer
  # parity test runs (milestone 2).
  inherit
    chain
    legacyAliases
    conf
    scripts
    han
    hanScripts
    common
    coverage
    ;
}
