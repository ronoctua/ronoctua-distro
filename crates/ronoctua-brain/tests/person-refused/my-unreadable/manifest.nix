# A FOLDER THE EVALUATION CANNOT READ: `what` is an error `tryEval` does not
# catch, so reading this folder stops whoever reads it. The machine leaves
# it out before it is read (`personModulesOnly`, from the update's isolated
# verification), and `brain-person-modules-load` proves it is never read.
{
  what = 1 + "one";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
