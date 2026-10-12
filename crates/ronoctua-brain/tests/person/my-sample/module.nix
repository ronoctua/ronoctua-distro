# The sample's NixOS half: one file in `/etc`. A person's half reaches root,
# so a machine imports it only once approved (`personNixosModules`, by id) —
# and the gate keeps it shut while the brain is off.
{ ... }:
{
  environment.etc."ronoctua/my-sample".text = "a person's sample module\n";
}
