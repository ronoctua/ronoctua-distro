# A person's module that reaches root: it is imported only while an approval
# names this folder's hash (`person-root-module-needs-approval`).
{
  environment.etc."ronoctua-fixture-my-approved".text = "imported\n";
}
