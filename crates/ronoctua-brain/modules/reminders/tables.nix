# THE WORDS A REMINDER'S ANSWER IS READ WITH, as the Rust table `grammar.rs`
# reads: this folder's own `reminders-grammar-*` texts, per declared language,
# in the languages' order — written by `grammar.py` with `gen-i18n.py`'s
# reader, so the words the build checks for every language and the words the
# brain reads are one list.
{ pkgs, ... }:
let
  languages = import ../../../../assets/i18n/languages.nix;
  prefix = "${baseNameOf ./.}-grammar-";
  words = pkgs.runCommand "ronoctua-brain-words-rs" { nativeBuildInputs = [ pkgs.python3 ]; } ''
    python3 ${./grammar.py} \
      --parser ${../../../../nix/scripts/gen-i18n.py} \
      --texts ${./i18n} \
      --prefix ${prefix} \
      ${pkgs.lib.concatMapStringsSep " " (l: "--language ${l}") languages} \
      --out $out
  '';
in
{
  "words.rs" = "${words}";
}
