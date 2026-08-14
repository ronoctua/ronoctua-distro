# noctua icon superpackage — section 6.6.
#
# It merges several themes into one, to serve step 2 of the tray icon
# resolution chain (4.2): a pixmap fallback for FOREIGN apps that bring
# their own `IconName`.
#
# INVARIANT 10: no design system component consults this package. The
# UI's icons are glyphs from the font (6.3). If some crate imports from here,
# someone has misunderstood.
{
  lib,
  stdenvNoCC,
  python3,
  zafiro-icons,
  adwaita-icon-theme,
  morewaita-icon-theme,
  papirus-icon-theme,
  kdePackages,
  hicolor-icon-theme,
}:

let
  # Section 6.6's priority order, plus the two justified extras.
  # The lower number wins.
  #
  # VARIANT CHOICE: several packages ship Dark/Light. We take the base
  # one when it exists (Papirus, breeze); Zafiro only publishes Dark and Light, so
  # Dark it is. Light/dark variant is the theme's business (7.6), not the
  # pixmap fallback's.
  sources = [
    {
      prio = 1;
      name = "Zafiro-icons-Dark";
      pkg = zafiro-icons;
      # Zafiro is GPL-3.0; see share/doc in the package itself.
      license = "GPL-3.0-or-later";
    }
    {
      prio = 2;
      name = "Adwaita";
      pkg = adwaita-icon-theme;
      license = "CC-BY-SA-3.0 OR LGPL-3.0-or-later";
    }
    {
      prio = 3;
      name = "MoreWaita";
      pkg = morewaita-icon-theme;
      license = "GPL-3.0-or-later";
    }
    {
      prio = 4;
      name = "Papirus";
      pkg = papirus-icon-theme;
      license = "GPL-3.0-only";
    }
    {
      # Extra 1: the only decent coverage of KDE/Qt apps. Adwaita and
      # MoreWaita ignore that whole world. NOT top-level in nixpkgs.
      prio = 5;
      name = "breeze";
      pkg = kdePackages.breeze-icons;
      license = "LGPL-3.0-only";
    }
    # Extra 2: `hicolor` does NOT join the merge — it has no icon at all
    # (verified: 0 files), and is only the mandatory terminal of the
    # inheritance chain. It shows up as `Inherits=hicolor` in the generated index.theme.
  ];

  py = python3;

  srcArgs = lib.concatMapStringsSep " " (
    s:
    "--source ${lib.escapeShellArg "${toString s.prio}:${s.name}:${s.pkg}/share/icons/${s.name}:${s.license}"}"
  ) sources;
in
stdenvNoCC.mkDerivation {
  pname = "noctua-icon-theme";
  version = "1";

  nativeBuildInputs = [
    py
    hicolor-icon-theme
  ];

  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    python3 ${../scripts/merge-icons.py} ${srcArgs} --theme-name noctua --out $out
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    # --- licences ------------------------------------------------------
    # Section 6.6 is explicit: since the repository is publishable (section 9), the
    # derivation has to record the SOURCE licences. We do not collapse them into
    # one — the manifest carries the licence by winning name, which is the
    # only way to answer "where did this icon come from and under what terms".
    lic=$out/share/doc/noctua-icons/LICENSES
    mkdir -p "$lic"
    ${lib.concatMapStringsSep "\n" (s: ''
      # `-type f` and the parentheses are NOT optional: without them `-o` also matches
      # the `share/licenses` directory, and `cp` fails with
      # "omitting directory". It cost a build.
      for cand in ${s.pkg}/share/licenses ${s.pkg}/share/doc; do
        [ -d "$cand" ] || continue
        f=$(find "$cand" -type f \( -iname 'COPYING*' -o -iname 'LICENSE*' \) 2>/dev/null | head -1)
        if [ -n "$f" ]; then
          cp "$f" "$lic/${s.name}.txt"
          break
        fi
      done
      if [ ! -f "$lic/${s.name}.txt" ]; then
        printf '%s\n' \
          "Tema: ${s.name}" \
          "Licenca declarada: ${s.license}" \
          "" \
          "O pacote de origem nao redistribui o texto da licenca. O" \
          "identificador SPDX acima e o que consta em nixpkgs e no upstream." \
          > "$lic/${s.name}.txt"
      fi
    '') sources}

    runHook postInstall
  '';

  meta = {
    description = "Superpacote de icones da ronoctua-distro — fallback de pixmap para apps estrangeiros";
    longDescription = ''
      Funde Zafiro, Adwaita, MoreWaita, Papirus e breeze num tema unico,
      casando por nome + tamanho + contexto + escala. Emite manifesto
      dizendo qual pacote venceu cada nome, e um relatorio de cobertura.

      Existe SO como fallback de pixmap para apps estrangeiros (secao 4.2,
      passo 2). Nenhum componente do design system o consulta.
    '';
    # Do NOT collapse to a single licence: the merged theme carries all the source ones.
    license = with lib.licenses; [
      gpl3Plus
      gpl3Only
      cc-by-sa-30
      lgpl3Plus
      lgpl3Only
    ];
    platforms = lib.platforms.all;
  };
}
