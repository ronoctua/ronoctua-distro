# A module whose NixOS half no approval covers: root never evaluates any of
# it, this manifest included (`machine-compiles-approved-person-modules`).
throw "the manifest of a module no approval covers was evaluated"
