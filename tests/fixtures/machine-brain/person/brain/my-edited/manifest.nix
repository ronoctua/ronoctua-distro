# A module whose NixOS half no approval covers: root never evaluates any of
# it, this manifest included (`person-root-module-needs-approval`).
throw "the manifest of a module no approval covers was evaluated"
