# A file the sample's `module.nix` imports, as a module that splits its units
# into siblings would: the gate reaches it too.
{
  environment.etc."ronoctua/pilot-sample-imported".text = "imported by the sample module\n";
}
