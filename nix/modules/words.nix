# The offline English dictionary on the machine — `ronoctua_words::wordnet`.
#
# The Open English WordNet, converted by `nix/wordsgen.nix` into two sorted TSV
# files (17 MB) that a definition or a synonym is read out of by binary search.
# Without it English is asked of Wiktionary like every other tongue, with the
# person's consent, and without consent the card says there is no dictionary.
#
# # Why the path is in the ENVIRONMENT and not in a binary
#
# The icon superpack's trap (`nix/modules/icons.nix`): a store path baked into
# `ronoctua-shell` — by `option_env!` or by the wrapper's `--set-default` — is a
# reference its closure carries whatever this switch says, and turning it off
# would remove nothing. So nothing bakes it: it is set here, inside the
# `mkIf`, and a machine without it simply has no variable.
#
# Both places, for icons.nix's reason: `sessionVariables` reaches a login
# shell, and the footer and the player are user units, which inherit the
# manager's environment rather than the shell's. The player's only where the
# player is: an `environment` for a unit nobody defines is a phantom unit.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.words;
  data = "${pkgs.callPackage ../wordsgen.nix { }}";
in
{
  options.ronoctua.words.offline = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Installs the offline English dictionary (Open English WordNet, ~17 MB
      converted), read by definitions and synonyms. The profile table turns it
      on for `full`. Off, English is asked online, with consent, like every
      other language.
    '';
  };

  config = lib.mkIf cfg.offline (
    lib.mkMerge [
      {
        environment.sessionVariables.RONOCTUA_WORDS = data;
        systemd.user.services.ronoctua-footer.environment.RONOCTUA_WORDS = data;
      }
      (lib.mkIf config.ronoctua.player.enable {
        systemd.user.services.ronoctua-player.environment.RONOCTUA_WORDS = data;
      })
    ]
  );
}
