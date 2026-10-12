# Code beside the person's modules, in a folder with no `module.nix`: no
# approval covers it. Imported as root, it would write this file.
{
  environment.etc."ronoctua-fixture-my-common".text = "reached\n";
}
