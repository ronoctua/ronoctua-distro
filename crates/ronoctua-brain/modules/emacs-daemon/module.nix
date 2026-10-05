# The Emacs watch has no NixOS half of its own.
#
# The unit it offers to start is the distro's editor, declared where the
# editor is (`nix/modules/emacs.nix`, `services.emacs.enable`): switching this
# watch off must not take anybody's editor away, and switching the editor off
# leaves a watch that says nothing — it finds no Emacs, and a machine with no
# Emacs is not a machine with a problem. So this file declares nothing, and
# is here because every module's folder has one.
_: { }
