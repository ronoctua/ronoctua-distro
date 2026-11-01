# A MANIFEST NIX'S PARSER REFUSES: a name nothing binds is refused while the
# file is parsed, before anything of it is evaluated — an error `tryEval`
# cannot catch. Root's check stops on it in a process of its own, and the
# machine leaves the folder out and never reads it
# (`a-folder-that-does-not-evaluate-stops-only-itself`). The check also
# plants one with a syntax error, which no file of the tree may hold.
{
  what = aNameNothingBinds;
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
