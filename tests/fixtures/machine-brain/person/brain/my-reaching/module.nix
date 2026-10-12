# A person's module that reaches root, approved — and that imports a sibling
# by `../` when it can: `my-common`, which has no NixOS half and so needs no
# approval. Imported from its own folder alone, `../` reaches nothing
# (`person-root-module-needs-approval`).
{
  imports = if builtins.pathExists ../my-common/helper.nix then [ ../my-common/helper.nix ] else [ ];
  environment.etc."ronoctua-fixture-my-reaching".text = "imported\n";
}
