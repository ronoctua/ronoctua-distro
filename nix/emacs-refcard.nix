# GNU's Emacs reference card, READ from the Emacs this system installs — its
# own source, `etc/refcards/refcard.tex`, and the Brazilian translation beside
# it — for the card window (`crates/ronoctua-shell-manual/src/refcard.rs`,
# `RONOCTUA_EMACS_REFCARD`).
#
# NOT TYPESET: the card's words, its sections and its keys come out of the
# source as data (`gen-refcard.py`, one `<language>.json` each), and the window
# lays them out itself, in the system's face and the theme's inks, as many
# columns as the screen holds. A macro the reader does not know fails the
# build here, with its line — never a card that quietly loses a row.
#
# NOT GNU.ORG'S PDF either: this is the installed version's card, nothing is
# downloaded, and nothing is pinned beyond Emacs itself. The output cites no
# Emacs (`disallowedReferences`).
{
  lib,
  runCommand,
  python3,
  emacs,
}:

let
  refcards = "${emacs}/share/emacs/${emacs.version}/etc/refcards";
  # The card, by the language the window asks it in: the person's when the
  # source has it, the English otherwise (`refcard::card`).
  cards = {
    en = "refcard";
    pt-BR = "pt-br-refcard";
  };
in
runCommand "ronoctua-emacs-refcard-${emacs.version}"
  {
    nativeBuildInputs = [ python3 ];
    disallowedReferences = [ emacs ];
  }
  ''
    mkdir -p $out
    ${lib.concatStrings (
      lib.mapAttrsToList (lang: card: ''
        python3 -I ${./scripts/gen-refcard.py} ${refcards}/${card}.tex ${refcards}/emacsver.tex $out/${lang}.json
      '') cards
    )}
  ''
