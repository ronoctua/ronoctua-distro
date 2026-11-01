# A person's module that reaches root, and no approval names it: the machine
# imports none of it and compiles none of it
# (`machine-compiles-approved-person-modules`).
{
  environment.etc."ronoctua-fixture-my-waiting".text = "imported\n";
}
