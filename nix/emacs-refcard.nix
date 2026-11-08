# GNU's Emacs reference card, built from the Emacs this system installs — its
# own source, `etc/refcards/refcard.tex`, and the Brazilian translation beside
# it — for the card window (`crates/ronoctua-shell-manual/src/refcard.rs`,
# `RONOCTUA_EMACS_REFCARD`).
#
# NOT GNU.ORG'S PDF: that one is built from the same source, for whichever
# release the site has; this is the installed version's, nothing is
# downloaded, and nothing is pinned beyond Emacs itself.
#
# ONE COLUMN TO A PAGE, the card's own switch (`\columnsperpage`), each page
# trimmed to its column (`\pdfpagewidth`, `\hoffset`) and with no number: the
# window lays the columns side by side as many as the screen holds, as the
# paper card lays three. Each page is then an SVG whose black is
# `currentColor`, so the window draws it in the theme's ink.
#
# TeX runs here and nowhere else: the output is the SVGs alone, and they cite
# neither TeX nor Emacs (`disallowedReferences`).
{
  lib,
  runCommand,
  texliveBasic,
  poppler-utils,
  emacs,
}:

let
  refcards = "${emacs}/share/emacs/${emacs.version}/etc/refcards";
  # The card, by the language the window asks it in: the person's when the
  # source has it, the English otherwise (`refcard::pages`).
  cards = {
    en = "refcard";
    pt-BR = "pt-br-refcard";
  };
in
runCommand "ronoctua-emacs-refcard-${emacs.version}"
  {
    nativeBuildInputs = [
      texliveBasic
      poppler-utils
    ];
    disallowedReferences = [
      texliveBasic
      emacs
    ];
  }
  ''
    cp ${refcards}/{emacsver.tex,pdflayout.sty} .
    ${lib.concatStrings (
      lib.mapAttrsToList (lang: card: ''
        cp ${refcards}/${card}.tex .
        chmod u+w ${card}.tex
        # ONE COLUMN, ITS OWN WIDTH, NO NUMBER — and refused, rather than
        # built as three columns on a page, if the card stops saying either.
        grep -q '^\\columnsperpage=3' ${card}.tex
        grep -qE '^\\pdflayout=\([01]l?\)' ${card}.tex
        sed -i -E \
          -e 's/^\\columnsperpage=3/\\columnsperpage=1/' \
          -e 's/^\\pdflayout=\([01]l?\)/\\pdfpagewidth=4.4in \\pdfpageheight=10.4in \\hoffset=-0.8in \\nopagenumbers/' \
          ${card}.tex
        pdftex -interaction=nonstopmode -halt-on-error ${card}.tex > ${card}.log || {
          tail -40 ${card}.log >&2
          exit 1
        }
        mkdir -p $out/${lang}
        pages=$(pdfinfo ${card}.pdf | sed -n 's/^Pages: *//p')
        [ "$pages" -ge 1 ]
        for n in $(seq 1 "$pages"); do
          page=$(printf '%02d' "$n")
          pdftocairo -svg -f "$n" -l "$n" ${card}.pdf $out/${lang}/$page.svg
          # THE INK IS THE THEME'S: the card's black, and only its black.
          sed -i -E 's/rgb\(0%, ?0%, ?0%\)/currentColor/g' $out/${lang}/$page.svg
          grep -q currentColor $out/${lang}/$page.svg
        done
      '') cards
    )}
  ''
